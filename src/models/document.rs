use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceType {
    Markdown,
    Typst,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetPayload {
    pub path: String,
    pub content_base64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RenderOptions {
    pub page_size: Option<String>,
    pub margin: Option<String>,
    pub language: Option<String>,
    pub font_family: Option<String>,
    pub show_toc: Option<bool>,
    pub theme: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderRequest {
    pub source_type: SourceType,
    pub source: String,
    #[serde(default)]
    pub entrypoint: Option<String>,
    #[serde(default)]
    pub assets: Vec<AssetPayload>,
    #[serde(default)]
    pub template_id: Option<String>,
    #[serde(default)]
    pub variables: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    pub render_options: RenderOptions,
}
