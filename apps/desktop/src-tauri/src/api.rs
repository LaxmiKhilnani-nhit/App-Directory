//! Wire types and the small HTTP layer the desktop client needs.
//!
//! This deliberately mirrors `apps/api/src/http/sync.rs`; keep the two in step when the
//! payload changes. Only the fields the client reads are declared.

use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncUser {
    pub name: String,
    pub department: Option<String>,
    pub must_change_password: bool,
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
    pub must_change_password: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginResponse {
    pub token: String,
    pub user: SessionUser,
    pub must_change_password: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    /// Stable machine code: `network_error`, `unauthenticated`, or the API's own code.
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

    pub fn unauthenticated() -> Self {
        Self::new(
            "unauthenticated",
            "Your session has expired. Please sign in again.",
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

pub struct ApiClient {
    http: reqwest::Client,
    base: String,
}

impl ApiClient {
    pub fn new(base: String) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(20))
            .user_agent(format!("AppDirectory/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("HTTP client");

        Self { http, base }
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

        request.send().await.map_err(CommandError::network)
    }

    fn failure(status: reqwest::StatusCode, body: &str) -> CommandError {
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return CommandError::unauthenticated();
        }

        let parsed: Option<ErrorEnvelope> = serde_json::from_str(body).ok();
        match parsed {
            Some(envelope) => CommandError::new(envelope.error.code, envelope.error.message),
            None => CommandError::new(
                "server_error",
                format!("The server answered {status} without a usable message."),
            ),
        }
    }

    pub async fn login(&self, email: &str, password: &str) -> Result<LoginResponse, CommandError> {
        let response = self
            .send(
                self.http
                    .post(self.url("/api/v1/auth/login"))
                    .json(&serde_json::json!({ "email": email, "password": password })),
                None,
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

    pub async fn session_user(
        &self,
        token: &str,
        timeout: Duration,
    ) -> Result<SessionUser, CommandError> {
        let response = self
            .send(
                self.http
                    .get(self.url("/api/v1/auth/me"))
                    .bearer_auth(token),
                Some(timeout),
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

    pub async fn sync(&self, token: &str) -> Result<SyncPayload, CommandError> {
        let response = self
            .send(
                self.http
                    .get(self.url("/api/v1/client/sync"))
                    .bearer_auth(token),
                None,
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

    pub async fn logout(&self, token: &str) -> Result<(), CommandError> {
        let response = self
            .send(
                self.http
                    .post(self.url("/api/v1/auth/logout"))
                    .bearer_auth(token),
                Some(Duration::from_secs(8)),
            )
            .await?;

        if response.status().is_success() || response.status() == reqwest::StatusCode::UNAUTHORIZED
        {
            return Ok(());
        }

        Err(Self::failure(
            response.status(),
            &response.text().await.unwrap_or_default(),
        ))
    }

    pub async fn change_password(
        &self,
        token: &str,
        current_password: &str,
        new_password: &str,
    ) -> Result<(), CommandError> {
        let response = self
            .send(
                self.http
                    .post(self.url("/api/v1/auth/change-password"))
                    .bearer_auth(token)
                    .json(&serde_json::json!({
                        "currentPassword": current_password,
                        "newPassword": new_password,
                    })),
                None,
            )
            .await?;

        if response.status().is_success() {
            return Ok(());
        }

        Err(Self::failure(
            response.status(),
            &response.text().await.unwrap_or_default(),
        ))
    }

    /// Always `202` for known and unknown addresses, so the UI copy stays neutral.
    pub async fn request_reset(&self, email: &str) -> Result<(), CommandError> {
        let response = self
            .send(
                self.http
                    .post(self.url("/api/v1/auth/forgot"))
                    .json(&serde_json::json!({ "email": email })),
                None,
            )
            .await?;

        if response.status().is_success() {
            return Ok(());
        }

        Err(Self::failure(
            response.status(),
            &response.text().await.unwrap_or_default(),
        ))
    }

    pub async fn reset_password(
        &self,
        email: &str,
        code: &str,
        new_password: &str,
    ) -> Result<(), CommandError> {
        let response = self
            .send(
                self.http
                    .post(self.url("/api/v1/auth/reset"))
                    .json(&serde_json::json!({
                        "email": email,
                        "code": code,
                        "newPassword": new_password,
                    })),
                None,
            )
            .await?;

        if response.status().is_success() {
            return Ok(());
        }

        Err(Self::failure(
            response.status(),
            &response.text().await.unwrap_or_default(),
        ))
    }

    /// `Ok(None)` means the stored icon is still current (the server answered `304`).
    pub async fn icon(
        &self,
        token: &str,
        path: &str,
        etag: Option<&str>,
    ) -> Result<Option<Vec<u8>>, CommandError> {
        let mut request = self.http.get(self.url(path)).bearer_auth(token);
        if let Some(etag) = etag {
            request = request.header(reqwest::header::IF_NONE_MATCH, etag);
        }

        let response = self.send(request, Some(Duration::from_secs(15))).await?;
        let status = response.status();

        if status == reqwest::StatusCode::NOT_MODIFIED {
            return Ok(None);
        }

        if status.is_success() {
            let bytes = response.bytes().await.map_err(CommandError::network)?;
            return Ok(Some(bytes.to_vec()));
        }

        // A missing icon is not fatal: the tile falls back to its letter.
        if status == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        Err(Self::failure(
            status,
            &response.text().await.unwrap_or_default(),
        ))
    }
}
