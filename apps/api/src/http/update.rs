use axum::{extract::rejection::JsonRejection, extract::State, Json};
use chrono::Utc;
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::domain::models::UpdateInfo;
use crate::error::{ApiError, ApiResult};
use crate::http::identity::TrustedUser;
use crate::http::json_body;
use crate::state::AppState;

const DEFAULT_URL: &str = "/download/app-directory.exe";
const DEFAULT_EXE: &str = "/app/client/App Directory.exe";

fn client_exe() -> std::path::PathBuf {
    std::env::var("CLIENT_EXE")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(DEFAULT_EXE))
}

/// Hashes the exe currently published on the server so admins only set a version and flag.
/// Returns an empty string when no binary is present — clients then do not offer the update.
pub fn published_sha256() -> String {
    let Ok(bytes) = std::fs::read(client_exe()) else {
        return String::new();
    };
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Driven by settings.csv keys `update_enabled`, `update_version` and `update_url`; the
/// checksum is computed from the mounted executable.
pub async fn get(State(state): State<AppState>) -> Json<UpdateInfo> {
    let data = state.store.read();
    let enabled = data.setting_i64("update_enabled", 0) == 1;
    let version = data.setting_str("update_version").unwrap_or_default();
    // The mounted exe is the source of truth. A manually supplied checksum remains a
    // fallback for installs where no exe is mounted (and for tests).
    let sha256 = {
        let computed = published_sha256();
        if computed.is_empty() {
            data.setting_str("update_sha256").unwrap_or_default()
        } else {
            computed
        }
    };
    let url = data
        .setting_str("update_url")
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_URL.to_string());

    Json(UpdateInfo {
        available: enabled && !version.is_empty() && !sha256.is_empty(),
        version,
        sha256,
        url,
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishRequest {
    pub version: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub sha256: String,
    #[serde(default)]
    pub url: Option<String>,
    /// Ignored: the binary itself is published by placing it in the mounted release folder.
    #[serde(default)]
    pub file: Option<String>,
}

pub async fn publish(
    State(state): State<AppState>,
    TrustedUser(actor): TrustedUser,
    body: Result<Json<PublishRequest>, JsonRejection>,
) -> ApiResult<Json<UpdateInfo>> {
    let request = json_body(body)?;
    let version = request.version.trim().to_string();
    if version.is_empty() {
        return Err(ApiError::bad_request(
            "invalid_version",
            "A version is required.",
        ));
    }

    let actor_id = actor.id;
    let actor_email = actor.email.clone();
    // Normally the server computes the checksum from the mounted exe; this value is a
    // fallback for installs without one (and for the HTTP tests).
    let sha256 = request.sha256.trim().to_string();
    let url = request
        .url
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_URL.to_string());

    state.store.mutate(|data| {
        let now = Utc::now();
        for (key, value) in [
            ("update_version", json!(version)),
            ("update_sha256", json!(sha256)),
            ("update_url", json!(url)),
            ("update_enabled", json!(1)),
        ] {
            if let Some(setting) = data.settings.iter_mut().find(|setting| setting.key == key) {
                setting.value = value;
                setting.updated_at = now;
                setting.updated_by = Some(actor_id);
            } else {
                data.settings.push(crate::store::SettingRow {
                    key: key.to_string(),
                    value,
                    updated_at: now,
                    updated_by: Some(actor_id),
                });
            }
        }
        data.bump_revision();
        data.audit(
            Some(actor_id),
            Some(actor_email.clone()),
            "update.publish",
            "settings",
            None,
            None,
            Some(json!({ "version": version })),
        );
        Ok(())
    })?;

    Ok(get(State(state)).await)
}
