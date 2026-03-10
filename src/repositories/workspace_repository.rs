use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use base64::Engine;
use tokio::fs;

use crate::models::AssetPayload;
use crate::utils::{AppError, AppResult};

#[async_trait]
pub trait WorkspaceRepository: Send + Sync {
    async fn ensure_layout(&self) -> AppResult<()>;
    async fn write_assets(&self, job_dir: &Path, assets: &[AssetPayload]) -> AppResult<()>;
    fn uploads_dir(&self) -> &Path;
}

pub type DynWorkspaceRepository = Arc<dyn WorkspaceRepository>;

#[derive(Debug)]
pub struct LocalWorkspaceRepository {
    uploads_dir: PathBuf,
}

impl LocalWorkspaceRepository {
    pub fn new(uploads_dir: PathBuf) -> Self {
        Self { uploads_dir }
    }
}

#[async_trait]
impl WorkspaceRepository for LocalWorkspaceRepository {
    async fn ensure_layout(&self) -> AppResult<()> {
        fs::create_dir_all(&self.uploads_dir).await?;
        Ok(())
    }

    async fn write_assets(&self, job_dir: &Path, assets: &[AssetPayload]) -> AppResult<()> {
        for asset in assets {
            let path = job_dir.join(&asset.path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).await?;
            }
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(asset.content_base64.as_bytes())
                .map_err(|err| AppError::BadRequest(format!("invalid base64 asset: {err}")))?;
            fs::write(path, bytes).await?;
        }
        Ok(())
    }

    fn uploads_dir(&self) -> &Path {
        &self.uploads_dir
    }
}
