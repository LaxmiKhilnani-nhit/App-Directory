//! Build-time configuration and on-disk locations for the App Directory client.
//!
//! The API base URL is baked in at build time (PLAN.md §11). Override it at runtime with
//! `APPDIR_API_BASE` for local development against a dev server.

use std::path::PathBuf;

pub const DEFAULT_API_BASE_URL: &str = "http://172.168.50.50:8090";
pub const APP_NAME: &str = "App Directory";
/// Used only to suggest an email on first run; the user can change it before saving.
pub const DEFAULT_MAIL_DOMAIN: &str = "nhit.co.in";

/// Bumped only when the on-disk layout changes in a way that needs migration.
pub const SCHEMA_VERSION: u32 = 1;

pub fn server_file_path() -> PathBuf {
    data_dir().join("server.txt")
}

pub fn api_base_url() -> String {
    // Order: process environment (dev), then an admin-editable file on the machine, then the
    // URL baked at build time. The file lets a pilot machine be repointed without a rebuild.
    let from_file = std::fs::read_to_string(server_file_path())
        .ok()
        .map(|value| value.trim().trim_end_matches('/').to_string())
        .filter(|value| !value.is_empty());

    std::env::var("APPDIR_API_BASE")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or(from_file)
        .unwrap_or_else(|| DEFAULT_API_BASE_URL.to_string())
        .trim_end_matches('/')
        .to_string()
}

/// `%LOCALAPPDATA%\AppDirectory` — matches PLAN.md §6.1 and the asset-protocol scope.
pub fn data_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("AppDirectory")
}

pub fn icons_dir() -> PathBuf {
    data_dir().join("icons")
}

pub fn config_path() -> PathBuf {
    data_dir().join("config.json")
}

/// The user's work email, entered once and reused on every launch.
pub fn identity_path() -> PathBuf {
    data_dir().join("identity.txt")
}

pub fn cached_identity() -> Option<String> {
    std::fs::read_to_string(identity_path())
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub fn save_identity(email: &str) -> Result<(), String> {
    let path = identity_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    std::fs::write(&path, email.trim())
        .map_err(|error| format!("Could not save your email on this device: {error}"))
}

pub fn clear_identity() {
    let _ = std::fs::remove_file(identity_path());
}

pub fn cache_path() -> PathBuf {
    data_dir().join("cache.bin")
}
