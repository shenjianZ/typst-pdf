use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::Datelike;
use tokio::time::timeout;
use typst::diag::{FileError, FileResult, SourceDiagnostic, Warned};
use typst::foundations::{Bytes, Datetime};
use typst::layout::PagedDocument;
use typst::syntax::{FileId, Source, VirtualPath};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World, compile};
use typst_kit::download::{Downloader, ProgressSink};
use typst_kit::fonts::{FontSearcher, FontSlot};
use typst_kit::package::PackageStorage;

use crate::config::RenderConfig;
use crate::models::{RenderOptions, RenderRequest, SourceType, TemplateRecord};
use crate::utils::{AppError, AppResult, markdown_to_typst};

#[derive(Debug, Clone)]
pub struct RenderedProject {
    pub workdir: PathBuf,
    pub entrypoint: PathBuf,
}

#[async_trait]
pub trait Renderer: Send + Sync {
    async fn materialize(
        &self,
        request: &RenderRequest,
        template: Option<&TemplateRecord>,
        workdir: &Path,
        templates_dir: &Path,
    ) -> AppResult<RenderedProject>;
    async fn compile_pdf(&self, project: &RenderedProject) -> AppResult<PathBuf>;
}

pub type DynRenderer = Arc<dyn Renderer>;

#[derive(Debug, Clone)]
pub struct TypstRenderer {
    config: RenderConfig,
}

impl TypstRenderer {
    pub fn new(config: RenderConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Renderer for TypstRenderer {
    async fn materialize(
        &self,
        request: &RenderRequest,
        template: Option<&TemplateRecord>,
        workdir: &Path,
        templates_dir: &Path,
    ) -> AppResult<RenderedProject> {
        match request.source_type {
            SourceType::Markdown => {
                let template_record = template.ok_or_else(|| {
                    AppError::BadRequest("markdown requests require template_id".to_owned())
                })?;
                let entrypoint = workdir.join("main.typ");
                let local_template_dir = workdir.join("templates").join(&template_record.id);
                copy_dir_all(
                    &templates_dir.join(&template_record.id),
                    &local_template_dir,
                )
                .await?;
                let typst =
                    markdown_to_typst(&request.source, &request.variables, &request.render_options);
                let content = wrap_with_template(
                    &typst,
                    template_record,
                    &local_template_dir,
                    &request.render_options,
                );
                tokio::fs::write(&entrypoint, content).await?;
                Ok(RenderedProject {
                    workdir: workdir.to_path_buf(),
                    entrypoint,
                })
            }
            SourceType::Typst => {
                let entrypoint_name = request
                    .entrypoint
                    .clone()
                    .unwrap_or_else(|| "main.typ".to_owned());
                let entrypoint = workdir.join(&entrypoint_name);
                if let Some(parent) = entrypoint.parent() {
                    tokio::fs::create_dir_all(parent).await?;
                }
                tokio::fs::write(&entrypoint, &request.source).await?;
                Ok(RenderedProject {
                    workdir: workdir.to_path_buf(),
                    entrypoint,
                })
            }
        }
    }

    async fn compile_pdf(&self, project: &RenderedProject) -> AppResult<PathBuf> {
        let pdf_path = project.workdir.join("output.pdf");
        let config = self.config.clone();
        let workdir = project.workdir.clone();
        let entrypoint = project.entrypoint.clone();

        let pdf_bytes = timeout(
            Duration::from_secs(config.timeout_secs),
            tokio::task::spawn_blocking(move || compile_project(&config, &workdir, &entrypoint)),
        )
        .await
        .map_err(|_| AppError::Render("typst compile timed out".to_owned()))?
        .map_err(|err| AppError::Render(format!("renderer join error: {err}")))??;

        tokio::fs::write(&pdf_path, pdf_bytes).await?;
        Ok(pdf_path)
    }
}

fn compile_project(config: &RenderConfig, workdir: &Path, entrypoint: &Path) -> AppResult<Vec<u8>> {
    let world = TypstSystemWorld::new(config, workdir, entrypoint)?;
    let Warned { output, warnings } = compile::<PagedDocument>(&world);

    if !warnings.is_empty() {
        tracing::warn!("typst emitted {} warnings", warnings.len());
    }

    let document = output.map_err(render_diagnostics)?;
    typst_pdf::pdf(&document, &Default::default())
        .map(|bytes| bytes.to_vec())
        .map_err(render_diagnostics)
}

fn render_diagnostics(errors: impl IntoIterator<Item = SourceDiagnostic>) -> AppError {
    let message = errors
        .into_iter()
        .map(|error| error.message.to_string())
        .collect::<Vec<_>>()
        .join("; ");
    AppError::Render(message)
}

struct TypstSystemWorld {
    root: PathBuf,
    main: FileId,
    library: LazyHash<Library>,
    book: LazyHash<FontBook>,
    fonts: Vec<FontSlot>,
    package_storage: PackageStorage,
}

impl TypstSystemWorld {
    fn new(config: &RenderConfig, root: &Path, entrypoint: &Path) -> AppResult<Self> {
        let (book, fonts) = load_fonts(&config.fonts_dir);
        let entry_rel = entrypoint
            .strip_prefix(root)
            .map_err(|_| AppError::Internal("entrypoint must be inside workdir".to_owned()))?;

        Ok(Self {
            root: root.to_path_buf(),
            main: FileId::new(None, VirtualPath::new(entry_rel)),
            library: LazyHash::new(Library::builder().build()),
            book: LazyHash::new(book),
            fonts,
            package_storage: PackageStorage::new(
                None,
                Some(config.packages_dir.clone()),
                Downloader::new("typst-service"),
            ),
        })
    }
}

impl World for TypstSystemWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &self.book
    }

    fn main(&self) -> FileId {
        self.main
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        let path = self.resolve_path(id)?;
        let text = std::fs::read_to_string(&path).map_err(|err| FileError::from_io(err, &path))?;
        Ok(Source::new(id, text))
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        let path = self.resolve_path(id)?;
        let bytes = std::fs::read(&path).map_err(|err| FileError::from_io(err, &path))?;
        Ok(Bytes::new(bytes))
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.get(index).and_then(FontSlot::get)
    }

    fn today(&self, offset: Option<i64>) -> Option<Datetime> {
        let now = match offset {
            Some(hours) => chrono::Utc::now() + chrono::Duration::hours(hours),
            None => chrono::Local::now().to_utc(),
        };
        Datetime::from_ymd(now.year(), now.month() as u8, now.day() as u8)
    }
}

impl TypstSystemWorld {
    fn resolve_path(&self, id: FileId) -> Result<PathBuf, FileError> {
        if let Some(spec) = id.package() {
            let mut progress = ProgressSink;
            let package_dir = self
                .package_storage
                .prepare_package(spec, &mut progress)
                .map_err(FileError::from)?;
            let vpath = id
                .vpath()
                .resolve(&package_dir)
                .ok_or(FileError::AccessDenied)?;
            if !vpath.starts_with(&package_dir) {
                return Err(FileError::AccessDenied);
            }
            return Ok(vpath);
        }

        let vpath = id
            .vpath()
            .resolve(&self.root)
            .ok_or(FileError::AccessDenied)?;
        if !vpath.starts_with(&self.root) {
            return Err(FileError::AccessDenied);
        }
        Ok(vpath)
    }
}

fn load_fonts(fonts_dir: &Path) -> (FontBook, Vec<FontSlot>) {
    let mut searcher = FontSearcher::new();
    let fonts = if fonts_dir.exists() {
        searcher.search_with([fonts_dir])
    } else {
        searcher.search()
    };
    (fonts.book, fonts.fonts)
}

fn wrap_with_template(
    body: &str,
    template: &TemplateRecord,
    _template_dir: &Path,
    options: &RenderOptions,
) -> String {
    let import_path = normalize_path(
        &Path::new("templates")
            .join(&template.id)
            .join(&template.entrypoint),
    );
    let page_size = options.page_size.as_deref().unwrap_or("a4");
    let margin = options.margin.as_deref().unwrap_or("2cm");
    let language = options.language.as_deref().unwrap_or("en");
    let font_family = options.font_family.as_deref().unwrap_or("Liberation Serif");
    let show_toc = options.show_toc.unwrap_or(true);

    format!(
        r#"#set page(paper: "{page_size}", margin: {margin})
#set text(lang: "{language}", font: "{font_family}")
#import "{import_path}": template

#let content = [
{body}
]

#show: doc => template(
  content,
  show_toc: {show_toc}
)
"#
    )
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

async fn copy_dir_all(source: &Path, destination: &Path) -> AppResult<()> {
    let mut stack = vec![(source.to_path_buf(), destination.to_path_buf())];

    while let Some((src_dir, dst_dir)) = stack.pop() {
        tokio::fs::create_dir_all(&dst_dir).await?;
        let mut entries = tokio::fs::read_dir(&src_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let entry_path = entry.path();
            let target_path = dst_dir.join(entry.file_name());
            let metadata = entry.metadata().await?;
            if metadata.is_dir() {
                stack.push((entry_path, target_path));
            } else {
                tokio::fs::copy(&entry_path, &target_path).await?;
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use chrono::Utc;
    use tempfile::tempdir;

    use super::{compile_project, wrap_with_template};
    use crate::config::RenderConfig;
    use crate::models::{RenderOptions, TemplateRecord};
    use crate::utils::markdown_to_typst;

    #[test]
    fn compiles_complex_markdown_to_pdf() {
        let markdown = r#"# Title

Paragraph with #hash and $cash.

1. ordered
   - nested item
   - [x] done item

> quoted text

| Name | Value |
| --- | --- |
| A | `code` |

Inline footnote[^one].

[^one]: Footnote text
"#;
        let body = markdown_to_typst(markdown, &BTreeMap::new(), &RenderOptions::default());

        let workspace = tempdir().expect("workspace");
        let template_dir = workspace.path().join("templates").join("report");
        std::fs::create_dir_all(&template_dir).expect("template dir");
        std::fs::write(
            template_dir.join("template.typ"),
            "#let template(content, show_toc: true) = [#content]",
        )
        .expect("template");

        let template = TemplateRecord {
            id: "report".to_owned(),
            name: "Report".to_owned(),
            description: None,
            entrypoint: "template.typ".to_owned(),
            created_at: Utc::now(),
        };
        let entrypoint = workspace.path().join("main.typ");
        std::fs::write(
            &entrypoint,
            wrap_with_template(&body, &template, &template_dir, &RenderOptions::default()),
        )
        .expect("entrypoint");

        let pdf = compile_project(
            &RenderConfig {
                fonts_dir: workspace.path().join("fonts"),
                packages_dir: workspace.path().join("packages"),
                timeout_secs: 30,
            },
            workspace.path(),
            &entrypoint,
        )
        .expect("pdf");

        assert!(!pdf.is_empty());
    }

    #[test]
    fn compiles_all_markdown_fixture_to_pdf() {
        let markdown = include_str!("../../data/examples/all-markdown-syntax.md");
        let body = markdown_to_typst(markdown, &BTreeMap::new(), &RenderOptions::default());
        let workspace = tempdir().expect("workspace");
        let template_dir = workspace.path().join("templates").join("report");
        std::fs::create_dir_all(&template_dir).expect("template dir");
        std::fs::write(
            template_dir.join("template.typ"),
            "#let template(content, show_toc: true) = [#content]",
        )
        .expect("template");

        std::fs::write(
            workspace.path().join("diagram.svg"),
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="24" viewBox="0 0 64 24">
  <rect width="64" height="24" fill="#f3f4f6"/>
  <circle cx="12" cy="12" r="6" fill="#2563eb"/>
  <rect x="24" y="7" width="28" height="10" rx="2" fill="#111827"/>
</svg>"##,
        )
        .expect("image");

        let template = TemplateRecord {
            id: "report".to_owned(),
            name: "Report".to_owned(),
            description: None,
            entrypoint: "template.typ".to_owned(),
            created_at: Utc::now(),
        };
        let entrypoint = workspace.path().join("main.typ");
        std::fs::write(
            &entrypoint,
            wrap_with_template(&body, &template, &template_dir, &RenderOptions::default()),
        )
        .expect("entrypoint");

        let pdf = compile_project(
            &RenderConfig {
                fonts_dir: workspace.path().join("fonts"),
                packages_dir: workspace.path().join("packages"),
                timeout_secs: 30,
            },
            workspace.path(),
            &entrypoint,
        )
        .expect("pdf");

        assert!(!pdf.is_empty());
    }
}
