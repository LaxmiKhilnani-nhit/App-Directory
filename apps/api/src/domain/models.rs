use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

use crate::store::{AppRow, Data, GroupRow, SettingRow, UserRow};

// ------------------------------------------------------------------ auth/session

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionUser {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    pub department: Option<String>,
    pub role: String,
    pub admin_scope: Option<Value>,
    pub must_change_password: bool,
}

impl From<&UserRow> for SessionUser {
    fn from(user: &UserRow) -> Self {
        Self {
            id: user.id,
            email: user.email.clone(),
            name: user.name.clone(),
            department: user.department.clone(),
            role: user.role.clone(),
            admin_scope: user.admin_scope.clone(),
            must_change_password: false,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginResponse {
    pub token: String,
    pub expires_at: DateTime<Utc>,
    pub user: SessionUser,
    pub must_change_password: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminMe {
    pub user: SessionUser,
    pub is_super_admin: bool,
}

// ------------------------------------------------------------------ client sync

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncUser {
    pub name: String,
    pub department: Option<String>,
    pub must_change_password: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncApp {
    pub id: Uuid,
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncPayload {
    pub revision: i64,
    pub server_time: DateTime<Utc>,
    pub offline_valid_until: DateTime<Utc>,
    pub snapshot_days: i64,
    pub sync_interval_hours: i64,
    pub announcement: Option<Value>,
    pub user: SyncUser,
    pub apps: Vec<SyncApp>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub available: bool,
    pub version: String,
    pub sha256: String,
    pub url: String,
}

// ------------------------------------------------------------------ admin

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminApp {
    pub id: Uuid,
    pub name: String,
    pub url: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub color: Option<String>,
    pub sort_order: i32,
    pub is_enabled: bool,
    pub offline_visible: bool,
    pub icon_source: String,
    pub icon_mime: Option<String>,
    pub icon_fetched_at: Option<DateTime<Utc>>,
    pub icon_etag: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub group_ids: Vec<Uuid>,
    pub user_ids: Vec<Uuid>,
}

impl AdminApp {
    pub fn from_row(app: &AppRow, data: &Data) -> Self {
        Self {
            id: app.id,
            name: app.name.clone(),
            url: app.url.clone(),
            description: app.description.clone(),
            category: app.category.clone(),
            color: app.color.clone(),
            sort_order: app.sort_order,
            is_enabled: app.is_enabled,
            offline_visible: app.offline_visible,
            icon_source: app.icon_source.clone(),
            icon_mime: app.icon_mime.clone(),
            icon_fetched_at: app.icon_fetched_at,
            icon_etag: app.icon_etag.clone(),
            created_at: app.created_at,
            updated_at: app.updated_at,
            deleted_at: app.deleted_at,
            group_ids: data.group_ids_for_app(app.id),
            user_ids: data.user_ids_for_app(app.id),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminGroup {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub auto_department: Option<String>,
    pub is_all_users: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub member_count: i64,
    pub app_count: i64,
}

impl AdminGroup {
    pub fn from_row(group: &GroupRow, data: &Data) -> Self {
        let member_count = if group.is_all_users {
            data.users.iter().filter(|user| user.is_active).count() as i64
        } else if let Some(department) = group.auto_department.as_deref() {
            data.users
                .iter()
                .filter(|user| user.is_active && user.department.as_deref() == Some(department))
                .count() as i64
        } else {
            data.group_members
                .iter()
                .filter(|member| member.group_id == group.id)
                .count() as i64
        };

        let app_count = data
            .app_groups
            .iter()
            .filter(|grant| grant.group_id == group.id)
            .count() as i64;

        Self {
            id: group.id,
            name: group.name.clone(),
            description: group.description.clone(),
            auto_department: group.auto_department.clone(),
            is_all_users: group.is_all_users,
            created_at: group.created_at,
            updated_at: group.updated_at,
            member_count,
            app_count,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupDetail {
    #[serde(flatten)]
    pub group: AdminGroup,
    pub user_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminUser {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    pub department: Option<String>,
    pub role: String,
    pub admin_scope: Option<Value>,
    pub is_active: bool,
    pub must_change_password: bool,
    pub last_login_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub has_password: bool,
}

impl From<&UserRow> for AdminUser {
    fn from(user: &UserRow) -> Self {
        Self {
            id: user.id,
            email: user.email.clone(),
            name: user.name.clone(),
            department: user.department.clone(),
            role: user.role.clone(),
            admin_scope: user.admin_scope.clone(),
            is_active: user.is_active,
            must_change_password: user.must_change_password,
            last_login_at: user.last_login_at,
            created_at: user.created_at,
            updated_at: user.updated_at,
            has_password: false,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserDetail {
    #[serde(flatten)]
    pub user: AdminUser,
    pub group_ids: Vec<Uuid>,
    pub app_ids: Vec<Uuid>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserPage {
    pub users: Vec<AdminUser>,
    pub total: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportErrorRow {
    pub line: usize,
    pub email: Option<String>,
    pub message: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub created: i64,
    pub updated: i64,
    pub unchanged: i64,
    pub errors: Vec<ImportErrorRow>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditRow {
    pub id: i64,
    pub actor_user_id: Option<Uuid>,
    pub actor_email: Option<String>,
    pub action: String,
    pub entity_type: String,
    pub entity_id: Option<Uuid>,
    pub before: Option<Value>,
    pub after: Option<Value>,
    pub created_at: DateTime<Utc>,
}

impl From<&crate::store::AuditRow> for AuditRow {
    fn from(row: &crate::store::AuditRow) -> Self {
        Self {
            id: row.id,
            actor_user_id: row.actor_user_id,
            actor_email: row.actor_email.clone(),
            action: row.action.clone(),
            entity_type: row.entity_type.clone(),
            entity_id: row.entity_id,
            before: row.before.clone(),
            after: row.after.clone(),
            created_at: row.created_at,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditPage {
    pub entries: Vec<AuditRow>,
    pub total: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Setting {
    pub key: String,
    pub value: Value,
    pub updated_at: DateTime<Utc>,
    pub updated_by: Option<Uuid>,
}

impl From<&SettingRow> for Setting {
    fn from(row: &SettingRow) -> Self {
        Self {
            key: row.key.clone(),
            value: row.value.clone(),
            updated_at: row.updated_at,
            updated_by: row.updated_by,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub users: i64,
    pub active_users: i64,
    pub apps: i64,
    pub enabled_apps: i64,
    pub groups: i64,
    pub logins_today: i64,
    pub revision: i64,
}
