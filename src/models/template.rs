use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::models::AssetPayload;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateCreateRequest {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub entrypoint: Option<String>,
    pub source: String,
    #[serde(default)]
    pub assets: Vec<AssetPayload>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateRecord {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub entrypoint: String,
    pub created_at: DateTime<Utc>,
}
