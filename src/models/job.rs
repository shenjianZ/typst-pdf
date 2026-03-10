use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::RenderRequest;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobRecord {
    pub id: Uuid,
    pub status: JobStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub artifact_path: Option<String>,
    pub error_message: Option<String>,
    pub request: RenderRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobResponse {
    pub job_id: Uuid,
    pub status: JobStatus,
    pub artifact_url: Option<String>,
    pub error_message: Option<String>,
}

impl JobResponse {
    pub fn from_record(record: &JobRecord) -> Self {
        Self {
            job_id: record.id,
            status: record.status.clone(),
            artifact_url: record
                .artifact_path
                .as_ref()
                .map(|_| format!("/v1/jobs/{}/artifact", record.id)),
            error_message: record.error_message.clone(),
        }
    }
}
