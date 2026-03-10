use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use base64::Engine;
use chrono::Utc;
use tokio::fs;

use crate::models::{TemplateCreateRequest, TemplateRecord};
use crate::utils::AppResult;

#[async_trait]
pub trait TemplateRepository: Send + Sync {
    async fn ensure_layout(&self) -> AppResult<()>;
    async fn save_template(&self, request: &TemplateCreateRequest) -> AppResult<TemplateRecord>;
    async fn list_templates(&self) -> AppResult<Vec<TemplateRecord>>;
    fn templates_dir(&self) -> &Path;
}

pub type DynTemplateRepository = Arc<dyn TemplateRepository>;

#[derive(Debug)]
pub struct LocalTemplateRepository {
    templates_dir: PathBuf,
}

impl LocalTemplateRepository {
    pub fn new(templates_dir: PathBuf) -> Self {
        Self { templates_dir }
    }
}

#[async_trait]
impl TemplateRepository for LocalTemplateRepository {
    async fn ensure_layout(&self) -> AppResult<()> {
        fs::create_dir_all(&self.templates_dir).await?;
        Ok(())
    }

    async fn save_template(&self, request: &TemplateCreateRequest) -> AppResult<TemplateRecord> {
        let template_dir = self.templates_dir.join(&request.id);
        fs::create_dir_all(&template_dir).await?;

        let entrypoint = request
            .entrypoint
            .clone()
            .unwrap_or_else(|| "template.typ".to_owned());
        write_file(&template_dir.join(&entrypoint), request.source.as_bytes()).await?;
        write_assets(&template_dir, &request.assets).await?;

        let record = TemplateRecord {
            id: request.id.clone(),
            name: request.name.clone(),
            description: request.description.clone(),
            entrypoint,
            created_at: Utc::now(),
        };

        let metadata = serde_json::to_vec_pretty(&record)?;
        write_file(&template_dir.join("template.json"), &metadata).await?;
        Ok(record)
    }

    async fn list_templates(&self) -> AppResult<Vec<TemplateRecord>> {
        let mut templates = Vec::new();
        if !fs::try_exists(&self.templates_dir).await? {
            return Ok(templates);
        }

        let mut dir = fs::read_dir(&self.templates_dir).await?;
        while let Some(entry) = dir.next_entry().await? {
            let metadata_path = entry.path().join("template.json");
            if fs::try_exists(&metadata_path).await? {
                let bytes = fs::read(metadata_path).await?;
                templates.push(serde_json::from_slice(&bytes)?);
            }
        }
        templates.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(templates)
    }

    fn templates_dir(&self) -> &Path {
        &self.templates_dir
    }
}

async fn write_file(path: &Path, content: &[u8]) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).await?;
    }
    fs::write(path, content).await?;
    Ok(())
}

async fn write_assets(job_dir: &Path, assets: &[crate::models::AssetPayload]) -> AppResult<()> {
    for asset in assets {
        let path = job_dir.join(&asset.path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(asset.content_base64.as_bytes())
            .map_err(|err| {
                crate::utils::AppError::BadRequest(format!("invalid base64 asset: {err}"))
            })?;
        fs::write(path, bytes).await?;
    }
    Ok(())
}
