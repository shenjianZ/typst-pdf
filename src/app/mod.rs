use std::sync::Arc;

use axum::Router;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::handlers;
use crate::infra::AppState;

pub fn build_router(state: Arc<AppState>) -> Router {
    handlers::router(state)
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_headers(Any)
                .allow_methods(Any),
        )
        .layer(TraceLayer::new_for_http())
}
