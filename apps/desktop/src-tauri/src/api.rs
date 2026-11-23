//! Wire types and the small HTTP layer the desktop client needs.
//!
//! There are no passwords, tokens or sessions: every request carries the current Windows
//! identity in `X-Windows-Identity`, and the server resolves entitlements from its CSV store.

use std::sync::{Arc, RwLock};
use std::time::Duration;

use chrono::{DateTime, Utc};
use reqwest::header::HeaderValue;
use serde::{Deserialize, Serialize};

use crate::identity;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncUser {
    pub name: String,
    pub department: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncApp {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncPayload {
    pub revision: i64,
    pub server_time: DateTime<Utc>,
    pub offline_valid_until: DateTime<Utc>,
    pub snapshot_days: i64,
    pub sync_interval_hours: i64,
    pub announcement: Option<serde_json::Value>,
    pub user: SyncUser,
    pub apps: Vec<SyncApp>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionUser {
    pub name: String,
    pub department: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub available: bool,
    pub version: String,
    pub sha256: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    pub code: String,
    pub message: String,
}

impl CommandError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }

    pub fn network(error: impl std::fmt::Display) -> Self {
        Self::new(
            "network_error",
            format!("Could not reach the App Directory server ({error})."),
        )
    }

    pub fn unknown_user() -> Self {
        Self::new(
            "unknown_user",
            "Your Windows account is not in the App Directory. Contact IT.",
        )
    }
}

#[derive(Deserialize)]
struct ErrorEnvelope {
    error: ErrorBody,
}

#[derive(Deserialize)]
struct ErrorBody {
    code: String,
    message: String,
}

#[derive(Clone)]
pub struct ApiClient {
    http: reqwest::Client,
    base: String,
    /// Changed when the user confirms their email; shared with the Tauri commands.
    identity: Arc<RwLock<String>>,
}

impl ApiClient {
    pub fn new(base: String) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .user_agent(format!("AppDirectory/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("HTTP client");

        Self {
            http,
            base,
            identity: Arc::new(RwLock::new(identity::current())),
        }
    }

    pub fn identity(&self) -> String {
        self.identity
            .read()
            .map(|value| value.clone())
            .unwrap_or_else(|_| identity::current())
    }

    pub fn set_identity(&self, email: &str) {
        if let Ok(mut value) = self.identity.write() {
            *value = email.trim().to_string();
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base, path)
    }

    async fn send(
        &self,
        request: reqwest::RequestBuilder,
        timeout: Option<Duration>,
    ) -> Result<reqwest::Response, CommandError> {
        let request = match timeout {
            Some(limit) => request.timeout(limit),
            None => request,
        };

        let value = self.identity();
        let request = match HeaderValue::from_str(&value) {
            Ok(header) => request.header("x-windows-identity", header),
            Err(_) => request,
        };

        request.send().await.map_err(CommandError::network)
    }

    fn failure(status: reqwest::StatusCode, body: &str) -> CommandError {
        let parsed: Option<ErrorEnvelope> = serde_json::from_str(body).ok();
        match parsed {
            Some(envelope) if envelope.error.code == "unknown_user" => CommandError::unknown_user(),
            Some(envelope) => CommandError::new(envelope.error.code, envelope.error.message),
            None => CommandError::new(
                "server_error",
                format!("The server answered {status} without a usable message."),
            ),
        }
    }

    pub async fn session_user(&self, timeout: Duration) -> Result<SessionUser, CommandError> {
        let response = self
            .send(self.http.get(self.url("/api/v1/auth/me")), Some(timeout))
            .await?;

        let status = response.status();
        let body = response.text().await.map_err(CommandError::network)?;
        if !status.is_success() {
            return Err(Self::failure(status, &body));
        }

        serde_json::from_str(&body)
            .map_err(|error| CommandError::new("invalid_response", error.to_string()))
    }

    pub async fn sync(&self) -> Result<SyncPayload, CommandError> {
        let response = self
            .send(self.http.get(self.url("/api/v1/client/sync")), None)
            .await?;

        let status = response.status();
        let body = response.text().await.map_err(CommandError::network)?;
        if !status.is_success() {
            return Err(Self::failure(status, &body));
        }

        serde_json::from_str(&body)
            .map_err(|error| CommandError::new("invalid_response", error.to_string()))
    }

    pub async fn icon(
        &self,
        path: &str,
        etag: Option<&str>,
    ) -> Result<Option<Vec<u8>>, CommandError> {
        let mut request = self.http.get(self.url(path));
        if let Some(etag) = etag {
            request = request.header(reqwest::header::IF_NONE_MATCH, etag);
        }

        let response = self.send(request, Some(Duration::from_secs(5))).await?;
        let status = response.status();

        if status == reqwest::StatusCode::NOT_MODIFIED {
            return Ok(None);
        }
        if status.is_success() {
            let bytes = response.bytes().await.map_err(CommandError::network)?;
            return Ok(Some(bytes.to_vec()));
        }
        if status == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        Err(Self::failure(
            status,
            &response.text().await.unwrap_or_default(),
        ))
    }

    pub async fn update_info(&self) -> Result<UpdateInfo, CommandError> {
        let response = self
            .send(
                self.http.get(self.url("/api/v1/client/update")),
                Some(Duration::from_secs(10)),
            )
            .await?;

        let status = response.status();
        let body = response.text().await.map_err(CommandError::network)?;
        if !status.is_success() {
            return Err(Self::failure(status, &body));
        }

        serde_json::from_str(&body)
            .map_err(|error| CommandError::new("invalid_response", error.to_string()))
    }

    /// Downloads a URL, reporting cumulative bytes through `on_progress(downloaded, total)`.
    pub async fn download_with_progress<F>(
        &self,
        url: &str,
        on_progress: F,
    ) -> Result<Vec<u8>, CommandError>
    where
        F: Fn(u64, u64),
    {
        let target = if url.starts_with("http://") || url.starts_with("https://") {
            url.to_string()
        } else {
            self.url(url)
        };

        let mut response = self
            .send(self.http.get(&target), Some(Duration::from_secs(120)))
            .await?;
        let status = response.status();
        if !status.is_success() {
            return Err(CommandError::new(
                "download_failed",
                format!("Could not download the update ({status})."),
            ));
        }

        let total = response.content_length().unwrap_or(0);
        let mut bytes = Vec::with_capacity(total.min(16 * 1024 * 1024) as usize);
        on_progress(0, total);
        while let Some(chunk) = response.chunk().await.map_err(CommandError::network)? {
            bytes.extend_from_slice(&chunk);
            on_progress(bytes.len() as u64, total);
        }
        Ok(bytes)
    }
}
