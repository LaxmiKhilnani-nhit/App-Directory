//! CSV-backed persistence.
//!
//! The whole data set is small (hundreds of rows), so every read uses one in-memory
//! snapshot behind an `RwLock` and every write reloads, mutates, and rewrites all files.
//! Writes are serialised by a process-wide mutex and each file is replaced atomically via a
//! `.tmp` sibling + rename. If an operator hand-edits a CSV file, its mtime changes and the
//! next read transparently reloads everything from disk.

pub mod csv;
pub mod models;

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, RwLock, RwLockReadGuard},
    time::SystemTime,
};

use anyhow::Context;
use chrono::Utc;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::error::ApiError;

pub use models::{
    AppGroupRow, AppRow, AuditRow, Data, GroupMemberRow, GroupRow, SettingRow, UserAppRow, UserRow,
};

const USERS: &str = "users.csv";
const GROUPS: &str = "groups.csv";
const GROUP_MEMBERS: &str = "group_members.csv";
const APPS: &str = "apps.csv";
const APP_GROUPS: &str = "app_groups.csv";
const USER_APPS: &str = "user_apps.csv";
const SETTINGS: &str = "settings.csv";
const AUDIT: &str = "audit.csv";
const REVISION: &str = "revision.csv";

const FILES: [&str; 9] = [
    USERS,
    GROUPS,
    GROUP_MEMBERS,
    APPS,
    APP_GROUPS,
    USER_APPS,
    SETTINGS,
    AUDIT,
    REVISION,
];

pub const ALL_USERS_GROUP: &str = "All Users";

/// Deterministic id for the built-in All Users group, so repeated startups do not create
/// duplicate groups when the CSV is hand-edited.
pub const ALL_USERS_GROUP_ID: Uuid = Uuid::from_u128(0x8f14e45f_ea0e_3a2a_9c1d_3f6a1b2c4d5e);

/// Values inserted on first start. Existing keys are never overwritten, so restarting the
/// container cannot undo an admin's changes.
pub fn default_settings() -> Vec<(&'static str, Value)> {
    vec![
        ("app_name", json!("App Directory")),
        ("sync_interval_hours", json!(4)),
        ("offline_grace_days", json!(7)),
        ("announcement", Value::Null),
        ("mail_domain", json!("nhit.co.in")),
        ("default_identity", json!("")),
        ("update_enabled", json!(0)),
        ("update_version", json!("")),
        ("update_sha256", json!("")),
        ("update_url", json!("/download/app-directory.exe")),
        // Kept so the admin settings screen and SMTP configuration stay compatible; no codes
        // or emails are generated any more.
        ("smtp_enabled", json!(false)),
        ("smtp_host", json!("")),
        ("smtp_port", json!(587)),
        ("smtp_username", json!("")),
        ("smtp_password", json!("")),
        ("smtp_from_email", json!("")),
        ("smtp_from_name", json!("App Directory")),
        ("smtp_starttls", json!(true)),
    ]
}

pub struct Store {
    dir: PathBuf,
    data: RwLock<Data>,
    /// Serialises writers and reloads so two requests cannot interleave file rewrites.
    write_mutex: Mutex<()>,
}

impl Store {
    /// Opens (and initialises) the CSV directory.
    pub fn open(dir: impl Into<PathBuf>) -> anyhow::Result<Self> {
        let dir = dir.into();
        fs::create_dir_all(dir.join("icons"))
            .with_context(|| format!("creating {}", dir.join("icons").display()))?;
        for name in FILES {
            let path = dir.join(name);
            if name == REVISION {
                if !path.exists() || fs::metadata(&path)?.len() == 0 {
                    csv::atomic_write(&path, b"1\n")?;
                }
            } else {
                csv::ensure_file(&path, header_for(name))?;
            }
        }

        let signatures = Self::signatures(&dir);
        let mut data = Self::load_from(&dir, signatures)?;

        // Persist seeded defaults and the All Users group on first start, so every CSV file
        // is immediately hand-editable (an empty settings.csv with only a header is useless).
        Self::save_all(&dir, &data)?;
        data.signatures = Self::signatures(&dir);

        Ok(Self {
            dir,
            data: RwLock::new(data),
            write_mutex: Mutex::new(()),
        })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn icons_dir(&self) -> PathBuf {
        self.dir.join("icons")
    }

    pub fn icon_path(&self, app_id: Uuid) -> PathBuf {
        self.icons_dir().join(format!("{app_id}.bin"))
    }

    pub fn has_icon(&self, app_id: Uuid) -> bool {
        self.icon_path(app_id).is_file()
    }

    pub fn read_icon(&self, app_id: Uuid) -> Option<Vec<u8>> {
        fs::read(self.icon_path(app_id)).ok()
    }

    /// Writes icon bytes atomically. Callers should already hold the write mutex; this
    /// helper takes its own lock so it is safe from the icon handler as well.
    pub fn write_icon(&self, app_id: Uuid, bytes: &[u8]) -> Result<(), ApiError> {
        let _guard = self.write_mutex.lock().unwrap_or_else(|p| p.into_inner());
        csv::atomic_write(&self.icon_path(app_id), bytes)?;
        Ok(())
    }

    /// Returns a read guard, reloading from disk first if any watched file changed.
    pub fn read(&self) -> RwLockReadGuard<'_, Data> {
        self.maybe_reload();
        self.data.read().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Runs a mutation against a fresh snapshot and persists every file on success.
    pub fn mutate<T>(&self, change: impl FnOnce(&mut Data) -> Result<T, ApiError>) -> Result<T, ApiError> {
        let _write_guard = self.write_mutex.lock().unwrap_or_else(|p| p.into_inner());

        let signatures = Self::signatures(&self.dir);
        let mut fresh = Self::load_from(&self.dir, signatures.clone())?;

        let result = change(&mut fresh)?;
        Self::save_all(&self.dir, &fresh)?;
        fresh.signatures = Self::signatures(&self.dir);

        let mut data = self.data.write().unwrap_or_else(|p| p.into_inner());
        *data = fresh;
        Ok(result)
    }

    // ---------------------------------------------------------------- internals

    fn maybe_reload(&self) {
        let signatures = Self::signatures(&self.dir);
        {
            let current = self.data.read().unwrap_or_else(|p| p.into_inner());
            if current.signatures == signatures {
                return;
            }
        }

        let _write_guard = self.write_mutex.lock().unwrap_or_else(|p| p.into_inner());
        let mut data = self.data.write().unwrap_or_else(|p| p.into_inner());
        if data.signatures == signatures {
            return;
        }

        match Self::load_from(&self.dir, signatures) {
            Ok(fresh) => *data = fresh,
            Err(error) => tracing::warn!(%error, "could not reload CSV files; keeping last snapshot"),
        }
    }

    fn signatures(dir: &Path) -> HashMap<String, Option<SystemTime>> {
        FILES
            .iter()
            .map(|name| {
                let modified = fs::metadata(dir.join(name))
                    .ok()
                    .and_then(|meta| meta.modified().ok());
                ((*name).to_string(), modified)
            })
            .collect()
    }

    fn save_all(dir: &Path, data: &Data) -> Result<(), ApiError> {
        let mut users = Vec::new();
        for row in &data.users {
            users.push(vec![
                models::uuid_str(row.id),
                row.email.clone(),
                row.name.clone(),
                models::opt_str(row.department.clone()),
                row.role.clone(),
                models::json_str(row.admin_scope.as_ref()),
                models::bool_str(row.is_active),
                models::bool_str(row.must_change_password),
                models::opt_dt_str(row.last_login_at),
                models::dt_str(row.created_at),
                models::dt_str(row.updated_at),
            ]);
        }
        csv::write_rows(&dir.join(USERS), header_for(USERS), &users)?;

        let mut groups = Vec::new();
        for row in &data.groups {
            groups.push(vec![
                models::uuid_str(row.id),
                row.name.clone(),
                models::opt_str(row.description.clone()),
                models::opt_str(row.auto_department.clone()),
                models::bool_str(row.is_all_users),
                models::dt_str(row.created_at),
                models::dt_str(row.updated_at),
            ]);
        }
        csv::write_rows(&dir.join(GROUPS), header_for(GROUPS), &groups)?;

        let mut members = Vec::new();
        for row in &data.group_members {
            members.push(vec![
                models::uuid_str(row.group_id),
                models::uuid_str(row.user_id),
                models::dt_str(row.created_at),
            ]);
        }
        csv::write_rows(&dir.join(GROUP_MEMBERS), header_for(GROUP_MEMBERS), &members)?;

        let mut apps = Vec::new();
        for row in &data.apps {
            apps.push(vec![
                models::uuid_str(row.id),
                row.name.clone(),
                row.url.clone(),
                models::opt_str(row.description.clone()),
                models::opt_str(row.category.clone()),
                models::opt_str(row.color.clone()),
                row.sort_order.to_string(),
                models::bool_str(row.is_enabled),
                models::bool_str(row.offline_visible),
                row.icon_source.clone(),
                models::opt_str(row.icon_mime.clone()),
                models::opt_dt_str(row.icon_fetched_at),
                models::opt_str(row.icon_etag.clone()),
                models::dt_str(row.created_at),
                models::dt_str(row.updated_at),
                models::opt_dt_str(row.deleted_at),
            ]);
        }
        csv::write_rows(&dir.join(APPS), header_for(APPS), &apps)?;

        let mut app_groups = Vec::new();
        for row in &data.app_groups {
            app_groups.push(vec![
                models::uuid_str(row.app_id),
                models::uuid_str(row.group_id),
            ]);
        }
        csv::write_rows(&dir.join(APP_GROUPS), header_for(APP_GROUPS), &app_groups)?;

        let mut user_apps = Vec::new();
        for row in &data.user_apps {
            user_apps.push(vec![
                models::uuid_str(row.user_id),
                models::uuid_str(row.app_id),
                models::dt_str(row.created_at),
            ]);
        }
        csv::write_rows(&dir.join(USER_APPS), header_for(USER_APPS), &user_apps)?;

        let mut settings = Vec::new();
        for row in &data.settings {
            settings.push(vec![
                row.key.clone(),
                models::json_str(Some(&row.value)),
                models::dt_str(row.updated_at),
                models::opt_uuid_str(row.updated_by),
            ]);
        }
        csv::write_rows(&dir.join(SETTINGS), header_for(SETTINGS), &settings)?;

        let mut audit = Vec::new();
        for row in &data.audit {
            audit.push(vec![
                row.id.to_string(),
                models::opt_uuid_str(row.actor_user_id),
                models::opt_str(row.actor_email.clone()),
                row.action.clone(),
                row.entity_type.clone(),
                models::opt_uuid_str(row.entity_id),
                models::json_str(row.before.as_ref()),
                models::json_str(row.after.as_ref()),
                models::dt_str(row.created_at),
            ]);
        }
        csv::write_rows(&dir.join(AUDIT), header_for(AUDIT), &audit)?;

        csv::atomic_write(
            &dir.join(REVISION),
            format!("{}\n", data.revision).as_bytes(),
        )?;

        Ok(())
    }

    fn load_from(dir: &Path, signatures: HashMap<String, Option<SystemTime>>) -> anyhow::Result<Data> {
        let mut data = Data {
            signatures,
            ..Data::default()
        };

        for row in csv::read_rows(&dir.join(USERS)).unwrap_or_default() {
            let Some(id) = models::opt_uuid(row.first().map(String::as_str).unwrap_or("")) else {
                continue;
            };
            let field = |index: usize| row.get(index).cloned().unwrap_or_default();
            data.users.push(UserRow {
                id,
                email: field(1).to_lowercase(),
                name: field(2),
                department: non_empty(field(3)),
                role: non_empty(field(4)).unwrap_or_else(|| "user".to_string()),
                admin_scope: models::parse_json(&field(5)),
                is_active: models::parse_bool(&field(6)),
                must_change_password: models::parse_bool(&field(7)),
                last_login_at: models::dt(&field(8)),
                created_at: models::dt(&field(9)).unwrap_or_else(Utc::now),
                updated_at: models::dt(&field(10)).unwrap_or_else(Utc::now),
            });
        }

        for row in csv::read_rows(&dir.join(GROUPS)).unwrap_or_default() {
            let Some(id) = models::opt_uuid(row.first().map(String::as_str).unwrap_or("")) else {
                continue;
            };
            let field = |index: usize| row.get(index).cloned().unwrap_or_default();
            data.groups.push(GroupRow {
                id,
                name: field(1),
                description: non_empty(field(2)),
                auto_department: non_empty(field(3)),
                is_all_users: models::parse_bool(&field(4)),
                created_at: models::dt(&field(5)).unwrap_or_else(Utc::now),
                updated_at: models::dt(&field(6)).unwrap_or_else(Utc::now),
            });
        }

        for row in csv::read_rows(&dir.join(GROUP_MEMBERS)).unwrap_or_default() {
            let (Some(group_id), Some(user_id)) = (
                models::opt_uuid(row.first().map(String::as_str).unwrap_or("")),
                models::opt_uuid(row.get(1).map(String::as_str).unwrap_or("")),
            ) else {
                continue;
            };
            data.group_members.push(GroupMemberRow {
                group_id,
                user_id,
                created_at: models::dt(row.get(2).map(String::as_str).unwrap_or(""))
                    .unwrap_or_else(Utc::now),
            });
        }

        for row in csv::read_rows(&dir.join(APPS)).unwrap_or_default() {
            let Some(id) = models::opt_uuid(row.first().map(String::as_str).unwrap_or("")) else {
                continue;
            };
            let field = |index: usize| row.get(index).cloned().unwrap_or_default();
            data.apps.push(AppRow {
                id,
                name: field(1),
                url: field(2),
                description: non_empty(field(3)),
                category: non_empty(field(4)),
                color: non_empty(field(5)),
                sort_order: field(6).parse().unwrap_or(100),
                is_enabled: models::parse_bool(&field(7)),
                offline_visible: models::parse_bool(&field(8)),
                icon_source: non_empty(field(9)).unwrap_or_else(|| "none".to_string()),
                icon_mime: non_empty(field(10)),
                icon_fetched_at: models::dt(&field(11)),
                icon_etag: non_empty(field(12)),
                created_at: models::dt(&field(13)).unwrap_or_else(Utc::now),
                updated_at: models::dt(&field(14)).unwrap_or_else(Utc::now),
                deleted_at: models::dt(&field(15)),
            });
        }

        for row in csv::read_rows(&dir.join(APP_GROUPS)).unwrap_or_default() {
            let (Some(app_id), Some(group_id)) = (
                models::opt_uuid(row.first().map(String::as_str).unwrap_or("")),
                models::opt_uuid(row.get(1).map(String::as_str).unwrap_or("")),
            ) else {
                continue;
            };
            data.app_groups.push(AppGroupRow { app_id, group_id });
        }

        for row in csv::read_rows(&dir.join(USER_APPS)).unwrap_or_default() {
            let (Some(user_id), Some(app_id)) = (
                models::opt_uuid(row.first().map(String::as_str).unwrap_or("")),
                models::opt_uuid(row.get(1).map(String::as_str).unwrap_or("")),
            ) else {
                continue;
            };
            data.user_apps.push(UserAppRow {
                user_id,
                app_id,
                created_at: models::dt(row.get(2).map(String::as_str).unwrap_or(""))
                    .unwrap_or_else(Utc::now),
            });
        }

        for row in csv::read_rows(&dir.join(SETTINGS)).unwrap_or_default() {
            let key = row.first().cloned().unwrap_or_default();
            if key.trim().is_empty() {
                continue;
            }
            let value = models::parse_json(row.get(1).map(String::as_str).unwrap_or(""))
                .unwrap_or(Value::Null);
            data.settings.push(SettingRow {
                key,
                value,
                updated_at: models::dt(row.get(2).map(String::as_str).unwrap_or(""))
                    .unwrap_or_else(Utc::now),
                updated_by: models::opt_uuid(row.get(3).map(String::as_str).unwrap_or("")),
            });
        }

        for row in csv::read_rows(&dir.join(AUDIT)).unwrap_or_default() {
            let id: i64 = match row.first().and_then(|value| value.parse().ok()) {
                Some(id) => id,
                None => continue,
            };
            let field = |index: usize| row.get(index).cloned().unwrap_or_default();
            data.audit.push(AuditRow {
                id,
                actor_user_id: models::opt_uuid(&field(1)),
                actor_email: non_empty(field(2)),
                action: field(3),
                entity_type: field(4),
                entity_id: models::opt_uuid(&field(5)),
                before: models::parse_json(&field(6)),
                after: models::parse_json(&field(7)),
                created_at: models::dt(&field(8)).unwrap_or_else(Utc::now),
            });
        }

        data.revision = fs::read_to_string(dir.join(REVISION))
            .ok()
            .and_then(|text| text.trim().lines().next().unwrap_or("").trim().parse().ok())
            .unwrap_or(1);

        // Structural defaults. Hand-edited files are never overwritten: we only insert rows
        // that are missing.
        if !data.groups.iter().any(|group| group.is_all_users) {
            let now = Utc::now();
            data.groups.push(GroupRow {
                id: ALL_USERS_GROUP_ID,
                name: ALL_USERS_GROUP.to_string(),
                description: Some("Every active user".to_string()),
                auto_department: None,
                is_all_users: true,
                created_at: now,
                updated_at: now,
            });
        }
        for (key, value) in default_settings() {
            if !data.settings.iter().any(|setting| setting.key == key) {
                data.settings.push(SettingRow {
                    key: key.to_string(),
                    value,
                    updated_at: Utc::now(),
                    updated_by: None,
                });
            }
        }

        Ok(data)
    }
}

fn header_for(name: &str) -> &'static str {
    match name {
        USERS => "id,email,name,department,role,adminScope,isActive,mustChangePassword,lastLoginAt,createdAt,updatedAt",
        GROUPS => "id,name,description,autoDepartment,isAllUsers,createdAt,updatedAt",
        GROUP_MEMBERS => "groupId,userId,createdAt",
        APPS => "id,name,url,description,category,color,sortOrder,isEnabled,offlineVisible,iconSource,iconMime,iconFetchedAt,iconEtag,createdAt,updatedAt,deletedAt",
        APP_GROUPS => "appId,groupId",
        USER_APPS => "userId,appId,createdAt",
        SETTINGS => "key,value,updatedAt,updatedBy",
        AUDIT => "id,actorUserId,actorEmail,action,entityType,entityId,before,after,createdAt",
        _ => "value",
    }
}

pub fn non_empty(value: String) -> Option<String> {
    let trimmed = value.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}
