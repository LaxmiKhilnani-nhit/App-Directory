//! Admin authentication. Credentials live in `admins.csv`, separate from `users.csv`, and
//! passwords are verified with Argon2id. Successful login issues a signed, expiring token.

use axum::{
    extract::{rejection::JsonRejection, State},
    http::StatusCode,
    Json,
};
use chrono::{Duration, Utc};
use serde::Deserialize;
use uuid::Uuid;

use crate::domain::admins;
use crate::domain::models::{LoginResponse, SessionUser};
use crate::error::{ApiError, ApiResult};
use crate::http::identity::TrustedUser;
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

pub async fn login(
    State(state): State<AppState>,
    body: Result<Json<LoginRequest>, JsonRejection>,
) -> ApiResult<Json<LoginResponse>> {
    let request = json_body(body)?;
    let email = request.email.trim().to_lowercase();

    let admin = admins::find(&state, &email).ok_or_else(|| {
        ApiError::new(
            StatusCode::UNAUTHORIZED,
            "invalid_credentials",
            "Email or password is incorrect.",
        )
    })?;

    if !admins::verify_password(&admin, &request.password) {
        return Err(ApiError::new(
            StatusCode::UNAUTHORIZED,
            "invalid_credentials",
            "Email or password is incorrect.",
        ));
    }

    let now = Utc::now();
    let token = admins::issue_token(&state, &admin.email);

    let session = match crate::http::identity::find_user(&state, &admin.email) {
        Some(user) => SessionUser {
            id: user.id,
            email: user.email,
            name: admin.name.clone(),
            department: user.department,
            role: "super_admin".to_string(),
            admin_scope: None,
            must_change_password: false,
        },
        None => SessionUser {
            id: Uuid::new_v5(&Uuid::NAMESPACE_DNS, format!("admin:{}", admin.email).as_bytes()),
            email: admin.email.clone(),
            name: admin.name.clone(),
            department: None,
            role: "super_admin".to_string(),
            admin_scope: None,
            must_change_password: false,
        },
    };

    let actor_id = session.id;
    let actor_email = session.email.clone();
    let _ = state.store.mutate(|data| {
        data.audit(
            Some(actor_id),
            Some(actor_email.clone()),
            "auth.login",
            "admin",
            None,
            None,
            None,
        );
        Ok(())
    });

    Ok(Json(LoginResponse {
        token,
        expires_at: now + Duration::days(30),
        user: session,
        must_change_password: false,
    }))
}

pub async fn me(TrustedUser(user): TrustedUser) -> Json<SessionUser> {
    Json(user)
}

/// Tokens are stateless; there is nothing to revoke server-side.
pub async fn logout() -> StatusCode {
    StatusCode::NO_CONTENT
}
