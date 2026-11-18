use axum::{
    body::Bytes,
    extract::{rejection::JsonRejection, Path, State},
    http::{header::CONTENT_TYPE, HeaderMap, StatusCode},
    Json,
};
use chrono::Utc;
use serde::Deserialize;
use uuid::Uuid;

use crate::domain::icon;
use crate::domain::models::AdminApp;
use crate::domain::validate;
use crate::error::{ApiError, ApiResult};
use crate::http::identity::TrustedUser;
use crate::http::json_body;
use crate::state::AppState;
use crate::store::{AppGroupRow, AppRow, UserAppRow};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DepartmentGrants {
    #[serde(default)]
    pub department_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewApp {
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default = "default_sort_order")]
    pub sort_order: i32,
    #[serde(default = "default_true")]
    pub is_enabled: bool,
    #[serde(default = "default_true")]
    pub offline_visible: bool,
}

fn default_sort_order() -> i32 {
    100
}
fn default_true() -> bool {
    true
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppPatch {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default, deserialize_with = "crate::domain::de::double_option")]
    pub description: Option<Option<String>>,
    #[serde(default, deserialize_with = "crate::domain::de::double_option")]
    pub category: Option<Option<String>>,
    #[serde(default, deserialize_with = "crate::domain::de::double_option")]
    pub color: Option<Option<String>>,
    #[serde(default)]
    pub sort_order: Option<i32>,
    #[serde(default)]
    pub is_enabled: Option<bool>,
    #[serde(default)]
    pub offline_visible: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GroupGrants {
    #[serde(default)]
    pub group_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UserGrants {
    #[serde(default)]
    pub user_ids: Vec<Uuid>,
}

pub async fn list(
    State(state): State<AppState>,
    TrustedUser(_actor): TrustedUser,
) -> ApiResult<Json<Vec<AdminApp>>> {
    let data = state.store.read();
    let mut apps: Vec<&AppRow> = data
        .apps
        .iter()
        .filter(|app| app.deleted_at.is_none())
        .collect();
    apps.sort_by(|left, right| {
        left.sort_order
            .cmp(&right.sort_order)
            .then_with(|| left.name.cmp(&right.name))
    });
    Ok(Json(
        apps.into_iter()
            .map(|app| AdminApp::from_row(app, &data))
            .collect(),
    ))
}

pub async fn get(
    State(state): State<AppState>,
    TrustedUser(_actor): TrustedUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<AdminApp>> {
    let data = state.store.read();
    let app = data
        .live_app_by_id(id)
        .ok_or_else(|| ApiError::not_found("app_not_found", "That app no longer exists."))?;
    Ok(Json(AdminApp::from_row(app, &data)))
}

pub async fn create(
    State(state): State<AppState>,
    TrustedUser(actor): TrustedUser,
    body: Result<Json<NewApp>, JsonRejection>,
) -> ApiResult<(StatusCode, Json<AdminApp>)> {
    let input = json_body(body)?;

    let name = validate::validate_name(&input.name)?;
    let url = validate::validate_url(&input.url)?;
    let description = validate::validate_description(input.description)?;
    let category = validate::validate_category(input.category)?;
    let color = match validate::clean(input.color) {
        Some(color) => Some(validate::validate_color(&color)?),
        None => None,
    };
    let sort_order = validate::validate_sort_order(input.sort_order)?;

    let now = Utc::now();
    let id = Uuid::new_v4();
    let row = AppRow {
        id,
        name,
        url,
        description,
        category,
        color,
        sort_order,
        is_enabled: input.is_enabled,
        offline_visible: input.offline_visible,
        icon_source: "none".to_string(),
        icon_mime: None,
        icon_fetched_at: None,
        icon_etag: None,
        created_at: now,
        updated_at: now,
        deleted_at: None,
    };

    let result = state.store.mutate(|data| {
        data.apps.push(row);
        data.bump_revision();
        let app = data.live_app_by_id(id).expect("just inserted");
        let view = AdminApp::from_row(app, data);
        data.audit(
            Some(actor.id),
            Some(actor.email.clone()),
            "app.create",
            "app",
            Some(id),
            None,
            Some(serde_json::to_value(&view).unwrap_or_default()),
        );
        Ok(view)
    })?;

    Ok((StatusCode::CREATED, Json(result)))
}

pub async fn update(
    State(state): State<AppState>,
    TrustedUser(actor): TrustedUser,
    Path(id): Path<Uuid>,
    body: Result<Json<AppPatch>, JsonRejection>,
) -> ApiResult<Json<AdminApp>> {
    let patch = json_body(body)?;

    state.store.mutate(|data| {
        let before = data
            .live_app_by_id(id)
            .ok_or_else(|| ApiError::not_found("app_not_found", "That app no longer exists."))?
            .clone();

        let name = match patch.name {
            Some(value) => validate::validate_name(&value)?,
            None => before.name.clone(),
        };
        let url = match patch.url {
            Some(value) => validate::validate_url(&value)?,
            None => before.url.clone(),
        };
        let description = match patch.description {
            Some(value) => validate::validate_description(value)?,
            None => before.description.clone(),
        };
        let category = match patch.category {
            Some(value) => validate::validate_category(value)?,
            None => before.category.clone(),
        };
        let color = match patch.color {
            Some(value) => match validate::clean(value) {
                Some(color) => Some(validate::validate_color(&color)?),
                None => None,
            },
            None => before.color.clone(),
        };
        let sort_order = match patch.sort_order {
            Some(value) => validate::validate_sort_order(value)?,
            None => before.sort_order,
        };

        let before_view = AdminApp::from_row(&before, data);
        {
            let app = data
                .apps
                .iter_mut()
                .find(|app| app.id == id)
                .expect("exists");
            app.name = name;
            app.url = url;
            app.description = description;
            app.category = category;
            app.color = color;
            app.sort_order = sort_order;
            app.is_enabled = patch.is_enabled.unwrap_or(app.is_enabled);
            app.offline_visible = patch.offline_visible.unwrap_or(app.offline_visible);
            app.updated_at = Utc::now();
        }

        data.bump_revision();
        let after_view = AdminApp::from_row(data.live_app_by_id(id).expect("exists"), data);
        data.audit(
            Some(actor.id),
            Some(actor.email.clone()),
            "app.update",
            "app",
            Some(id),
            Some(serde_json::to_value(&before_view).unwrap_or_default()),
            Some(serde_json::to_value(&after_view).unwrap_or_default()),
        );
        Ok(Json(after_view))
    })
}

pub async fn delete(
    State(state): State<AppState>,
    TrustedUser(actor): TrustedUser,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    state.store.mutate(|data| {
        let before = data
            .live_app_by_id(id)
            .ok_or_else(|| ApiError::not_found("app_not_found", "That app no longer exists."))?
            .clone();
        let now = Utc::now();
        let app = data
            .apps
            .iter_mut()
            .find(|app| app.id == id)
            .expect("exists");
        app.deleted_at = Some(now);
        app.is_enabled = false;
        app.updated_at = now;

        data.bump_revision();
        data.audit(
            Some(actor.id),
            Some(actor.email.clone()),
            "app.delete",
            "app",
            Some(id),
            Some(serde_json::to_value(AdminApp::from_row(&before, data)).unwrap_or_default()),
            None,
        );
        Ok(())
    })?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn set_departments(
    State(state): State<AppState>,
    TrustedUser(actor): TrustedUser,
    Path(id): Path<Uuid>,
    body: Result<Json<DepartmentGrants>, JsonRejection>,
) -> ApiResult<Json<AdminApp>> {
    let request = json_body(body)?;
    let department_ids = dedupe(request.department_ids);

    state.store.mutate(|data| {
        if data.live_app_by_id(id).is_none() {
            return Err(ApiError::not_found(
                "app_not_found",
                "That app no longer exists.",
            ));
        }
        if !department_ids
            .iter()
            .all(|department_id| data.department_by_id(*department_id).is_some())
        {
            return Err(ApiError::bad_request(
                "unknown_department",
                "One of those departments no longer exists.",
            ));
        }

        let before = data.department_ids_for_app(id);
        data.app_departments.retain(|grant| grant.app_id != id);
        data.app_departments
            .extend(
                department_ids
                    .iter()
                    .map(|department_id| crate::store::AppDepartmentRow {
                        app_id: id,
                        department_id: *department_id,
                    }),
            );

        data.bump_revision();
        let view = AdminApp::from_row(data.live_app_by_id(id).expect("exists"), data);
        data.audit(
            Some(actor.id),
            Some(actor.email.clone()),
            "app.departments",
            "app",
            Some(id),
            Some(serde_json::json!({ "departmentIds": before })),
            Some(serde_json::json!({ "departmentIds": department_ids })),
        );
        Ok(Json(view))
    })
}

pub async fn set_groups(
    State(state): State<AppState>,
    TrustedUser(actor): TrustedUser,
    Path(id): Path<Uuid>,
    body: Result<Json<GroupGrants>, JsonRejection>,
) -> ApiResult<Json<AdminApp>> {
    let request = json_body(body)?;
    let group_ids = dedupe(request.group_ids);

    state.store.mutate(|data| {
        if data.live_app_by_id(id).is_none() {
            return Err(ApiError::not_found(
                "app_not_found",
                "That app no longer exists.",
            ));
        }
        if !group_ids
            .iter()
            .all(|group_id| data.group_by_id(*group_id).is_some())
        {
            return Err(ApiError::bad_request(
                "unknown_group",
                "One of those groups no longer exists.",
            ));
        }

        let before = data.group_ids_for_app(id);
        data.app_groups.retain(|grant| grant.app_id != id);
        data.app_groups
            .extend(group_ids.iter().map(|group_id| AppGroupRow {
                app_id: id,
                group_id: *group_id,
            }));

        data.bump_revision();
        let view = AdminApp::from_row(data.live_app_by_id(id).expect("exists"), data);
        data.audit(
            Some(actor.id),
            Some(actor.email.clone()),
            "app.groups",
            "app",
            Some(id),
            Some(serde_json::json!({ "groupIds": before })),
            Some(serde_json::json!({ "groupIds": group_ids })),
        );
        Ok(Json(view))
    })
}

pub async fn set_users(
    State(state): State<AppState>,
    TrustedUser(actor): TrustedUser,
    Path(id): Path<Uuid>,
    body: Result<Json<UserGrants>, JsonRejection>,
) -> ApiResult<Json<AdminApp>> {
    let request = json_body(body)?;
    let user_ids = dedupe(request.user_ids);

    state.store.mutate(|data| {
        if data.live_app_by_id(id).is_none() {
            return Err(ApiError::not_found(
                "app_not_found",
                "That app no longer exists.",
            ));
        }
        if !user_ids
            .iter()
            .all(|user_id| data.user_by_id(*user_id).is_some())
        {
            return Err(ApiError::bad_request(
                "unknown_user",
                "One of those users no longer exists.",
            ));
        }

        let before = data.user_ids_for_app(id);
        data.user_apps.retain(|grant| grant.app_id != id);
        let now = Utc::now();
        data.user_apps
            .extend(user_ids.iter().map(|user_id| UserAppRow {
                user_id: *user_id,
                app_id: id,
                created_at: now,
            }));

        data.bump_revision();
        let view = AdminApp::from_row(data.live_app_by_id(id).expect("exists"), data);
        data.audit(
            Some(actor.id),
            Some(actor.email.clone()),
            "app.users",
            "app",
            Some(id),
            Some(serde_json::json!({ "userIds": before })),
            Some(serde_json::json!({ "userIds": user_ids })),
        );
        Ok(Json(view))
    })
}

pub async fn upload_icon(
    State(state): State<AppState>,
    TrustedUser(actor): TrustedUser,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    bytes: Bytes,
) -> ApiResult<Json<AdminApp>> {
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
    let mime = icon::detect_mime(&bytes).ok_or_else(|| {
        ApiError::bad_request(
            "invalid_icon",
            "That file is not a PNG, JPEG, ICO, GIF, WebP or SVG image.",
        )
    })?;

    store_icon(&state, &actor, id, bytes.to_vec(), mime, "upload")
}

pub async fn refetch_icon(
    State(state): State<AppState>,
    TrustedUser(actor): TrustedUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<AdminApp>> {
    let url = {
        let data = state.store.read();
        data.live_app_by_id(id)
            .ok_or_else(|| ApiError::not_found("app_not_found", "That app no longer exists."))?
            .url
            .clone()
    };

    let fetched = icon::fetch_favicon(&url).await.map_err(|message| {
        ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "icon_fetch_failed",
            message,
        )
    })?;

    store_icon(&state, &actor, id, fetched.bytes, fetched.mime, "favicon")
}

fn store_icon(
    state: &AppState,
    actor: &crate::domain::models::SessionUser,
    id: Uuid,
    bytes: Vec<u8>,
    mime: &str,
    source: &str,
) -> ApiResult<Json<AdminApp>> {
    if bytes.is_empty() {
        return Err(ApiError::bad_request(
            "invalid_icon",
            "The icon file is empty.",
        ));
    }
    if bytes.len() > icon::MAX_ICON_BYTES {
        return Err(ApiError::bad_request(
            "icon_too_large",
            "Icons must be 512 KB or smaller.",
        ));
    }

    let etag = icon::etag_for(&bytes);
    state.store.write_icon(id, &bytes)?;

    state.store.mutate(|data| {
        if data.live_app_by_id(id).is_none() {
            return Err(ApiError::not_found(
                "app_not_found",
                "That app no longer exists.",
            ));
        }
        let before = AdminApp::from_row(data.live_app_by_id(id).expect("exists"), data);
        {
            let app = data
                .apps
                .iter_mut()
                .find(|app| app.id == id)
                .expect("exists");
            app.icon_mime = Some(mime.to_string());
            app.icon_source = source.to_string();
            app.icon_fetched_at = Some(Utc::now());
            app.icon_etag = Some(etag.clone());
            app.updated_at = Utc::now();
        }
        data.bump_revision();
        let after = AdminApp::from_row(data.live_app_by_id(id).expect("exists"), data);
        data.audit(
            Some(actor.id),
            Some(actor.email.clone()),
            "app.icon",
            "app",
            Some(id),
            Some(serde_json::json!({
                "iconSource": before.icon_source,
                "iconEtag": before.icon_etag,
            })),
            Some(serde_json::json!({
                "iconSource": after.icon_source,
                "iconMime": after.icon_mime,
                "iconEtag": after.icon_etag,
                "bytes": bytes.len(),
            })),
        );
        Ok(Json(after))
    })
}

fn dedupe(values: Vec<Uuid>) -> Vec<Uuid> {
    let mut unique = Vec::with_capacity(values.len());
    for value in values {
        if !unique.contains(&value) {
            unique.push(value);
        }
    }
    unique
}
