use axum::{
    body::Bytes,
    extract::{rejection::JsonRejection, Path, State},
    http::{header::CONTENT_TYPE, HeaderMap, StatusCode},
    Json,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::domain::apps::{self, AdminApp, AppPatch, NewApp};
use crate::domain::icon_fetch;
use crate::error::{ApiError, ApiResult};
use crate::http::admin::{pool_of, AdminUser, Scope};
use crate::http::extract::ClientIp;
use crate::http::json_body;
use crate::state::AppState;

pub async fn list(State(state): State<AppState>, AdminUser(session): AdminUser) -> ApiResult<Json<Vec<AdminApp>>> {
    let pool = pool_of(&state)?;
    let apps = apps::list(pool).await?;

    Ok(Json(Scope::new(&session.user).filter(apps)))
}

pub async fn get(
    State(state): State<AppState>,
    AdminUser(session): AdminUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<AdminApp>> {
    let pool = pool_of(&state)?;
    let app = apps::get(pool, id)
        .await?
        .ok_or_else(|| ApiError::not_found("app_not_found", "That app no longer exists."))?;

    Scope::new(&session.user).require_app(&app)?;

    Ok(Json(app))
}

pub async fn create(
    State(state): State<AppState>,
    AdminUser(session): AdminUser,
    ClientIp(ip): ClientIp,
    body: Result<Json<NewApp>, JsonRejection>,
) -> ApiResult<(StatusCode, Json<AdminApp>)> {
    let input = json_body(body)?;
    let pool = pool_of(&state)?;

    Scope::new(&session.user).require_category(input.category.as_deref())?;

    let app = apps::create(pool, session.user.id, input, ip.map(|a| a.to_string()).as_deref()).await?;

    Ok((StatusCode::CREATED, Json(app)))
}

pub async fn update(
    State(state): State<AppState>,
    AdminUser(session): AdminUser,
    ClientIp(ip): ClientIp,
    Path(id): Path<Uuid>,
    body: Result<Json<AppPatch>, JsonRejection>,
) -> ApiResult<Json<AdminApp>> {
    let patch = json_body(body)?;
    let pool = pool_of(&state)?;

    let existing = apps::get(pool, id)
        .await?
        .ok_or_else(|| ApiError::not_found("app_not_found", "That app no longer exists."))?;

    let scope = Scope::new(&session.user);
    scope.require_app(&existing)?;
    if let Some(category) = patch.category.as_ref() {
        scope.require_category(category.as_deref())?;
    }

    let app = apps::update(
        pool,
        session.user.id,
        id,
        patch,
        ip.map(|a| a.to_string()).as_deref(),
    )
    .await?;

    Ok(Json(app))
}

pub async fn delete(
    State(state): State<AppState>,
    AdminUser(session): AdminUser,
    ClientIp(ip): ClientIp,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    let pool = pool_of(&state)?;

    let existing = apps::get(pool, id)
        .await?
        .ok_or_else(|| ApiError::not_found("app_not_found", "That app no longer exists."))?;

    Scope::new(&session.user).require_app(&existing)?;

    apps::delete(
        pool,
        session.user.id,
        id,
        ip.map(|a| a.to_string()).as_deref(),
    )
    .await?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn upload_icon(
    State(state): State<AppState>,
    AdminUser(session): AdminUser,
    ClientIp(ip): ClientIp,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    bytes: Bytes,
) -> ApiResult<Json<AdminApp>> {
    let pool = pool_of(&state)?;

    let existing = apps::get(pool, id)
        .await?
        .ok_or_else(|| ApiError::not_found("app_not_found", "That app no longer exists."))?;

    Scope::new(&session.user).require_app(&existing)?;

    let declared = headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");

    if !declared.is_empty() && !declared.starts_with("image/") {
        return Err(ApiError::new(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "unsupported_media_type",
            "Send the icon as an image.",
        ));
    }

    let mime = icon_fetch::detect_mime(&bytes).ok_or_else(|| {
        ApiError::bad_request(
            "invalid_icon",
            "That file is not a PNG, JPEG, ICO, GIF, WebP or SVG image.",
        )
    })?;

    let app = apps::set_icon(
        pool,
        session.user.id,
        id,
        bytes.to_vec(),
        mime,
        "upload",
        ip.map(|a| a.to_string()).as_deref(),
    )
    .await?;

    Ok(Json(app))
}

pub async fn refetch_icon(
    State(state): State<AppState>,
    AdminUser(session): AdminUser,
    ClientIp(ip): ClientIp,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<AdminApp>> {
    let pool = pool_of(&state)?;

    let existing = apps::get(pool, id)
        .await?
        .ok_or_else(|| ApiError::not_found("app_not_found", "That app no longer exists."))?;

    Scope::new(&session.user).require_app(&existing)?;

    let fetched = icon_fetch::fetch_favicon(&existing.url).await.map_err(|message| {
        ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "icon_fetch_failed",
            message,
        )
    })?;

    let app = apps::set_icon(
        pool,
        session.user.id,
        id,
        fetched.bytes,
        fetched.mime,
        "favicon",
        ip.map(|a| a.to_string()).as_deref(),
    )
    .await?;

    Ok(Json(app))
}

/// Replaces every group grant on the app; unknown fields are rejected rather than ignored
/// so a malformed body cannot quietly drop grants (PLAN.md §5).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GroupGrants {
    #[serde(default)]
    pub group_ids: Vec<Uuid>,
}

pub async fn set_groups(
    State(state): State<AppState>,
    AdminUser(session): AdminUser,
    ClientIp(ip): ClientIp,
    Path(id): Path<Uuid>,
    body: Result<Json<GroupGrants>, JsonRejection>,
) -> ApiResult<Json<AdminApp>> {
    let request = json_body(body)?;
    let pool = pool_of(&state)?;

    let existing = apps::get(pool, id)
        .await?
        .ok_or_else(|| ApiError::not_found("app_not_found", "That app no longer exists."))?;

    Scope::new(&session.user).require_app(&existing)?;

    let app = apps::set_groups(
        pool,
        session.user.id,
        id,
        &request.group_ids,
        ip.map(|a| a.to_string()).as_deref(),
    )
    .await?;

    Ok(Json(app))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserGrants {
    #[serde(default)]
    pub user_ids: Vec<Uuid>,
}

pub async fn set_users(
    State(state): State<AppState>,
    AdminUser(session): AdminUser,
    ClientIp(ip): ClientIp,
    Path(id): Path<Uuid>,
    body: Result<Json<UserGrants>, JsonRejection>,
) -> ApiResult<Json<AdminApp>> {
    let request = json_body(body)?;
    let pool = pool_of(&state)?;

    let existing = apps::get(pool, id)
        .await?
        .ok_or_else(|| ApiError::not_found("app_not_found", "That app no longer exists."))?;

    Scope::new(&session.user).require_app(&existing)?;

    let app = apps::set_users(
        pool,
        session.user.id,
        id,
        &request.user_ids,
        ip.map(|a| a.to_string()).as_deref(),
    )
    .await?;

    Ok(Json(app))
}
