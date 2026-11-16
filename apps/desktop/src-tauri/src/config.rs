//! Build-time configuration for the App Directory client.
//!
//! The API base URL is baked in at build time (PLAN.md §11). Override it at runtime with
//! `APPDIR_API_BASE` for local development against a dev server.

pub const DEFAULT_API_BASE_URL: &str = "http://172.168.50.50:8090";
pub const APP_NAME: &str = "App Directory";

pub fn api_base_url() -> String {
    std::env::var("APPDIR_API_BASE").unwrap_or_else(|_| DEFAULT_API_BASE_URL.to_string())
}
