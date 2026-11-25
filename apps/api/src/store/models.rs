use std::{collections::HashMap, time::SystemTime};

use chrono::{DateTime, Utc};
use serde_json::Value;
use uuid::Uuid;

// ------------------------------------------------------------------ scalar helpers

pub(crate) fn dt(value: &str) -> Option<DateTime<Utc>> {
    if value.trim().is_empty() {
        return None;
    }
    DateTime::parse_from_rfc3339(value.trim())
        .ok()
        .map(|parsed| parsed.with_timezone(&Utc))
}

pub(crate) fn dt_str(value: DateTime<Utc>) -> String {
    value.to_rfc3339()
}

pub(crate) fn opt_dt_str(value: Option<DateTime<Utc>>) -> String {
    value.map(dt_str).unwrap_or_default()
}

pub(crate) fn opt_str(value: Option<String>) -> String {
    value.unwrap_or_default()
}

pub(crate) fn json_str(value: Option<&Value>) -> String {
    match value {
        None => String::new(),
        Some(Value::Null) => String::new(),
        Some(value) => value.to_string(),
    }
}

pub(crate) fn parse_json(value: &str) -> Option<Value> {
    if value.trim().is_empty() {
        None
    } else {
        serde_json::from_str(value).ok()
    }
}

pub(crate) fn bool_str(value: bool) -> String {
    if value { "1" } else { "0" }.to_string()
}

pub(crate) fn parse_bool(value: &str) -> bool {
    matches!(value.trim(), "1" | "true" | "TRUE" | "True" | "yes")
}

pub(crate) fn opt_uuid(value: &str) -> Option<Uuid> {
    if value.trim().is_empty() {
        None
    } else {
        Uuid::parse_str(value.trim()).ok()
    }
}

pub(crate) fn uuid_str(value: Uuid) -> String {
    value.to_string()
}

pub(crate) fn opt_uuid_str(value: Option<Uuid>) -> String {
    value.map(uuid_str).unwrap_or_default()
}

// ------------------------------------------------------------------ rows

#[derive(Debug, Clone)]
pub struct UserRow {
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
}

#[derive(Debug, Clone)]
pub struct DeletedUserRow {
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
    pub deleted_at: DateTime<Utc>,
    pub deleted_by: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GroupRow {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub auto_department: Option<String>,
    pub is_all_users: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct GroupMemberRow {
    pub group_id: Uuid,
    pub user_id: Uuid,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct DepartmentRow {
    pub id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct AppDepartmentRow {
    pub app_id: Uuid,
    pub department_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct AppGroupRow {
    pub app_id: Uuid,
    pub group_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct UserAppRow {
    pub user_id: Uuid,
    pub app_id: Uuid,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct AppRow {
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
}

#[derive(Debug, Clone)]
pub struct SettingRow {
    pub key: String,
    pub value: Value,
    pub updated_at: DateTime<Utc>,
    pub updated_by: Option<Uuid>,
}

#[derive(Debug, Clone)]
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

// ------------------------------------------------------------------ in-memory snapshot

/// A complete, immutable snapshot of every CSV file. Hand edits are picked up by
/// re-reading all files whenever any mtime changes.
#[derive(Debug, Default)]
pub struct Data {
    pub users: Vec<UserRow>,
    pub deleted_users: Vec<DeletedUserRow>,
    pub departments: Vec<DepartmentRow>,
    pub app_departments: Vec<AppDepartmentRow>,
    pub groups: Vec<GroupRow>,
    pub group_members: Vec<GroupMemberRow>,
    pub apps: Vec<AppRow>,
    pub app_groups: Vec<AppGroupRow>,
    pub user_apps: Vec<UserAppRow>,
    pub settings: Vec<SettingRow>,
    pub audit: Vec<AuditRow>,
    pub revision: i64,
    /// mtime of each watched file at the time this snapshot was loaded.
    pub signatures: HashMap<String, Option<SystemTime>>,
}

impl Data {
    pub fn setting(&self, key: &str) -> Option<&Value> {
        self.settings
            .iter()
            .find(|setting| setting.key == key)
            .map(|setting| &setting.value)
    }

    pub fn setting_i64(&self, key: &str, default: i64) -> i64 {
        match self.setting(key) {
            Some(Value::Number(number)) => number.as_i64().unwrap_or(default),
            Some(Value::Bool(flag)) => {
                if *flag {
                    1
                } else {
                    0
                }
            }
            Some(Value::String(text)) => text.trim().parse().unwrap_or(default),
            _ => default,
        }
    }

    pub fn setting_str(&self, key: &str) -> Option<String> {
        self.setting(key).and_then(|value| match value {
            Value::String(text) => Some(text.clone()),
            Value::Null => None,
            other => Some(other.to_string()),
        })
    }

    pub fn user_by_id(&self, id: Uuid) -> Option<&UserRow> {
        self.users.iter().find(|user| user.id == id)
    }

    pub fn user_by_email(&self, email: &str) -> Option<&UserRow> {
        let email = email.trim().to_lowercase();
        self.users
            .iter()
            .find(|user| user.email.to_lowercase() == email)
    }

    /// Creates a department when a CSV or form introduces a name we do not know yet.
    /// Returns true when a row was added.
    pub fn ensure_department(&mut self, name: &str) -> bool {
        let name = name.trim();
        if name.is_empty()
            || self
                .departments
                .iter()
                .any(|department| department.name.eq_ignore_ascii_case(name))
        {
            return false;
        }

        let now = Utc::now();
        self.departments.push(DepartmentRow {
            id: Uuid::new_v4(),
            name: name.to_string(),
            created_at: now,
            updated_at: now,
        });
        true
    }

    pub fn department_by_id(&self, id: Uuid) -> Option<&DepartmentRow> {
        self.departments
            .iter()
            .find(|department| department.id == id)
    }

    pub fn department_ids_for_app(&self, app_id: Uuid) -> Vec<Uuid> {
        self.app_departments
            .iter()
            .filter(|grant| grant.app_id == app_id)
            .map(|grant| grant.department_id)
            .collect()
    }
    pub fn group_by_id(&self, id: Uuid) -> Option<&GroupRow> {
        self.groups.iter().find(|group| group.id == id)
    }

    pub fn app_by_id(&self, id: Uuid) -> Option<&AppRow> {
        self.apps.iter().find(|app| app.id == id)
    }

    pub fn live_app_by_id(&self, id: Uuid) -> Option<&AppRow> {
        self.app_by_id(id).filter(|app| app.deleted_at.is_none())
    }

    pub fn group_ids_for_user(&self, user_id: Uuid) -> Vec<Uuid> {
        self.group_members
            .iter()
            .filter(|member| member.user_id == user_id)
            .map(|member| member.group_id)
            .collect()
    }

    pub fn app_ids_for_user(&self, user_id: Uuid) -> Vec<Uuid> {
        self.user_apps
            .iter()
            .filter(|grant| grant.user_id == user_id)
            .map(|grant| grant.app_id)
            .collect()
    }

    pub fn group_ids_for_app(&self, app_id: Uuid) -> Vec<Uuid> {
        self.app_groups
            .iter()
            .filter(|grant| grant.app_id == app_id)
            .map(|grant| grant.group_id)
            .collect()
    }

    pub fn user_ids_for_app(&self, app_id: Uuid) -> Vec<Uuid> {
        self.user_apps
            .iter()
            .filter(|grant| grant.app_id == app_id)
            .map(|grant| grant.user_id)
            .collect()
    }

    /// Adds a fire-and-forget audit row. The caller is expected to hold the write lock.
    #[allow(clippy::too_many_arguments)]
    pub fn audit(
        &mut self,
        actor_user_id: Option<Uuid>,
        actor_email: Option<String>,
        action: &str,
        entity_type: &str,
        entity_id: Option<Uuid>,
        before: Option<Value>,
        after: Option<Value>,
    ) {
        let next_id = self.audit.iter().map(|entry| entry.id).max().unwrap_or(0) + 1;
        self.audit.push(AuditRow {
            id: next_id,
            actor_user_id,
            actor_email: actor_email.or_else(|| {
                actor_user_id
                    .and_then(|id| self.user_by_id(id))
                    .map(|user| user.email.clone())
            }),
            action: action.to_string(),
            entity_type: entity_type.to_string(),
            entity_id,
            before,
            after,
            created_at: Utc::now(),
        });
    }

    pub fn bump_revision(&mut self) -> i64 {
        self.revision += 1;
        self.revision
    }
}
