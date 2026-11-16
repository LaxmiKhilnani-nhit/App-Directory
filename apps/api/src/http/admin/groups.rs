use axum::{
    extract::{rejection::JsonRejection, Path, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::domain::groups::{self, AdminGroup, GroupDetail, GroupPatch, NewGroup};
use crate::error::{ApiError, ApiResult};
use crate::http::admin::{pool_of, AdminUser, SuperAdmin};
use crate::http::extract::ClientIp;
use crate::http::json_body;
use crate::state::AppState;

pub async fn list(
    State(state): State<AppState>,
    AdminUser(_session): AdminUser,
) -> ApiResult<Json<Vec<AdminGroup>>> {
    let pool = pool_of(&state)?;
    Ok(Json(groups::list(pool).await?))
}

pub async fn get(
    State(state): State<AppState>,
    SuperAdmin(_session): SuperAdmin,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<GroupDetail>> {
    let pool = pool_of(&state)?;
    groups::get(pool, id)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("group_not_found", "That group no longer exists."))
}

pub async fn create(
    State(state): State<AppState>,
    SuperAdmin(session): SuperAdmin,
    ClientIp(ip): ClientIp,
    body: Result<Json<NewGroup>, JsonRejection>,
) -> ApiResult<(StatusCode, Json<AdminGroup>)> {
    let input = json_body(body)?;
    let pool = pool_of(&state)?;

    let group = groups::create(
        pool,
        session.user.id,
        input,
        ip.map(|a| a.to_string()).as_deref(),
    )
    .await?;

    Ok((StatusCode::CREATED, Json(group)))
}

pub async fn update(
    State(state): State<AppState>,
    SuperAdmin(session): SuperAdmin,
    ClientIp(ip): ClientIp,
    Path(id): Path<Uuid>,
    body: Result<Json<GroupPatch>, JsonRejection>,
) -> ApiResult<Json<AdminGroup>> {
    let patch = json_body(body)?;
    let pool = pool_of(&state)?;

    let group = groups::update(
        pool,
        session.user.id,
        id,
        patch,
        ip.map(|a| a.to_string()).as_deref(),
    )
    .await?;

    Ok(Json(group))
}

pub async fn delete(
    State(state): State<AppState>,
    SuperAdmin(session): SuperAdmin,
    ClientIp(ip): ClientIp,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    let pool = pool_of(&state)?;

    groups::delete(
        pool,
        session.user.id,
        id,
        ip.map(|a| a.to_string()).as_deref(),
    )
    .await?;

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MembersRequest {
    #[serde(default)]
    pub user_ids: Vec<Uuid>,
}

pub async fn set_members(
    State(state): State<AppState>,
    SuperAdmin(session): SuperAdmin,
    ClientIp(ip): ClientIp,
    Path(id): Path<Uuid>,
    body: Result<Json<MembersRequest>, JsonRejection>,
) -> ApiResult<Json<GroupDetail>> {
    let request = json_body(body)?;
    let pool = pool_of(&state)?;

    let detail = groups::set_members(
        pool,
        session.user.id,
        id,
        &request.user_ids,
        ip.map(|a| a.to_string()).as_deref(),
    )
    .await?;

    Ok(Json(detail))
}
