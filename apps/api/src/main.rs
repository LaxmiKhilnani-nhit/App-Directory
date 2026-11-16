use std::net::SocketAddr;

use tracing_subscriber::EnvFilter;

use app_directory_api::config::Config;
use app_directory_api::state::AppState;
use app_directory_api::{build_router, db, domain};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_env("RUST_LOG").unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let config = Config::from_env();
    let pool = db::connect_and_migrate(&config).await?;

    if let Some(pool) = pool.as_ref() {
        domain::seed::run(pool, &config).await?;
    }

    let state = AppState::new(config, pool);
    let addr = format!("{}:{}", state.config.bind_addr, state.config.port);

    tracing::info!(
        bind = %addr,
        app = %state.config.app_name,
        admin_dist = %state.config.admin_dist.display(),
        session_ttl_days = state.config.session_ttl_days,
        "starting App Directory API"
    );

    let listener = tokio::net::TcpListener::bind(&addr).await?;

    // Connect info is what gives handlers the client IP for rate limiting and audit rows.
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
