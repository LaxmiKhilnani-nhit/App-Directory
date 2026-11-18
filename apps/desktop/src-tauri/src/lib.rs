mod api;
mod cache;
mod config;
mod credentials;
mod icons;
mod identity;
mod shortcuts;
mod sync;
mod updater;

use std::sync::Mutex;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use serde::Serialize;
use tauri::{Manager, State};

use api::{ApiClient, CommandError};

/// Shared per-process state. The manual refresh budget stays at one per minute.
struct Client {
    api: ApiClient,
    last_manual_sync: Mutex<Option<Instant>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CoreInfo {
    app_name: String,
    version: String,
    api_base_url: String,
    running_in_tauri: bool,
    identity: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct IdentityState {
    identity: String,
    short_name: String,
    name: Option<String>,
    department: Option<String>,
    cache_available: bool,
    cache_expired: bool,
    last_synced_at: Option<DateTime<Utc>>,
    server_reachable: bool,
    known_user: bool,
}

#[tauri::command]
fn core_info() -> CoreInfo {
    CoreInfo {
        app_name: config::APP_NAME.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        api_base_url: config::api_base_url(),
        running_in_tauri: true,
        identity: identity::windows_identity(),
    }
}

/// Boot probe: derive the Windows identity, check the cached snapshot, and try the server.
#[tauri::command]
async fn identity_state(client: State<'_, Client>) -> Result<IdentityState, CommandError> {
    let raw = identity::windows_identity();
    let cached = cache::load().ok().flatten();
    let now = Utc::now();

    let mut state = IdentityState {
        identity: raw.clone(),
        short_name: identity::short_name(&raw),
        name: None,
        department: None,
        cache_available: cached.is_some(),
        cache_expired: cached.as_ref().is_some_and(|stored| stored.is_expired(now)),
        last_synced_at: cached.as_ref().map(|stored| stored.last_synced_at),
        server_reachable: false,
        known_user: false,
    };

    if let Some(stored) = &cached {
        state.name = Some(stored.payload.user.name.clone());
        state.department = stored.payload.user.department.clone();
    }

    match client.api.session_user(Duration::from_secs(4)).await {
        Ok(user) => {
            state.name = Some(user.name);
            state.department = user.department;
            state.server_reachable = true;
            state.known_user = true;
        }
        Err(error) if error.code == "unknown_user" => {
            state.known_user = false;
        }
        Err(_) => {}
    }

    Ok(state)
}

/// Live sync. `manual` enforces the one-per-minute refresh budget.
#[tauri::command]
async fn sync_snapshot(
    client: State<'_, Client>,
    manual: bool,
) -> Result<sync::SnapshotView, CommandError> {
    if manual {
        let mut last = client
            .last_manual_sync
            .lock()
            .map_err(|_| CommandError::new("internal_error", "The client state is poisoned."))?;
        if let Some(previous) = *last {
            let elapsed = previous.elapsed();
            if elapsed < Duration::from_secs(60) {
                let wait = 60 - elapsed.as_secs();
                return Err(CommandError::new(
                    "rate_limited",
                    format!("Please wait {wait}s before refreshing again."),
                ));
            }
        }
        *last = Some(Instant::now());
    }

    let stored = sync::fetch(&client.api).await?;
    Ok(sync::view(&stored, "server", Utc::now()))
}

#[tauri::command]
fn cached_snapshot() -> Result<Option<sync::SnapshotView>, CommandError> {
    let stored = cache::load().map_err(|error| CommandError::new("cache_error", error))?;
    Ok(stored.map(|stored| sync::view(&stored, "cache", Utc::now())))
}

#[tauri::command]
async fn check_update(client: State<'_, Client>) -> Result<api::UpdateInfo, CommandError> {
    client.api.update_info().await
}

#[tauri::command]
async fn apply_update(client: State<'_, Client>) -> Result<(), CommandError> {
    let info = client.api.update_info().await?;
    if !info.available {
        return Err(CommandError::new("no_update", "No update is available."));
    }
    updater::install(&client.api, &info).await
}

/// Opens a tile in the default browser. Only `http`/`https` leave the client.
#[tauri::command]
fn open_app(url: String) -> Result<(), CommandError> {
    let parsed = reqwest::Url::parse(&url)
        .map_err(|_| CommandError::new("invalid_url", "That app has an invalid address."))?;

    match parsed.scheme() {
        "http" | "https" => open::that(parsed.as_str())
            .map_err(|error| CommandError::new("open_failed", format!("Could not open the link: {error}"))),
        _ => Err(CommandError::new(
            "invalid_url",
            "Only web links can be opened from the directory.",
        )),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let client = Client {
        api: ApiClient::new(config::api_base_url()),
        last_manual_sync: Mutex::new(None),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .manage(client)
        .setup(|_app| {
            let should_create =
                !cfg!(debug_assertions) || std::env::var("APPDIR_CREATE_SHORTCUTS").is_ok();

            if should_create {
                match std::env::current_exe() {
                    Ok(executable) => {
                        if let Err(error) = shortcuts::ensure(&executable) {
                            tracing::warn!("could not create shortcuts: {error}");
                        }
                    }
                    Err(error) => tracing::warn!("could not locate the executable: {error}"),
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            core_info,
            identity_state,
            sync_snapshot,
            cached_snapshot,
            check_update,
            apply_update,
            open_app,
        ])
        .run(tauri::generate_context!())
        .expect("error while running App Directory");
}

/// Live end-to-end check against a running server.
#[cfg(test)]
mod live {
    use super::*;

    #[tokio::test]
    #[ignore = "requires a running App Directory server"]
    async fn identity_sync_and_cache_roundtrip() {
        let api = ApiClient::new(config::api_base_url());
        let user = api.session_user(Duration::from_secs(5)).await.expect("me");
        assert!(!user.name.is_empty());

        let stored = sync::fetch(&api).await.expect("fetch and persist");
        let view = sync::view(&stored, "server", Utc::now());
        assert!(!view.expired);

        let reloaded = cache::load().expect("read cache").expect("cache exists");
        assert_eq!(reloaded.payload.revision, stored.payload.revision);
    }
}
