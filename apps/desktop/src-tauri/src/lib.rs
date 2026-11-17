mod api;
mod cache;
mod config;
mod credentials;
mod icons;
mod shortcuts;
mod sync;

use std::sync::Mutex;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use serde::Serialize;
use tauri::{Manager, State};

use api::{ApiClient, CommandError};

/// Shared per-process state. The HTTP client is cheap to clone; the timestamp enforces the
/// one-per-minute manual refresh budget from PLAN.md §6.3.
struct Client {
    api: ApiClient,
    last_manual_sync: Mutex<Option<Instant>>,
}

impl Client {
    fn token(&self) -> Result<String, CommandError> {
        credentials::token()
            .map_err(|error| CommandError::new("credential_error", error))?
            .ok_or_else(CommandError::unauthenticated)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CoreInfo {
    app_name: String,
    version: String,
    api_base_url: String,
    running_in_tauri: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionInfo {
    name: String,
    department: Option<String>,
    must_change_password: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionState {
    signed_in: bool,
    name: Option<String>,
    department: Option<String>,
    must_change_password: bool,
    cache_available: bool,
    cache_expired: bool,
    last_synced_at: Option<DateTime<Utc>>,
    server_reachable: bool,
}

#[tauri::command]
fn core_info() -> CoreInfo {
    CoreInfo {
        app_name: config::APP_NAME.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        api_base_url: config::api_base_url(),
        running_in_tauri: true,
    }
}

/// Boot probe: is there a stored session, and what does the cache know about it?
#[tauri::command]
async fn session_state(client: State<'_, Client>) -> Result<SessionState, CommandError> {
    let token =
        credentials::token().map_err(|error| CommandError::new("credential_error", error))?;
    let cached = cache::load().ok().flatten();
    let now = Utc::now();

    let mut state = SessionState {
        signed_in: token.is_some(),
        name: None,
        department: None,
        must_change_password: false,
        cache_available: cached.is_some(),
        cache_expired: cached.as_ref().is_some_and(|stored| stored.is_expired(now)),
        last_synced_at: cached.as_ref().map(|stored| stored.last_synced_at),
        server_reachable: false,
    };

    if let Some(stored) = &cached {
        state.name = Some(stored.payload.user.name.clone());
        state.department = stored.payload.user.department.clone();
        state.must_change_password = stored.payload.user.must_change_password;
    }

    if let Some(token) = token {
        match client
            .api
            .session_user(&token, Duration::from_secs(4))
            .await
        {
            Ok(user) => {
                state.name = Some(user.name);
                state.department = user.department;
                state.must_change_password = user.must_change_password;
                state.server_reachable = true;
            }
            Err(error) if error.code == "unauthenticated" => {
                let _ = credentials::clear_token();
                state.signed_in = false;
            }
            Err(_) => {
                state.server_reachable = false;
            }
        }
    }

    Ok(state)
}

#[tauri::command]
async fn login(
    client: State<'_, Client>,
    email: String,
    password: String,
) -> Result<SessionInfo, CommandError> {
    let response = client.api.login(email.trim(), &password).await?;

    // A session can only be created online, so any snapshot on disk belongs to whoever was
    // signed in before. Drop it before this user's data is fetched, or a failed sync would
    // show the previous person's apps.
    cache::clear();

    credentials::set_token(&response.token)
        .map_err(|error| CommandError::new("credential_error", error))?;

    Ok(SessionInfo {
        name: response.user.name,
        department: response.user.department,
        must_change_password: response.must_change_password,
    })
}

#[tauri::command]
async fn logout(client: State<'_, Client>) -> Result<(), CommandError> {
    if let Ok(Some(token)) = credentials::token() {
        // Best effort: a dead server should not keep the user signed in locally.
        let _ = client.api.logout(&token).await;
    }

    // Drop the decrypted-at-rest snapshot too: a signed-out profile should not leave
    // another person's app list readable offline (PLAN.md §6.1).
    cache::clear();

    credentials::clear_token().map_err(|error| CommandError::new("credential_error", error))
}

#[tauri::command]
async fn change_password(
    client: State<'_, Client>,
    current_password: String,
    new_password: String,
) -> Result<(), CommandError> {
    let token = client.token()?;
    client
        .api
        .change_password(&token, &current_password, &new_password)
        .await?;

    // The stored snapshot still says "must change password"; keep the offline view honest.
    if let Ok(Some(mut stored)) = cache::load() {
        stored.payload.user.must_change_password = false;
        let _ = cache::save(&stored);
    }

    Ok(())
}

#[tauri::command]
async fn request_password_reset(
    client: State<'_, Client>,
    email: String,
) -> Result<(), CommandError> {
    client.api.request_reset(email.trim()).await
}

#[tauri::command]
async fn reset_password(
    client: State<'_, Client>,
    email: String,
    code: String,
    new_password: String,
) -> Result<(), CommandError> {
    client
        .api
        .reset_password(email.trim(), code.trim(), &new_password)
        .await
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

    let token = client.token()?;
    let stored = match sync::fetch(&client.api, &token).await {
        Ok(stored) => stored,
        Err(error) if error.code == "unauthenticated" => {
            let _ = credentials::clear_token();
            return Err(error);
        }
        Err(error) => return Err(error),
    };

    Ok(sync::view(&stored, "server", Utc::now()))
}

#[tauri::command]
fn cached_snapshot() -> Result<Option<sync::SnapshotView>, CommandError> {
    let stored = cache::load().map_err(|error| CommandError::new("cache_error", error))?;
    Ok(stored.map(|stored| sync::view(&stored, "cache", Utc::now())))
}

/// Opens a tile in the default browser. Only `http`/`https` leave the client.
#[tauri::command]
fn open_app(url: String) -> Result<(), CommandError> {
    let parsed = reqwest::Url::parse(&url)
        .map_err(|_| CommandError::new("invalid_url", "That app has an invalid address."))?;

    match parsed.scheme() {
        "http" | "https" => open::that(parsed.as_str()).map_err(|error| {
            CommandError::new("open_failed", format!("Could not open the link: {error}"))
        }),
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
            session_state,
            login,
            logout,
            change_password,
            request_password_reset,
            reset_password,
            sync_snapshot,
            cached_snapshot,
            open_app,
        ])
        .run(tauri::generate_context!())
        .expect("error while running App Directory");
}

/// Live end-to-end check against a running server (PLAN.md §10, Phase 5).
///
/// Ignored by default because it needs the dev stack and a real account. Run with:
/// `APPDIR_API_BASE=http://127.0.0.1:8090 APPDIR_E2E_EMAIL=... APPDIR_E2E_PASSWORD=...`
/// `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml -- --ignored`.
///
/// It writes and then removes the same files and credentials the real client uses, so the
/// next launch starts clean.
#[cfg(test)]
mod live {
    use super::*;

    #[tokio::test]
    #[ignore = "requires a running App Directory server and a test account"]
    async fn login_sync_cache_roundtrip() {
        let email = std::env::var("APPDIR_E2E_EMAIL").expect("APPDIR_E2E_EMAIL");
        let password = std::env::var("APPDIR_E2E_PASSWORD").expect("APPDIR_E2E_PASSWORD");
        let api = ApiClient::new(config::api_base_url());

        let login = api.login(&email, &password).await.expect("login");
        assert!(!login.token.is_empty());
        credentials::set_token(&login.token).expect("store token");
        let token = credentials::token()
            .expect("read token")
            .expect("token present");

        let payload = api.sync(&token).await.expect("sync");
        assert!(
            payload.sync_interval_hours >= 1,
            "server must send a cadence"
        );

        let stored = sync::fetch(&api, &token).await.expect("fetch and persist");
        assert_eq!(stored.payload.revision, payload.revision);

        let view = sync::view(&stored, "server", Utc::now());
        assert_eq!(view.apps.len(), payload.apps.len());
        assert!(!view.expired, "a fresh snapshot is inside the grace window");
        assert_eq!(view.source, "server");

        // The decrypted cache must survive a process restart, which is the offline path.
        let reloaded = cache::load().expect("read cache").expect("cache exists");
        assert_eq!(reloaded.payload.revision, payload.revision);
        assert_eq!(reloaded.payload.apps.len(), payload.apps.len());
        assert_eq!(reloaded.payload.user.name, view.user.name);
        let offline = sync::view(&reloaded, "cache", Utc::now());
        assert!(!offline.expired);

        // Cold start with the server down: this network failure is what sends the UI to the
        // cached snapshot, so it must be classified as recoverable rather than an auth error.
        let unreachable = ApiClient::new("http://127.0.0.1:1".to_string());
        let failure = unreachable.sync(&token).await.expect_err("server is down");
        assert_eq!(failure.code, "network_error");

        api.logout(&token).await.ok();

        // `APPDIR_E2E_KEEP=1` leaves the token and cache in place so a packaged exe can be
        // launched offline against them. Without it the test cleans up after itself.
        if std::env::var("APPDIR_E2E_KEEP").is_ok() {
            eprintln!("APPDIR_E2E_KEEP is set: token and cache left in place");
            return;
        }

        cache::clear();
        credentials::clear_token().expect("clear token");
    }
}
