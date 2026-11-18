use axum::{extract::State, routing::get, Json, Router};
use serde::Serialize;

use crate::state::AppState;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Health {
    status: &'static str,
    version: &'static str,
    app_name: String,
    database: &'static str,
}

pub fn router() -> Router<AppState> {
    Router::new().route("/api/v1/healthz", get(health))
}

async fn health(State(state): State<AppState>) -> Json<Health> {
    Json(Health {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
        app_name: state.config.app_name.clone(),
        database: "csv",
    })
}
