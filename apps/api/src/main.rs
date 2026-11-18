use std::{net::SocketAddr, sync::Arc};

use tracing_subscriber::EnvFilter;

use app_directory_api::config::Config;
use app_directory_api::state::AppState;
use app_directory_api::store::Store;
use app_directory_api::build_router;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_env("RUST_LOG").unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let config = Config::from_env();
    let store = Arc::new(Store::open(&config.data_dir)?);
    let state = AppState::new(config, store);
    let addr = format!("{}:{}", state.config.bind_addr, state.config.port);

    tracing::info!(
        bind = %addr,
        app = %state.config.app_name,
        admin_dist = %state.config.admin_dist.display(),
        data_dir = %state.config.data_dir.display(),
        "starting App Directory API (CSV store, trusted Windows identity)"
    );

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(
        listener,
        build_router(state).into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;

    Ok(())
}

async fn shutdown_signal() {
    if let Err(error) = tokio::signal::ctrl_c().await {
        tracing::error!(%error, "failed to listen for shutdown signal");
    }
}
