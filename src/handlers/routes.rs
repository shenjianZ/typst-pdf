use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderValue, header};
use axum::middleware;
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};

use crate::handlers::middleware::auth_middleware;
use crate::infra::AppState;
use crate::models::{JobResponse, RenderRequest, TemplateCreateRequest};
use crate::utils::AppResult;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .route("/v1/render/pdf", post(render_pdf))
        .route("/v1/jobs", post(create_job))
        .route("/v1/jobs/{job_id}", get(get_job))
        .route("/v1/jobs/{job_id}/artifact", get(download_job_artifact))
        .route("/v1/templates", get(list_templates).post(create_template))
        .layer(middleware::from_fn_with_state(
            Arc::clone(&state),
            auth_middleware,
        ))
        .with_state(state)
}

async fn healthz() -> &'static str {
    "ok"
}

async fn readyz() -> &'static str {
    "ready"
}

async fn render_pdf(
    State(state): State<Arc<AppState>>,
    Json(request): Json<RenderRequest>,
) -> AppResult<Response> {
    let bytes = state.render_service.render_now(request).await?;
    Ok(pdf_response(bytes))
}

async fn create_job(
    State(state): State<Arc<AppState>>,
    Json(request): Json<RenderRequest>,
) -> AppResult<Json<JobResponse>> {
    Ok(Json(state.render_service.enqueue(request).await?))
}

async fn get_job(
    State(state): State<Arc<AppState>>,
    Path(job_id): Path<uuid::Uuid>,
) -> AppResult<Json<JobResponse>> {
    Ok(Json(state.render_service.get_job(job_id).await?))
}

async fn download_job_artifact(
    State(state): State<Arc<AppState>>,
    Path(job_id): Path<uuid::Uuid>,
) -> AppResult<Response> {
    let bytes = state.render_service.get_job_artifact(job_id).await?;
    Ok(pdf_response(bytes))
}

async fn create_template(
    State(state): State<Arc<AppState>>,
    Json(request): Json<TemplateCreateRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let record = state.render_service.create_template(request).await?;
    Ok(Json(serde_json::json!({ "template": record })))
}

async fn list_templates(State(state): State<Arc<AppState>>) -> AppResult<Json<serde_json::Value>> {
    Ok(Json(serde_json::json!({
        "templates": state.render_service.list_templates().await?
    })))
}

fn pdf_response(bytes: Vec<u8>) -> Response {
    let mut response = Response::new(Body::from(bytes));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/pdf"),
    );
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("inline; filename=\"document.pdf\""),
    );
    response
}
