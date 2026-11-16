use std::net::IpAddr;
use std::time::Duration;

use axum::{
    extract::rejection::JsonRejection,
    extract::State,
    http::{header, HeaderMap, StatusCode},
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::audit;
use crate::domain::auth::{self, AuthError, SessionUser};
use crate::domain::settings;
use crate::error::{ApiError, ApiResult};
use crate::http::extract::{AuthUser, ClientIp};
use crate::http::json_body;
use crate::state::AppState;

const RATE_WINDOW: Duration = Duration::from_secs(60);
const LOGIN_ATTEMPTS_PER_MINUTE: usize = 20;
const FORGOT_REQUESTS_PER_MINUTE: usize = 10;
const RESET_ATTEMPTS_PER_MINUTE: usize = 20;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginResponse {
    pub token: String,
    pub expires_at: DateTime<Utc>,
    pub user: SessionUser,
    pub must_change_password: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Deserialize)]
pub struct ForgotRequest {
    pub email: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetRequest {
    pub email: String,
    pub code: String,
    pub new_password: String,
}

fn pool_of(state: &AppState) -> ApiResult<&sqlx::PgPool> {
    state
        .pool
        .as_ref()
        .ok_or_else(|| ApiError::internal("Database is not configured."))
}

fn ip_key(ip: Option<IpAddr>) -> String {
    ip.map_or_else(|| "unknown".to_string(), |address| address.to_string())
}

fn rate_limited(state: &AppState, scope: &str, ip: Option<IpAddr>, limit: usize) -> bool {
    !state
        .auth_limiter
        .allow(&format!("{scope}:{}", ip_key(ip)), limit, RATE_WINDOW)
}

pub async fn login(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    headers: HeaderMap,
    body: Result<Json<LoginRequest>, JsonRejection>,
) -> ApiResult<Json<LoginResponse>> {
    let request = json_body(body)?;

    if rate_limited(&state, "login", ip, LOGIN_ATTEMPTS_PER_MINUTE) {
        return Err(ApiError::too_many_requests(
            "Too many sign-in attempts. Wait a minute and try again.",
        ));
    }

    let pool = pool_of(&state)?;
    let ip_text = ip.map(|address| address.to_string());

    let user_agent: String = headers
        .get(header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .chars()
        .take(200)
        .collect();
    let client_info = (!user_agent.is_empty()).then_some(user_agent.as_str());

    let policy = settings::lockout_policy(pool).await;

    let (token, expires_at, user) = match auth::login(
        pool,
        &request.email,
        &request.password,
        state.config.session_ttl_days,
        policy,
        client_info,
    )
    .await
    {
        Ok(result) => result,
        Err(error) => {
            // Failed attempts go to the log, not the audit table: a brute-force run would
            // otherwise fill the table the admins rely on.
            if matches!(error, AuthError::InvalidCredentials) {
                tracing::warn!(email = %request.email, ip = %ip_key(ip), "failed sign-in attempt");
            }
            return Err(error.into());
        }
    };

    audit::record(
        pool,
        Some(user.id),
        "auth.login",
        "session",
        None,
        None,
        None,
        ip_text.as_deref(),
    )
    .await;

    let must_change_password = user.must_change_password;

    Ok(Json(LoginResponse {
        token,
        expires_at,
        user,
        must_change_password,
    }))
}

pub async fn me(AuthUser(session): AuthUser) -> Json<SessionUser> {
    Json(session.user)
}

pub async fn logout(
    State(state): State<AppState>,
    AuthUser(session): AuthUser,
    ClientIp(ip): ClientIp,
) -> ApiResult<StatusCode> {
    let pool = pool_of(&state)?;
    let ip_text = ip.map(|address| address.to_string());

    auth::revoke_session(pool, session.session_id).await?;

    audit::record(
        pool,
        Some(session.user.id),
        "auth.logout",
        "session",
        Some(session.session_id),
        None,
        None,
        ip_text.as_deref(),
    )
    .await;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn change_password(
    State(state): State<AppState>,
    AuthUser(session): AuthUser,
    ClientIp(ip): ClientIp,
    body: Result<Json<ChangePasswordRequest>, JsonRejection>,
) -> ApiResult<StatusCode> {
    let request = json_body(body)?;
    let pool = pool_of(&state)?;
    let ip_text = ip.map(|address| address.to_string());

    let min_length = settings::password_min_length(pool).await;

    auth::change_password(
        pool,
        session.user.id,
        &request.current_password,
        &request.new_password,
        min_length,
        Some(session.session_id),
    )
    .await?;

    audit::record(
        pool,
        Some(session.user.id),
        "auth.password_changed",
        "user",
        Some(session.user.id),
        None,
        None,
        ip_text.as_deref(),
    )
    .await;

    Ok(StatusCode::NO_CONTENT)
}

/// Always answers `202`, whether or not the address belongs to an account.
pub async fn forgot(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    body: Result<Json<ForgotRequest>, JsonRejection>,
) -> ApiResult<StatusCode> {
    let request = json_body(body)?;

    if rate_limited(&state, "forgot", ip, FORGOT_REQUESTS_PER_MINUTE) {
        return Err(ApiError::too_many_requests(
            "Too many reset requests. Wait a minute and try again.",
        ));
    }

    let pool = pool_of(&state)?;
    let ip_text = ip.map(|address| address.to_string());

    match auth::queue_reset_request(pool, &request.email).await {
        Ok(Some(request_id)) => {
            audit::record(
                pool,
                None,
                "auth.reset_requested",
                "reset_request",
                Some(request_id),
                None,
                None,
                ip_text.as_deref(),
            )
            .await;
        }
        Ok(None) => {}
        Err(AuthError::Internal(message)) => {
            tracing::error!(error = %message, "failed to queue reset request");
        }
        Err(other) => {
            tracing::warn!(error = ?other, "unexpected error queueing reset request");
        }
    }

    Ok(StatusCode::ACCEPTED)
}

pub async fn reset(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    body: Result<Json<ResetRequest>, JsonRejection>,
) -> ApiResult<StatusCode> {
    let request = json_body(body)?;

    if rate_limited(&state, "reset", ip, RESET_ATTEMPTS_PER_MINUTE) {
        return Err(ApiError::too_many_requests(
            "Too many reset attempts. Wait a minute and try again.",
        ));
    }

    let pool = pool_of(&state)?;
    let ip_text = ip.map(|address| address.to_string());
    let min_length = settings::password_min_length(pool).await;

    let (user_id, _email) = auth::reset_password(
        pool,
        &request.email,
        &request.code,
        &request.new_password,
        min_length,
    )
    .await?;

    audit::record(
        pool,
        Some(user_id),
        "auth.password_reset",
        "user",
        Some(user_id),
        None,
        None,
        ip_text.as_deref(),
    )
    .await;

    Ok(StatusCode::NO_CONTENT)
}
