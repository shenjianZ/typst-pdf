use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("unauthorized")]
    Unauthorized,
    #[error("not found: {0}")]
    NotFound(String),
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("render failed: {0}")]
    Render(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("internal error: {0}")]
    Internal(String),
}

#[derive(Debug, Serialize)]
struct ErrorBody {
    error: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match &self {
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::Render(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::Io(_) | Self::Serde(_) | Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        let body = axum::Json(ErrorBody {
            error: self.to_string(),
        });

        (status, body).into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;
