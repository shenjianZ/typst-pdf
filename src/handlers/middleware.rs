use std::sync::Arc;

use axum::extract::State;
use axum::http::HeaderMap;
use axum::middleware::Next;
use axum::response::IntoResponse;

use crate::infra::AppState;
use crate::utils::AppError;

pub async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    request: axum::extract::Request,
    next: Next,
) -> Result<impl IntoResponse, AppError> {
    let path = request.uri().path();
    if path == "/healthz" || path == "/readyz" {
        return Ok(next.run(request).await);
    }

    let Some(value) = headers.get("x-api-key") else {
        return Err(AppError::Unauthorized);
    };
    let token = value.to_str().map_err(|_| AppError::Unauthorized)?.trim();

    if state
        .config
        .auth
        .api_keys
        .iter()
        .any(|candidate| candidate == token)
    {
        Ok(next.run(request).await)
    } else {
        Err(AppError::Unauthorized)
    }
}
