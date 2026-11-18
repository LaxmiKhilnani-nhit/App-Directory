//! Compatibility auth endpoints.
//!
//! Identity is trusted from `X-Windows-Identity`; no password, session, reset code or SMTP
//! machinery remains. These routes exist so the unmodified admin SPA and desktop client
//! keep working: login returns a dummy `"csv"` token and the other endpoints are no-ops.

use axum::{
    extract::{rejection::JsonRejection, Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use chrono::{Duration, Utc};
use serde::Deserialize;

use crate::domain::models::{LoginResponse, SessionUser};
use crate::error::{ApiError, ApiResult};
use crate::http::identity::{self, TrustedUser};
use crate::http::json_body;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginRequest {
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct IdentityQuery {
    pub identity: Option<String>,
}

pub async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<IdentityQuery>,
    body: Result<Json<LoginRequest>, JsonRejection>,
) -> ApiResult<Json<LoginResponse>> {
    // Some clients send the Windows identity in the login form's email field; tolerate a
    // completely empty/optional body too so the SPA can sign in with just a proxy header.
    let email = match body {
        Ok(Json(request)) => request.email,
        Err(_) => String::new(),
    };

    let header_identity = headers
        .get("x-windows-identity")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    let raw = header_identity
        .or_else(|| params.identity.clone())
        .or_else(|| {
            if email.trim().is_empty() {
                None
            } else {
                Some(email.clone())
            }
        })
        .ok_or_else(|| ApiError::unauthorized("A Windows identity is required."))?;

    let user = identity::find_user(&state, &raw).ok_or_else(|| {
        ApiError::new(
            StatusCode::UNAUTHORIZED,
            "unknown_user",
            "This Windows account is not registered in the App Directory.",
        )
    })?;

    let now = Utc::now();
    let session = SessionUser::from(&user);
    let user_id = user.id;

    let _ = state.store.mutate(|data| {
        if let Some(row) = data.users.iter_mut().find(|row| row.id == user_id) {
            row.last_login_at = Some(now);
        }
        data.audit(
            Some(user_id),
            Some(user.email.clone()),
            "auth.login",
            "session",
            None,
            None,
            None,
        );
        Ok(())
    });

    // The `Authorization` header is ignored everywhere; this token is intentionally fixed.
    Ok(Json(LoginResponse {
        token: "csv".to_string(),
        expires_at: now + Duration::days(365 * 10),
        user: session.clone(),
        must_change_password: false,
    }))
}

pub async fn me(TrustedUser(user): TrustedUser) -> Json<SessionUser> {
    Json(user)
}

/// No sessions exist, so there is nothing to revoke. The SPA just needs a 2xx.
pub async fn logout() -> StatusCode {
    StatusCode::NO_CONTENT
}

/// Passwords are not stored or checked any more. These endpoints stay as no-ops so the
/// change-password and forgot-password screens do not break.
pub async fn change_password(
    body: Result<Json<serde_json::Value>, JsonRejection>,
) -> StatusCode {
    let _ = json_body(body);
    StatusCode::NO_CONTENT
}

pub async fn forgot(body: Result<Json<serde_json::Value>, JsonRejection>) -> StatusCode {
    let _ = json_body(body);
    StatusCode::ACCEPTED
}

pub async fn reset(body: Result<Json<serde_json::Value>, JsonRejection>) -> StatusCode {
    let _ = json_body(body);
    StatusCode::NO_CONTENT
}
