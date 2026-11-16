use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    pub department: Option<String>,
    #[serde(skip_serializing)]
    pub password_hash: Option<String>,
    pub must_change_password: bool,
    pub role: String,
    pub admin_scope: Option<serde_json::Value>,
    pub is_active: bool,
    pub failed_attempts: i32,
    pub locked_until: Option<DateTime<Utc>>,
    pub last_login_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub auto_department: Option<String>,
    pub is_all_users: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Full application record, including icon bytes. Used by the admin API and icon route.
#[derive(Debug, Clone, FromRow)]
pub struct App {
    pub id: Uuid,
    pub name: String,
    pub url: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub color: Option<String>,
    pub sort_order: i32,
    pub is_enabled: bool,
    pub offline_visible: bool,
    pub icon_data: Option<Vec<u8>>,
    pub icon_mime: Option<String>,
    pub icon_source: String,
    pub icon_fetched_at: Option<DateTime<Utc>>,
    pub icon_etag: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// What the client sync endpoint sends: no icon bytes, just a URL + ETag.
#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct AppSummary {
    pub id: Uuid,
    pub name: String,
    pub url: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub color: Option<String>,
    pub sort_order: i32,
    pub offline_visible: bool,
    pub icon_source: String,
    pub icon_etag: Option<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct Session {
    pub id: Uuid,
    pub user_id: Uuid,
    pub token_hash: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub client_info: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
pub struct ResetCode {
    pub id: Uuid,
    pub user_id: Uuid,
    pub code_hash: String,
    pub purpose: String,
    pub expires_at: DateTime<Utc>,
    pub used_at: Option<DateTime<Utc>>,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct ResetRequest {
    pub id: Uuid,
    pub user_id: Uuid,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub resolved_by: Option<Uuid>,
}

#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Setting {
    pub key: String,
    pub value: serde_json::Value,
    pub updated_at: DateTime<Utc>,
    pub updated_by: Option<Uuid>,
}

#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditEntry {
    pub id: i64,
    pub actor_user_id: Option<Uuid>,
    pub action: String,
    pub entity_type: String,
    pub entity_id: Option<Uuid>,
    pub before: Option<serde_json::Value>,
    pub after: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}
