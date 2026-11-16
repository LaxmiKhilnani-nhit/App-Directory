mod config;
mod static_assets;

use axum::{extract::State, routing::get, Json, Router};
use serde::Serialize;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

use crate::config::Config;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Health {
    status: &'static str,
    version: &'static str,
    app_name: String,
}

async fn health(State(config): State<Config>) -> Json<Health> {
    Json(Health {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
        app_name: config.app_name,
    })
}

fn app(config: Config) -> Router {
    let admin = static_assets::router(&config);

    Router::new()
        .route("/api/v1/healthz", get(health))
        .with_state(config)
        .merge(admin)
        .layer(TraceLayer::new_for_http())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_env("RUST_LOG").unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let config = Config::from_env();
    let addr = format!("{}:{}", config.bind_addr, config.port);

    tracing::info!(
        bind = %addr,
        app = %config.app_name,
        admin_dist = %config.admin_dist.display(),
        "starting App Directory API"
    );

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app(config))
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    if let Err(error) = tokio::signal::ctrl_c().await {
        tracing::error!(%error, "failed to listen for shutdown signal");
    }
}
