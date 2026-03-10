use std::collections::BTreeSet;
use std::hash::{Hash, Hasher};
use std::io::Read;
use std::path::{Path, PathBuf};

use base64::Engine;
use pulldown_cmark::{Event, Options, Parser, Tag};
use tokio::fs;

use crate::models::AssetPayload;
use crate::utils::{AppError, AppResult};

#[derive(Debug, Default)]
pub struct LocalMarkdownAssets {
    pub assets: Vec<AssetPayload>,
    pub replacements: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
struct MarkdownRef {
    destination: String,
    kind: MarkdownRefKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MarkdownRefKind {
    Image,
    Link,
}

pub async fn collect_local_markdown_assets(
    markdown: &str,
    base_dir: &Path,
) -> AppResult<LocalMarkdownAssets> {
    let refs = markdown_refs(markdown);
    if refs.is_empty() {
        return Ok(LocalMarkdownAssets::default());
    }

    let base_dir = fs::canonicalize(base_dir)
        .await
        .map_err(|err| AppError::Internal(format!("failed to resolve markdown base dir: {err}")))?;
    let mut seen_destinations = BTreeSet::new();
    let mut seen_asset_paths = BTreeSet::new();
    let mut assets = Vec::new();
    let mut replacements = Vec::new();

    for reference in refs {
        if !seen_destinations.insert(reference.destination.clone()) {
            continue;
        }
        let Some((path_part, suffix)) = split_destination_suffix(&reference.destination) else {
            continue;
        };

        if is_remote_ref(path_part) {
            if reference.kind == MarkdownRefKind::Image {
                if let Some((asset_path, asset)) =
                    collect_remote_image_asset(path_part, suffix, &mut seen_asset_paths).await?
                {
                    replacements.push((
                        reference.destination.clone(),
                        format!("{asset_path}{suffix}"),
                    ));
                    if let Some(asset) = asset {
                        assets.push(asset);
                    }
                }
            }
            continue;
        }

        if !is_local_ref(path_part) {
            continue;
        }

        let Some(resolved_path) = resolve_local_ref(path_part, &base_dir) else {
            continue;
        };
        let canonical = match fs::canonicalize(&resolved_path).await {
            Ok(path) => path,
            Err(_) => continue,
        };
        if !canonical.starts_with(&base_dir) {
            continue;
        }

        let metadata = match fs::metadata(&canonical).await {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };
        if !metadata.is_file() {
            continue;
        }

        let relative = canonical.strip_prefix(&base_dir).map_err(|_| {
            AppError::Internal("failed to compute markdown local asset path".to_owned())
        })?;
        let asset_path = format!("__local_assets/{}", normalize_relative_path(relative));
        replacements.push((
            reference.destination.clone(),
            format!("{asset_path}{suffix}"),
        ));

        if seen_asset_paths.insert(asset_path.clone()) {
            let bytes = fs::read(&canonical).await?;
            assets.push(AssetPayload {
                path: asset_path,
                content_base64: base64::engine::general_purpose::STANDARD.encode(bytes),
            });
        }
    }

    Ok(LocalMarkdownAssets {
        assets,
        replacements,
    })
}

pub fn apply_typst_asset_replacements(source: &str, replacements: &[(String, String)]) -> String {
    replacements.iter().fold(source.to_owned(), |acc, (from, to)| {
        acc.replace(
            &format!("\"{}\"", escape_typst_string(from)),
            &format!("\"{}\"", escape_typst_string(to)),
        )
    })
}

async fn collect_remote_image_asset(
    path_part: &str,
    _suffix: &str,
    seen_asset_paths: &mut BTreeSet<String>,
) -> AppResult<Option<(String, Option<AssetPayload>)>> {
    let url = path_part.to_owned();
    let asset_path = remote_asset_path(&url);
    if !seen_asset_paths.insert(asset_path.clone()) {
        return Ok(Some((asset_path, None)));
    }

    let fetched = tokio::task::spawn_blocking(move || fetch_remote_asset(&url))
        .await
        .map_err(|err| AppError::Internal(format!("remote asset task failed: {err}")))??;

    let Some((bytes, _content_type)) = fetched else {
        return Ok(None);
    };

    let payload = AssetPayload {
        path: asset_path.clone(),
        content_base64: base64::engine::general_purpose::STANDARD.encode(bytes),
    };

    Ok(Some((asset_path, Some(payload))))
}

fn fetch_remote_asset(url: &str) -> AppResult<Option<(Vec<u8>, Option<String>)>> {
    let response = match ureq::get(url).call() {
        Ok(response) => response,
        Err(ureq::Error::Status(_, _)) => return Ok(None),
        Err(err) => {
            return Err(AppError::Render(format!(
                "failed to download remote image {url}: {err}"
            )))
        }
    };

    let content_type = response.header("Content-Type").map(str::to_owned);
    let mut reader = response.into_reader();
    let mut bytes = Vec::new();
    reader
        .read_to_end(&mut bytes)
        .map_err(|err| AppError::Render(format!("failed to read remote image {url}: {err}")))?;
    Ok(Some((bytes, content_type)))
}

fn markdown_refs(markdown: &str) -> Vec<MarkdownRef> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_MATH);
    options.insert(Options::ENABLE_SUPERSCRIPT);
    options.insert(Options::ENABLE_SUBSCRIPT);
    options.insert(Options::ENABLE_SMART_PUNCTUATION);

    let parser = Parser::new_ext(markdown, options);
    let mut refs = Vec::new();

    for event in parser {
        match event {
            Event::Start(Tag::Link { dest_url, .. }) => refs.push(MarkdownRef {
                destination: dest_url.to_string(),
                kind: MarkdownRefKind::Link,
            }),
            Event::Start(Tag::Image { dest_url, .. }) => refs.push(MarkdownRef {
                destination: dest_url.to_string(),
                kind: MarkdownRefKind::Image,
            }),
            _ => {}
        }
    }

    refs
}

fn split_destination_suffix(destination: &str) -> Option<(&str, &str)> {
    if destination.is_empty() {
        return None;
    }
    let split_at = destination.find(['?', '#']).unwrap_or(destination.len());
    Some(destination.split_at(split_at))
}

fn is_remote_ref(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

fn is_local_ref(path: &str) -> bool {
    if path.is_empty() || path.starts_with('#') {
        return false;
    }
    let lower = path.to_ascii_lowercase();
    !matches!(
        (),
        _ if lower.starts_with("mailto:")
            || lower.starts_with("tel:")
            || lower.starts_with("data:")
            || lower.starts_with("ftp://")
    )
}

fn resolve_local_ref(path: &str, base_dir: &Path) -> Option<PathBuf> {
    if path.starts_with('/') || path.starts_with('\\') {
        let trimmed = path.trim_start_matches(['/', '\\']);
        Some(base_dir.join(trimmed))
    } else {
        let ref_path = Path::new(path);
        if ref_path.is_absolute() {
            Some(ref_path.to_path_buf())
        } else {
            Some(base_dir.join(ref_path))
        }
    }
}

fn normalize_relative_path(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

fn remote_asset_path(url: &str) -> String {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    let filename = Path::new(path)
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("remote-image");
    let sanitized = sanitize_filename(filename);
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    url.hash(&mut hasher);
    format!("__remote_assets/{:016x}-{}", hasher.finish(), sanitized)
}

fn sanitize_filename(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '.' || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect()
}

fn escape_typst_string(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('#', "\\#")
        .replace('$', "\\$")
        .replace('[', "\\[")
        .replace(']', "\\]")
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    use tempfile::tempdir;

    use super::{apply_typst_asset_replacements, collect_local_markdown_assets};

    #[tokio::test]
    async fn collects_relative_and_rooted_markdown_assets() {
        let dir = tempdir().expect("tempdir");
        std::fs::create_dir_all(dir.path().join("images")).expect("images");
        std::fs::write(dir.path().join("images").join("logo.png"), b"png").expect("image");
        std::fs::write(dir.path().join("guide.pdf"), b"pdf").expect("pdf");

        let markdown = "![logo](/images/logo.png)\n[guide](guide.pdf#intro)\n";
        let resolved = collect_local_markdown_assets(markdown, dir.path())
            .await
            .expect("resolved");

        assert_eq!(resolved.assets.len(), 2);
        assert!(resolved
            .replacements
            .iter()
            .any(|(from, to)| from == "/images/logo.png" && to == "__local_assets/images/logo.png"));
        assert!(resolved
            .replacements
            .iter()
            .any(|(from, to)| from == "guide.pdf#intro"
                && to == "__local_assets/guide.pdf#intro"));
    }

    #[tokio::test]
    async fn downloads_remote_images() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let addr = listener.local_addr().expect("addr");
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut request = [0_u8; 1024];
            let _ = stream.read(&mut request);
            let body = b"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"10\" height=\"10\"></svg>";
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: image/svg+xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            stream.write_all(response.as_bytes()).expect("headers");
            stream.write_all(body).expect("body");
        });

        let markdown = format!("![remote](http://{addr}/demo.svg)");
        let dir = tempdir().expect("tempdir");
        let resolved = collect_local_markdown_assets(&markdown, dir.path())
            .await
            .expect("resolved");

        handle.join().expect("server");
        assert_eq!(resolved.assets.len(), 1);
        assert!(resolved.assets[0].path.starts_with("__remote_assets/"));
        assert!(resolved
            .replacements
            .iter()
            .any(|(from, to)| from.starts_with("http://") && to.starts_with("__remote_assets/")));
    }

    #[test]
    fn rewrites_typst_string_destinations() {
        let input = "#link(\"docs/readme.md\")[docs]\n#md_figure(\"images/a.png\")";
        let output = apply_typst_asset_replacements(
            input,
            &[
                ("docs/readme.md".to_owned(), "__local_assets/docs/readme.md".to_owned()),
                ("images/a.png".to_owned(), "__local_assets/images/a.png".to_owned()),
            ],
        );

        assert!(output.contains("\"__local_assets/docs/readme.md\""));
        assert!(output.contains("\"__local_assets/images/a.png\""));
    }
}
