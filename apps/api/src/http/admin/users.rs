use axum::{
    extract::{rejection::JsonRejection, Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::domain::users::{
    self, AdminUser as AdminUserRecord, ImportReport, IssuedCode, NewUser, UserDetail, UserPage,
    UserPatch,
};
use crate::error::{ApiError, ApiResult};
use crate::http::admin::{pool_of, SuperAdmin};
use crate::http::extract::ClientIp;
use crate::http::json_body;
use crate::state::AppState;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserQuery {
    pub query: Option<String>,
    pub role: Option<String>,
    pub active: Option<bool>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub async fn list(
    State(state): State<AppState>,
    SuperAdmin(_session): SuperAdmin,
    Query(params): Query<UserQuery>,
) -> ApiResult<Json<UserPage>> {
    let pool = pool_of(&state)?;

    if let Some(role) = params.role.as_deref() {
        if !users::is_role(role) {
            return Err(ApiError::bad_request(
                "invalid_role",
                "Role must be user, app_admin or super_admin.",
            ));
        }
    }

    let page = users::list(
        pool,
        params.query.as_deref(),
        params.role.as_deref(),
        params.active,
        params.limit.unwrap_or(100),
        params.offset.unwrap_or(0),
    )
    .await?;

    Ok(Json(page))
}

pub async fn get(
    State(state): State<AppState>,
    SuperAdmin(_session): SuperAdmin,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<UserDetail>> {
    let pool = pool_of(&state)?;

    users::get(pool, id)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("user_not_found", "That user no longer exists."))
}

pub async fn create(
    State(state): State<AppState>,
    SuperAdmin(session): SuperAdmin,
    ClientIp(ip): ClientIp,
    body: Result<Json<NewUser>, JsonRejection>,
) -> ApiResult<(StatusCode, Json<AdminUserRecord>)> {
    let input = json_body(body)?;
    let pool = pool_of(&state)?;

    let user = users::create(
        pool,
        session.user.id,
        input,
        ip.map(|a| a.to_string()).as_deref(),
    )
    .await?;

    Ok((StatusCode::CREATED, Json(user)))
}

pub async fn update(
    State(state): State<AppState>,
    SuperAdmin(session): SuperAdmin,
    ClientIp(ip): ClientIp,
    Path(id): Path<Uuid>,
    body: Result<Json<UserPatch>, JsonRejection>,
) -> ApiResult<Json<AdminUserRecord>> {
    let patch = json_body(body)?;
    let pool = pool_of(&state)?;

    let user = users::update(
        pool,
        session.user.id,
        id,
        patch,
        ip.map(|a| a.to_string()).as_deref(),
    )
    .await?;

    Ok(Json(user))
}

pub async fn delete(
    State(state): State<AppState>,
    SuperAdmin(session): SuperAdmin,
    ClientIp(ip): ClientIp,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    let pool = pool_of(&state)?;

    users::deactivate(
        pool,
        session.user.id,
        id,
        ip.map(|a| a.to_string()).as_deref(),
    )
    .await?;

    Ok(StatusCode::NO_CONTENT)
}

/// Replaces every app granted directly to the user; see `GroupGrants` in `apps.rs`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppGrants {
    #[serde(default)]
    pub app_ids: Vec<Uuid>,
}

pub async fn set_apps(
    State(state): State<AppState>,
    SuperAdmin(session): SuperAdmin,
    ClientIp(ip): ClientIp,
    Path(id): Path<Uuid>,
    body: Result<Json<AppGrants>, JsonRejection>,
) -> ApiResult<Json<UserDetail>> {
    let request = json_body(body)?;
    let pool = pool_of(&state)?;

    let detail = users::set_apps(
        pool,
        session.user.id,
        id,
        &request.app_ids,
        ip.map(|a| a.to_string()).as_deref(),
    )
    .await?;

    Ok(Json(detail))
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetCodeRequest {
    pub purpose: Option<String>,
}

pub async fn reset_code(
    State(state): State<AppState>,
    SuperAdmin(session): SuperAdmin,
    ClientIp(ip): ClientIp,
    Path(id): Path<Uuid>,
    body: Option<Json<ResetCodeRequest>>,
) -> ApiResult<Json<IssuedCode>> {
    let purpose = body.and_then(|Json(request)| request.purpose);
    let pool = pool_of(&state)?;

    let code = users::issue_reset_code(
        pool,
        session.user.id,
        id,
        purpose.as_deref(),
        ip.map(|a| a.to_string()).as_deref(),
    )
    .await?;

    Ok(Json(code))
}

#[derive(Debug, Deserialize)]
pub struct ImportRequest {
    pub csv: String,
}

pub async fn import(
    State(state): State<AppState>,
    SuperAdmin(session): SuperAdmin,
    ClientIp(ip): ClientIp,
    body: Result<Json<ImportRequest>, JsonRejection>,
) -> ApiResult<Json<ImportReport>> {
    let request = json_body(body)?;
    let pool = pool_of(&state)?;

    let report = users::import(
        pool,
        session.user.id,
        &request.csv,
        ip.map(|a| a.to_string()).as_deref(),
    )
    .await?;

    Ok(Json(report))
}
