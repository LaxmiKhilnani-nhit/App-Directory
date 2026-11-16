use axum::{extract::State, routing::get, Json, Router};
use serde::Serialize;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

use app_directory_api::config::Config;
use app_directory_api::state::AppState;
use app_directory_api::{db, domain, static_assets};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Health {
    status: &'static str,
    version: &'static str,
    app_name: String,
    database: &'static str,
}

async fn health(State(state): State<AppState>) -> Json<Health> {
    let database = match state.pool.as_ref() {
        None => "not_configured",
        Some(pool) => match sqlx::query_scalar::<_, i32>("select 1").fetch_one(pool).await {
            Ok(_) => "ok",
            Err(error) => {
                tracing::error!(%error, "health check query failed");
                "error"
            }
        },
    };

    Json(Health {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
        app_name: state.config.app_name.clone(),
        database,
    })
}

fn app(state: AppState) -> Router {
    let admin = static_assets::router(&state.config);

    Router::new()
        .route("/api/v1/healthz", get(health))
        .with_state(state)
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
        "starting App Directory API"
    );

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app(state))
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    if let Err(error) = tokio::signal::ctrl_c().await {
        tracing::error!(%error, "failed to listen for shutdown signal");
    }
}
