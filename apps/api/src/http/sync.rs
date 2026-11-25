use axum::{extract::Query, Json};
use chrono::{DateTime, Duration, Utc};
use serde::Deserialize;
use serde_json::Value;

use crate::domain::entitlements;
use crate::domain::models::{PhoneEntry, SyncApp, SyncPayload, SyncUser};
use crate::error::ApiResult;
use crate::http::identity::KnownUser;
use crate::state::AppState;

#[derive(Debug, Default, Deserialize)]
pub struct SyncQuery {
    /// Accepted for forward compatibility; the payload is always a full snapshot.
    #[allow(dead_code)]
    pub cursor: Option<i64>,
    /// Windows identity, e.g. `ad\laxmikhilnani`. Also accepted as a header.
    pub identity: Option<String>,
}

pub async fn sync(
    state: axum::extract::State<AppState>,
    KnownUser(user): KnownUser,
    Query(_params): Query<SyncQuery>,
) -> ApiResult<Json<SyncPayload>> {
    let now = Utc::now();
    let data = state.store.read();

    let snapshot_days = data.setting_i64("offline_grace_days", 7).max(0);
    let sync_interval_hours = data.setting_i64("sync_interval_hours", 4);
    let announcement = data
        .setting("announcement")
        .filter(|value| !value.is_null())
        .cloned()
        .and_then(active_announcement);

    // The directory is shared by everybody: it contains no per-user entitlements.
    let phone_directory = PhoneEntry::sorted_from(&data);

    let apps = entitlements::apps_for_user(&data, user.id)
        .into_iter()
        .filter(|app| app.offline_visible)
        .map(|app| SyncApp {
            icon_url: if state.store.has_icon(app.id) {
                Some(format!("/api/v1/icons/{}", app.id))
            } else {
                None
            },
            icon_etag: app.icon_etag.clone(),
            id: app.id,
            name: app.name,
            url: app.url,
            description: app.description,
            category: app.category,
            color: app.color,
            sort_order: app.sort_order,
            offline_visible: app.offline_visible,
            updated_at: app.updated_at,
        })
        .collect();

    Ok(Json(SyncPayload {
        revision: data.revision,
        server_time: now,
        offline_valid_until: now + Duration::days(snapshot_days),
        snapshot_days,
        sync_interval_hours,
        announcement,
        user: SyncUser {
            name: user.name,
            department: user.department,
            must_change_password: false,
        },
        apps,
        phone_directory,
    }))
}

/// Drops an announcement whose active window has passed, or has not opened yet.
fn active_announcement(value: Value) -> Option<Value> {
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
