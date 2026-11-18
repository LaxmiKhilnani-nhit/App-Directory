use axum::{extract::Path, http::StatusCode, Json};
use serde::Deserialize;
use uuid::Uuid;

use crate::domain::models::ResetRequest;
use crate::error::{ApiError, ApiResult};
use crate::http::identity::TrustedUser;

#[derive(Debug, Default, Deserialize)]
pub struct ResetQuery {
    pub status: Option<String>,
}

/// Reset requests and onboarding codes are deliberately gone: identity is trusted from
/// Windows. The queue is always empty and resolving a request is a no-op, which keeps the
/// admin portal's reset-requests screen from breaking.
pub async fn list(
    TrustedUser(_actor): TrustedUser,
    axum::extract::Query(_params): axum::extract::Query<ResetQuery>,
) -> Json<Vec<ResetRequest>> {
    Json(Vec::new())
}

pub async fn issue(
    TrustedUser(_actor): TrustedUser,
    Path(_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    Err(ApiError::not_found(
        "reset_request_not_found",
        "Reset requests are disabled; identity is trusted from Windows.",
    ))
}

pub async fn dismiss(
    TrustedUser(_actor): TrustedUser,
    Path(_id): Path<Uuid>,
) -> StatusCode {
    StatusCode::NO_CONTENT
}
