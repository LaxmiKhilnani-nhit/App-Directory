//! Build-time configuration and on-disk locations for the App Directory client.
//!
//! The API base URL is baked in at build time (PLAN.md §11). Override it at runtime with
//! `APPDIR_API_BASE` for local development against a dev server.

use std::path::PathBuf;

pub const DEFAULT_API_BASE_URL: &str = "http://172.168.50.50:8090";
pub const APP_NAME: &str = "App Directory";

/// Bumped only when the on-disk layout changes in a way that needs migration.
pub const SCHEMA_VERSION: u32 = 1;

pub fn api_base_url() -> String {
    std::env::var("APPDIR_API_BASE")
        .unwrap_or_else(|_| DEFAULT_API_BASE_URL.to_string())
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

pub fn cache_path() -> PathBuf {
    data_dir().join("cache.bin")
}
