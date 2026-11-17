//! App Directory API library.
//!
//! Exposes the modules so integration tests can exercise the domain logic and the HTTP
//! layer directly against a real database, while `main.rs` only wires configuration and
//! the server.

pub mod config;
pub mod db;
pub mod domain;
pub mod error;
pub mod http;
pub mod rate_limit;
pub mod state;
pub mod static_assets;

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
