use std::env;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub auth: AuthConfig,
    pub storage: StorageConfig,
    pub render: RenderConfig,
    pub jobs: JobConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub bind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub api_keys: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub root_dir: PathBuf,
    pub artifacts_dir: PathBuf,
    pub uploads_dir: PathBuf,
    pub templates_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderConfig {
    pub fonts_dir: PathBuf,
    pub packages_dir: PathBuf,
    pub timeout_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobConfig {
    pub worker_concurrency: usize,
    pub artifact_ttl_hours: u64,
}

impl AppConfig {
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let root_dir = env_path("APP_STORAGE_ROOT", "./data");
        let artifacts_dir = env_path("APP_ARTIFACTS_DIR", root_dir.join("artifacts"));
        let uploads_dir = env_path("APP_UPLOADS_DIR", root_dir.join("uploads"));
        let templates_dir = env_path("APP_TEMPLATES_DIR", root_dir.join("templates"));
        let fonts_dir = env_path("APP_FONTS_DIR", "./assets/fonts");
        let packages_dir = env_path("APP_PACKAGES_DIR", "./assets/packages");

        let api_keys = env::var("APP_API_KEYS")
            .unwrap_or_else(|_| "dev-secret".to_owned())
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();

        Ok(Self {
            server: ServerConfig {
                bind: env::var("APP_BIND").unwrap_or_else(|_| "0.0.0.0:3000".to_owned()),
            },
            auth: AuthConfig { api_keys },
            storage: StorageConfig {
                root_dir,
                artifacts_dir,
                uploads_dir,
                templates_dir,
            },
            render: RenderConfig {
                fonts_dir,
                packages_dir,
                timeout_secs: env_u64("APP_RENDER_TIMEOUT_SECS", 60),
            },
            jobs: JobConfig {
                worker_concurrency: env_usize("APP_JOB_WORKERS", 2),
                artifact_ttl_hours: env_u64("APP_ARTIFACT_TTL_HOURS", 24),
            },
        })
    }
}

fn env_path(key: &str, default: impl Into<PathBuf>) -> PathBuf {
    env::var(key)
        .map(PathBuf::from)
        .unwrap_or_else(|_| default.into())
}

fn env_u64(key: &str, default: u64) -> u64 {
    env::var(key)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(default)
}

fn env_usize(key: &str, default: usize) -> usize {
    env::var(key)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::AppConfig;

    #[test]
    fn loads_defaults() {
        let config = AppConfig::load().expect("config");
        assert_eq!(config.server.bind, "0.0.0.0:3000");
        assert!(!config.auth.api_keys.is_empty());
    }
}
