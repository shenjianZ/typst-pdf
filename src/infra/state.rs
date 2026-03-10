use std::sync::Arc;

use crate::config::AppConfig;
use crate::infra::{DynRenderer, TypstRenderer};
use crate::repositories::{
    DynArtifactRepository, DynTemplateRepository, DynWorkspaceRepository, LocalArtifactRepository,
    LocalTemplateRepository, LocalWorkspaceRepository,
};
use crate::services::RenderService;
use crate::utils::AppResult;

#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub render_service: RenderService,
}

impl AppState {
    pub async fn build(config: AppConfig) -> AppResult<Self> {
        let artifact_repository: DynArtifactRepository = Arc::new(LocalArtifactRepository::new(
            config.storage.artifacts_dir.clone(),
        ));
        let workspace_repository: DynWorkspaceRepository = Arc::new(LocalWorkspaceRepository::new(
            config.storage.uploads_dir.clone(),
        ));
        let template_repository: DynTemplateRepository = Arc::new(LocalTemplateRepository::new(
            config.storage.templates_dir.clone(),
        ));

        artifact_repository.ensure_layout().await?;
        workspace_repository.ensure_layout().await?;
        template_repository.ensure_layout().await?;

        if !tokio::fs::try_exists(&config.storage.templates_dir).await? {
            tokio::fs::create_dir_all(&config.storage.templates_dir).await?;
        }
        seed_builtin_templates(&config.storage.templates_dir).await?;

        let renderer: DynRenderer = Arc::new(TypstRenderer::new(config.render.clone()));
        let render_service = RenderService::new(
            Arc::clone(&artifact_repository),
            Arc::clone(&workspace_repository),
            Arc::clone(&template_repository),
            Arc::clone(&renderer),
            config.jobs.worker_concurrency,
        )
        .await?;

        Ok(Self {
            config,
            render_service,
        })
    }
}

async fn seed_builtin_templates(templates_dir: &std::path::Path) -> AppResult<()> {
    let built_in_dir = std::path::Path::new("./assets/templates");
    if !tokio::fs::try_exists(built_in_dir).await? {
        return Ok(());
    }

    let mut entries = tokio::fs::read_dir(built_in_dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        let source_dir = entry.path();
        let target_dir = templates_dir.join(entry.file_name());
        if tokio::fs::try_exists(&target_dir).await? {
            continue;
        }
        copy_dir_all(&source_dir, &target_dir).await?;
    }
    Ok(())
}

async fn copy_dir_all(source: &std::path::Path, destination: &std::path::Path) -> AppResult<()> {
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
