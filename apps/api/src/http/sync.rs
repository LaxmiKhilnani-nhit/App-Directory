use axum::{
    extract::{Query, State},
    Json,
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::domain::entitlements;
use crate::domain::{revision, settings};
use crate::error::ApiResult;
use crate::http::admin::pool_of;
use crate::http::extract::AuthUser;
use crate::state::AppState;

#[derive(Debug, Default, Deserialize)]
pub struct SyncQuery {
    /// Accepted for forward compatibility; the payload is always a full snapshot.
    #[allow(dead_code)]
    pub cursor: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncUser {
    pub name: String,
    pub department: Option<String>,
    pub must_change_password: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncApp {
    pub id: uuid::Uuid,
    pub name: String,
    pub url: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub color: Option<String>,
    pub sort_order: i32,
    pub offline_visible: bool,
    pub icon_url: Option<String>,
    pub icon_etag: Option<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncPayload {
    pub revision: i64,
    pub server_time: DateTime<Utc>,
    pub offline_valid_until: DateTime<Utc>,
    pub snapshot_days: i64,
    /// How often the client should refresh in the background.
    pub sync_interval_hours: i64,
    pub announcement: Option<Value>,
    pub user: SyncUser,
    pub apps: Vec<SyncApp>,
}

/// `/client/sync` — the only endpoint that decides what a user may see, and it is built on
/// `entitlements::apps_for_user` so the rule exists in exactly one place.
pub async fn sync(
    State(state): State<AppState>,
    AuthUser(session): AuthUser,
    Query(_params): Query<SyncQuery>,
) -> ApiResult<Json<SyncPayload>> {
    let pool = pool_of(&state)?;

    let now = Utc::now();
    let snapshot_days = settings::offline_grace_days(pool).await;
    let sync_interval_hours = settings::sync_interval_hours(pool).await;
    let revision = revision::current(pool).await?;
    let announcement = settings::get::<Value>(pool, settings::ANNOUNCEMENT)
        .await
        .ok()
        .flatten()
        .and_then(active_announcement);

    let apps = entitlements::apps_for_user(pool, session.user.id)
        .await?
        .into_iter()
        .filter(|app| app.offline_visible)
        .map(|app| SyncApp {
            icon_url: app
                .icon_etag
                .as_ref()
                .map(|_| format!("/api/v1/icons/{}", app.id)),
            id: app.id,
            name: app.name,
            url: app.url,
            description: app.description,
            category: app.category,
            color: app.color,
            sort_order: app.sort_order,
            offline_visible: app.offline_visible,
            icon_etag: app.icon_etag,
            updated_at: app.updated_at,
        })
        .collect();

    Ok(Json(SyncPayload {
        revision,
        server_time: now,
        offline_valid_until: now + Duration::days(snapshot_days.max(0)),
        snapshot_days,
        sync_interval_hours,
        announcement,
        user: SyncUser {
            name: session.user.name,
            department: session.user.department,
            must_change_password: session.user.must_change_password,
        },
        apps,
    }))
}

/// Drops an announcement whose active window has passed, or has not opened yet.
fn active_announcement(value: Value) -> Option<Value> {
    if value.is_null() {
        return None;
    }

    let object = value.as_object()?;
    let now = Utc::now();

    if let Some(from) = object.get("activeFrom").and_then(Value::as_str) {
        if let Ok(from) = DateTime::parse_from_rfc3339(from) {
            if now < from.with_timezone(&Utc) {
                return None;
            }
        }
    }

    if let Some(to) = object.get("activeTo").and_then(Value::as_str) {
        if let Ok(to) = DateTime::parse_from_rfc3339(to) {
            if now > to.with_timezone(&Utc) {
                return None;
            }
        }
    }

    Some(value)
}
