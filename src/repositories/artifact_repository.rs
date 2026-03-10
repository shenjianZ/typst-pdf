use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use tokio::fs;
use tracing::info;

use crate::utils::AppResult;

#[async_trait]
pub trait ArtifactRepository: Send + Sync {
    async fn ensure_layout(&self) -> AppResult<()>;
    async fn persist_pdf(&self, job_id: &str, source: &Path) -> AppResult<PathBuf>;
    async fn read_bytes(&self, path: &Path) -> AppResult<Vec<u8>>;
}

pub type DynArtifactRepository = Arc<dyn ArtifactRepository>;

#[derive(Debug)]
pub struct LocalArtifactRepository {
    artifacts_dir: PathBuf,
}

impl LocalArtifactRepository {
    pub fn new(artifacts_dir: PathBuf) -> Self {
        Self { artifacts_dir }
    }
}

#[async_trait]
impl ArtifactRepository for LocalArtifactRepository {
    async fn ensure_layout(&self) -> AppResult<()> {
        fs::create_dir_all(&self.artifacts_dir).await?;
        Ok(())
    }

    async fn persist_pdf(&self, job_id: &str, source: &Path) -> AppResult<PathBuf> {
        let destination = self.artifacts_dir.join(format!("{job_id}.pdf"));
        fs::copy(source, &destination).await?;
        info!("stored artifact at {}", destination.display());
        Ok(destination)
    }

    async fn read_bytes(&self, path: &Path) -> AppResult<Vec<u8>> {
        Ok(fs::read(path).await?)
    }
}
