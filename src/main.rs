use std::net::SocketAddr;
use std::sync::Arc;

use tokio::net::TcpListener;
use tracing::info;

use typst_pdf_service::app::build_router;
use typst_pdf_service::config::AppConfig;
use typst_pdf_service::infra::AppState;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    typst_pdf_service::utils::init_telemetry();

    let config = AppConfig::load()?;
    let state = Arc::new(AppState::build(config.clone()).await?);
    let app = build_router(state);

    let addr: SocketAddr = config.server.bind.parse()?;
    let listener = TcpListener::bind(addr).await?;
    info!("listening on {}", addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut signal) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            signal.recv().await;
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {}
        _ = terminate => {}
    }
}
