//! App Directory API library.
//!
//! All state lives in CSV files under `data/`; there is no database, no password, no session
//! and no onboarding code. Exposes the modules so integration tests can drive the real router
//! against a temporary CSV directory, while `main.rs` only wires configuration and the server.

pub mod config;
pub mod domain;
pub mod error;
pub mod http;
pub mod state;
pub mod static_assets;
pub mod store;

use axum::Router;
use tower_http::trace::TraceLayer;

use crate::state::AppState;

/// Builds the full router: JSON API plus the admin SPA with its history fallback.
pub fn build_router(state: AppState) -> Router {
    let admin = static_assets::router(&state.config);

    Router::new()
        .merge(http::router())
        .with_state(state)
        .merge(admin)
        .layer(TraceLayer::new_for_http())
}
