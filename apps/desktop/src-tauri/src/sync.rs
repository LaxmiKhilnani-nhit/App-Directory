//! Fetch the entitled snapshot, refresh icons and persist everything for offline use.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::api::{ApiClient, CommandError, SyncApp, SyncPayload, SyncUser};
use crate::{cache, config, icons};

/// Mirrors `AppTile` in `packages/types`, with `iconUrl` already turned into a data URL.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppView {
    pub id: String,
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotView {
    pub revision: i64,
    pub server_time: DateTime<Utc>,
    pub offline_valid_until: DateTime<Utc>,
    pub snapshot_days: i64,
    pub sync_interval_hours: i64,
    pub announcement: Option<serde_json::Value>,
    pub user: SyncUser,
    pub apps: Vec<AppView>,
    pub last_synced_at: DateTime<Utc>,
    /// `server` when it just came off the wire, `cache` when read from disk.
    pub source: &'static str,
    /// Only meaningful for `cache`: the offline grace window has closed.
    pub expired: bool,
}

fn view_of(app: &SyncApp) -> AppView {
    AppView {
        id: app.id.clone(),
        name: app.name.clone(),
        url: app.url.clone(),
        description: app.description.clone(),
        category: app.category.clone(),
        color: app.color.clone(),
        sort_order: app.sort_order,
        offline_visible: app.offline_visible,
        icon_url: if app.icon_etag.is_some() {
            icons::data_url(&app.id)
        } else {
            None
        },
        icon_etag: app.icon_etag.clone(),
        updated_at: app.updated_at,
    }
}

pub fn view(
    stored: &cache::StoredSnapshot,
    source: &'static str,
    now: DateTime<Utc>,
) -> SnapshotView {
    SnapshotView {
        revision: stored.payload.revision,
        server_time: stored.payload.server_time,
        offline_valid_until: stored.payload.offline_valid_until,
        snapshot_days: stored.payload.snapshot_days,
        sync_interval_hours: stored.payload.sync_interval_hours.max(1),
        announcement: stored.payload.announcement.clone(),
        user: stored.payload.user.clone(),
        apps: stored.payload.apps.iter().map(view_of).collect(),
        last_synced_at: stored.last_synced_at,
        source,
        expired: stored.is_expired(now),
    }
}

fn previous_etags() -> HashMap<String, String> {
    cache::load()
        .ok()
        .flatten()
        .map(|stored| {
            stored
                .payload
                .apps
                .into_iter()
                .filter_map(|app| app.icon_etag.map(|etag| (app.id, etag)))
                .collect()
        })
        .unwrap_or_default()
}

async fn refresh_icons(api: &ApiClient, apps: &[SyncApp], prior: &HashMap<String, String>) {
    for app in apps {
        let (Some(path), Some(etag)) = (app.icon_url.as_deref(), app.icon_etag.as_deref()) else {
            continue;
        };

        let file = icons::path_for(&app.id);
        let unchanged = prior.get(&app.id).map(String::as_str) == Some(etag);
        if unchanged && file.exists() {
            continue;
        }

        match api.icon(path, prior.get(&app.id).map(String::as_str)).await {
            Ok(Some(bytes)) => {
                if let Err(error) = icons::save(&file, &bytes) {
                    tracing::warn!("icon save failed: {error}");
                }
            }
            // 304: the cached file is still correct even though the payload changed elsewhere.
            Ok(None) => {}
            Err(error) => tracing::warn!("icon fetch failed for {}: {}", app.name, error.message),
        }
    }

    let keep: HashSet<String> = apps
        .iter()
        .filter(|app| app.icon_etag.is_some())
        .map(|app| app.id.clone())
        .collect();
    icons::prune(&keep);
}

/// Hits `/client/sync`, refreshes icons, then writes the encrypted cache.
pub async fn fetch(api: &ApiClient) -> Result<cache::StoredSnapshot, CommandError> {
    let payload: SyncPayload = api.sync().await?;
    let prior = previous_etags();

    refresh_icons(api, &payload.apps, &prior).await;

    let stored = cache::StoredSnapshot {
        schema_version: config::SCHEMA_VERSION,
        last_synced_at: Utc::now(),
        payload,
    };

    cache::save(&stored).map_err(|error| {
        CommandError::new("cache_error", format!("Could not write the cache: {error}"))
    })?;
    cache::remember_sync(stored.last_synced_at, stored.payload.revision).map_err(|error| {
        CommandError::new(
            "cache_error",
            format!("Could not write config.json: {error}"),
        )
    })?;

    Ok(stored)
}
