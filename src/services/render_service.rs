use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use chrono::Utc;
use tempfile::tempdir_in;
use tokio::sync::{RwLock, mpsc};
use tracing::error;
use uuid::Uuid;

use crate::infra::DynRenderer;
use crate::models::{
    JobRecord, JobResponse, JobStatus, RenderRequest, TemplateCreateRequest, TemplateRecord,
};
use crate::repositories::{DynArtifactRepository, DynTemplateRepository, DynWorkspaceRepository};
use crate::utils::{AppError, AppResult};

#[derive(Clone)]
pub struct RenderService {
    artifact_repository: DynArtifactRepository,
    workspace_repository: DynWorkspaceRepository,
    template_repository: DynTemplateRepository,
    renderer: DynRenderer,
    templates: Arc<RwLock<HashMap<String, TemplateRecord>>>,
    jobs: Arc<RwLock<HashMap<Uuid, JobRecord>>>,
    queue: mpsc::Sender<Uuid>,
}

impl RenderService {
    pub async fn new(
        artifact_repository: DynArtifactRepository,
        workspace_repository: DynWorkspaceRepository,
        template_repository: DynTemplateRepository,
        renderer: DynRenderer,
        worker_count: usize,
    ) -> AppResult<Self> {
        let templates = template_repository
            .list_templates()
            .await?
            .into_iter()
            .map(|item| (item.id.clone(), item))
            .collect::<HashMap<_, _>>();

        let jobs = Arc::new(RwLock::new(HashMap::new()));
        let templates = Arc::new(RwLock::new(templates));
        let (queue, rx) = mpsc::channel(128);
        let service = Self {
            artifact_repository: Arc::clone(&artifact_repository),
            workspace_repository: Arc::clone(&workspace_repository),
            template_repository: Arc::clone(&template_repository),
            renderer: Arc::clone(&renderer),
            templates: Arc::clone(&templates),
            jobs: Arc::clone(&jobs),
            queue,
        };

        let receiver = Arc::new(tokio::sync::Mutex::new(rx));
        for _ in 0..worker_count.max(1) {
            spawn_worker(
                Arc::clone(&artifact_repository),
                Arc::clone(&workspace_repository),
                Arc::clone(&template_repository),
                Arc::clone(&renderer),
                Arc::clone(&templates),
                Arc::clone(&jobs),
                Arc::clone(&receiver),
            );
        }

        Ok(service)
    }

    pub async fn render_now(&self, request: RenderRequest) -> AppResult<Vec<u8>> {
        let artifact = self.render_request(&request, None).await?;
        self.artifact_repository.read_bytes(&artifact).await
    }

    pub async fn enqueue(&self, request: RenderRequest) -> AppResult<JobResponse> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let record = JobRecord {
            id,
            status: JobStatus::Queued,
            created_at: now,
            updated_at: now,
            artifact_path: None,
            error_message: None,
            request,
        };
        self.jobs.write().await.insert(id, record);
        self.queue
            .send(id)
            .await
            .map_err(|_| AppError::Internal("job queue unavailable".to_owned()))?;
        Ok(JobResponse {
            job_id: id,
            status: JobStatus::Queued,
            artifact_url: None,
            error_message: None,
        })
    }

    pub async fn get_job(&self, id: Uuid) -> AppResult<JobResponse> {
        let jobs = self.jobs.read().await;
        let record = jobs
            .get(&id)
            .ok_or_else(|| AppError::NotFound(format!("job {id}")))?;
        Ok(JobResponse::from_record(record))
    }

    pub async fn get_job_artifact(&self, id: Uuid) -> AppResult<Vec<u8>> {
        let jobs = self.jobs.read().await;
        let record = jobs
            .get(&id)
            .ok_or_else(|| AppError::NotFound(format!("job {id}")))?;
        let artifact_path = record
            .artifact_path
            .as_ref()
            .ok_or_else(|| AppError::BadRequest("artifact not ready".to_owned()))?;
        self.artifact_repository
            .read_bytes(PathBuf::from(artifact_path).as_path())
            .await
    }

    pub async fn create_template(
        &self,
        request: TemplateCreateRequest,
    ) -> AppResult<TemplateRecord> {
        let record = self.template_repository.save_template(&request).await?;
        self.templates
            .write()
            .await
            .insert(record.id.clone(), record.clone());
        Ok(record)
    }

    pub async fn list_templates(&self) -> AppResult<Vec<TemplateRecord>> {
        let templates = self.templates.read().await;
        let mut items = templates.values().cloned().collect::<Vec<_>>();
        items.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(items)
    }

    async fn render_request(
        &self,
        request: &RenderRequest,
        job_id: Option<Uuid>,
    ) -> AppResult<PathBuf> {
        let tempdir = tempdir_in(self.workspace_repository.uploads_dir())
            .map_err(|err| AppError::Internal(format!("failed to create temp dir: {err}")))?;
        let workdir = tempdir.path().to_path_buf();
        self.workspace_repository
            .write_assets(&workdir, &request.assets)
            .await?;

        let template = match &request.template_id {
            Some(id) => self.templates.read().await.get(id).cloned(),
            None => None,
        };

        let project = self
            .renderer
            .materialize(
                request,
                template.as_ref(),
                &workdir,
                self.template_repository.templates_dir(),
            )
            .await?;
        let pdf_path = self.renderer.compile_pdf(&project).await?;

        let id = job_id
            .map(|value| value.to_string())
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let artifact_path = self.artifact_repository.persist_pdf(&id, &pdf_path).await?;
        tempdir
            .close()
            .map_err(|err| AppError::Internal(format!("failed to cleanup temp dir: {err}")))?;
        Ok(artifact_path)
    }
}

fn spawn_worker(
    artifact_repository: DynArtifactRepository,
    workspace_repository: DynWorkspaceRepository,
    template_repository: DynTemplateRepository,
    renderer: DynRenderer,
    templates: Arc<RwLock<HashMap<String, TemplateRecord>>>,
    jobs: Arc<RwLock<HashMap<Uuid, JobRecord>>>,
    receiver: Arc<tokio::sync::Mutex<mpsc::Receiver<Uuid>>>,
) {
    tokio::spawn(async move {
        loop {
            let next_job = {
                let mut guard = receiver.lock().await;
                guard.recv().await
            };

            let Some(job_id) = next_job else {
                break;
            };

            let request = {
                let mut job_map = jobs.write().await;
                let Some(record) = job_map.get_mut(&job_id) else {
                    continue;
                };
                record.status = JobStatus::Running;
                record.updated_at = Utc::now();
                record.request.clone()
            };

            let result = render_job(
                &artifact_repository,
                &workspace_repository,
                &template_repository,
                &renderer,
                &templates,
                &request,
                job_id,
            )
            .await;
            let mut job_map = jobs.write().await;
            if let Some(record) = job_map.get_mut(&job_id) {
                record.updated_at = Utc::now();
                match result {
                    Ok(path) => {
                        record.status = JobStatus::Succeeded;
                        record.artifact_path = Some(path.to_string_lossy().to_string());
                        record.error_message = None;
                    }
                    Err(err) => {
                        error!("job {} failed: {}", job_id, err);
                        record.status = JobStatus::Failed;
                        record.error_message = Some(err.to_string());
                    }
                }
            }
        }
    });
}

async fn render_job(
    artifact_repository: &DynArtifactRepository,
    workspace_repository: &DynWorkspaceRepository,
    template_repository: &DynTemplateRepository,
    renderer: &DynRenderer,
    templates: &Arc<RwLock<HashMap<String, TemplateRecord>>>,
    request: &RenderRequest,
    job_id: Uuid,
) -> AppResult<PathBuf> {
    let tempdir = tempdir_in(workspace_repository.uploads_dir())
        .map_err(|err| AppError::Internal(format!("failed to create temp dir: {err}")))?;
    let workdir = tempdir.path().to_path_buf();
    workspace_repository
        .write_assets(&workdir, &request.assets)
        .await?;

    let template = match &request.template_id {
        Some(id) => templates.read().await.get(id).cloned(),
        None => None,
    };

    let project = renderer
        .materialize(
            request,
            template.as_ref(),
            &workdir,
            template_repository.templates_dir(),
        )
        .await?;
    let pdf = renderer.compile_pdf(&project).await?;
    let artifact = artifact_repository
        .persist_pdf(&job_id.to_string(), &pdf)
        .await?;
    tempdir
        .close()
        .map_err(|err| AppError::Internal(format!("failed to cleanup temp dir: {err}")))?;
    Ok(artifact)
}
