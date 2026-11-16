pub mod auth;
pub mod extract;
pub mod health;

use axum::{
    extract::rejection::JsonRejection,
    routing::{get, post},
    Json, Router,
};
use serde::de::DeserializeOwned;

use crate::error::ApiError;
use crate::state::AppState;

/// All JSON API routes. Static assets are merged separately in `build_router`.
pub fn router() -> Router<AppState> {
    Router::new()
        .merge(health::router())
        .route("/api/v1/auth/login", post(auth::login))
        .route("/api/v1/auth/logout", post(auth::logout))
        .route("/api/v1/auth/me", get(auth::me))
        .route(
            "/api/v1/auth/change-password",
            post(auth::change_password),
        )
        .route("/api/v1/auth/forgot", post(auth::forgot))
        .route("/api/v1/auth/reset", post(auth::reset))
}

/// Turns axum's JSON extractor failures into the same error envelope as everything else.
pub(crate) fn json_body<T: DeserializeOwned>(
    body: Result<Json<T>, JsonRejection>,
) -> Result<T, ApiError> {
    match body {
        Ok(Json(value)) => Ok(value),
        Err(rejection) => Err(ApiError::bad_request("invalid_request", rejection.body_text())),
    }
}
