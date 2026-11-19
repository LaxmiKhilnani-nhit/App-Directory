#![allow(dead_code)]

use std::{fs, path::PathBuf, sync::Arc};

use axum::{
    body::Body,
    http::{HeaderMap, Request, StatusCode},
    Router,
};
use chrono::Utc;
use serde_json::{json, Value};
use tower::ServiceExt;
use uuid::Uuid;

use app_directory_api::build_router;
use app_directory_api::config::Config;
use app_directory_api::state::AppState;
use app_directory_api::store::{AppRow, DepartmentRow, GroupRow, Store, UserRow};

pub struct Harness {
    pub dir: PathBuf,
    pub state: AppState,
    pub app: Router,
}

impl Drop for Harness {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

pub fn setup() -> Harness {
    let dir = std::env::temp_dir().join(format!("appdir-csv-test-{}", Uuid::new_v4()));
    let _ = fs::remove_dir_all(&dir);

    let store = Arc::new(Store::open(&dir).expect("open temp CSV store"));
    let state = AppState::new(
        Config {
            data_dir: dir.clone(),
            ..Config::default()
        },
        store,
    );
    // Admin credentials live in a separate CSV. Pre-seed the accounts the HTTP tests use.
    let hash = app_directory_api::domain::admins::hash_password("anything");
    let mut admins = String::from("email;name;password_hash\n");
    for name in ["admin", "anyone", "laxmikhilnani"] {
        admins.push_str(&format!("{}@nhit.co.in;Test Admin;{hash}\n", name));
    }
    let _ = fs::write(dir.join("admins.csv"), admins);
    let app = build_router(state.clone());
    Harness { dir, state, app }
}

pub fn email(name: &str) -> String {
    format!("{name}@nhit.co.in")
}

pub fn add_user(h: &Harness, local: &str, department: Option<&str>, active: bool) -> Uuid {
    let now = Utc::now();
    let id = Uuid::new_v4();
    h.state
        .store
        .mutate(|data| {
            data.users.push(UserRow {
                id,
                email: email(local),
                name: local.to_string(),
                department: department.map(str::to_string),
                role: "user".to_string(),
                admin_scope: None,
                is_active: active,
                must_change_password: false,
                last_login_at: None,
                created_at: now,
                updated_at: now,
            });
            Ok(())
        })
        .expect("add user");
    id
}

pub fn add_admin_user(h: &Harness, local: &str, role: &str) -> Uuid {
    let now = Utc::now();
    let id = Uuid::new_v4();
    h.state
        .store
        .mutate(|data| {
            data.users.push(UserRow {
                id,
                email: email(local),
                name: local.to_string(),
                department: Some("IT".to_string()),
                role: role.to_string(),
                admin_scope: None,
                is_active: true,
                must_change_password: false,
                last_login_at: None,
                created_at: now,
                updated_at: now,
            });
            Ok(())
        })
        .expect("add admin user");
    id
}
pub fn add_group(h: &Harness, name: &str, auto_department: Option<&str>) -> Uuid {
    let now = Utc::now();
    let id = Uuid::new_v4();
    h.state
        .store
        .mutate(|data| {
            data.groups.push(GroupRow {
                id,
                name: name.to_string(),
                description: None,
                auto_department: auto_department.map(str::to_string),
                is_all_users: false,
                created_at: now,
                updated_at: now,
            });
            Ok(())
        })
        .expect("add group");
    id
}

pub fn add_department(h: &Harness, name: &str) -> Uuid {
    let now = Utc::now();
    let id = Uuid::new_v4();
    h.state
        .store
        .mutate(|data| {
            data.departments.push(DepartmentRow {
                id,
                name: name.to_string(),
                created_at: now,
                updated_at: now,
            });
            Ok(())
        })
        .expect("add department");
    id
}

pub fn grant_department(h: &Harness, department_id: Uuid, app_id: Uuid) {
    h.state
        .store
        .mutate(|data| {
            data.app_departments
                .push(app_directory_api::store::AppDepartmentRow {
                    department_id,
                    app_id,
                });
            Ok(())
        })
        .expect("grant department");
}
pub fn add_app(h: &Harness, name: &str, sort_order: i32) -> Uuid {
    let now = Utc::now();
    let id = Uuid::new_v4();
    h.state
        .store
        .mutate(|data| {
            data.apps.push(AppRow {
                id,
                name: name.to_string(),
                url: format!("http://127.0.0.1:8080/{name}"),
                description: None,
                category: None,
                color: None,
                sort_order,
                is_enabled: true,
                offline_visible: true,
                icon_source: "none".to_string(),
                icon_mime: None,
                icon_fetched_at: None,
                icon_etag: None,
                created_at: now,
                updated_at: now,
                deleted_at: None,
            });
            Ok(())
        })
        .expect("add app");
    id
}

pub fn grant_user(h: &Harness, user_id: Uuid, app_id: Uuid) {
    h.state
        .store
        .mutate(|data| {
            data.user_apps.push(app_directory_api::store::UserAppRow {
                user_id,
                app_id,
                created_at: Utc::now(),
            });
            Ok(())
        })
        .expect("grant user");
}

pub fn add_member(h: &Harness, group_id: Uuid, user_id: Uuid) {
    h.state
        .store
        .mutate(|data| {
            data.group_members
                .push(app_directory_api::store::GroupMemberRow {
                    group_id,
                    user_id,
                    created_at: Utc::now(),
                });
            Ok(())
        })
        .expect("add member");
}

pub fn grant_group(h: &Harness, group_id: Uuid, app_id: Uuid) {
    h.state
        .store
        .mutate(|data| {
            data.app_groups
                .push(app_directory_api::store::AppGroupRow { group_id, app_id });
            Ok(())
        })
        .expect("grant group");
}

pub fn set_app_enabled(h: &Harness, app_id: Uuid, enabled: bool) {
    h.state
        .store
        .mutate(|data| {
            if let Some(app) = data.apps.iter_mut().find(|app| app.id == app_id) {
                app.is_enabled = enabled;
            }
            Ok(())
        })
        .expect("set app enabled");
}

pub fn set_app_deleted(h: &Harness, app_id: Uuid) {
    h.state
        .store
        .mutate(|data| {
            if let Some(app) = data.apps.iter_mut().find(|app| app.id == app_id) {
                app.deleted_at = Some(Utc::now());
            }
            Ok(())
        })
        .expect("soft delete app");
}

#[derive(Default)]
pub struct SendOptions<'a> {
    pub identity: Option<&'a str>,
    pub token: Option<&'a str>,
    pub body: Option<Value>,
    pub method: &'a str,
}

pub async fn call(
    h: &Harness,
    uri: &str,
    options: SendOptions<'_>,
) -> (StatusCode, HeaderMap, Value) {
    let mut builder = Request::builder()
        .method(options.method)
        .uri(uri)
        .header("accept", "application/json");

    if let Some(identity) = options.identity {
        builder = builder.header("x-windows-identity", identity);
    }
    if let Some(token) = options.token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }

    let request = match options.body {
        Some(body) => builder
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .expect("build request"),
        None => builder.body(Body::empty()).expect("build request"),
    };

    let response = h.app.clone().oneshot(request).await.expect("call handler");
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read body");
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| json!({ "raw": String::from_utf8_lossy(&bytes) }))
    };
    (status, headers, value)
}

pub async fn get(h: &Harness, uri: &str, identity: Option<&str>) -> (StatusCode, Value) {
    let (status, _, body) = call(
        h,
        uri,
        SendOptions {
            method: "GET",
            identity,
            ..Default::default()
        },
    )
    .await;
    (status, body)
}

pub async fn json(
    h: &Harness,
    method: &str,
    uri: &str,
    identity: Option<&str>,
    body: Value,
) -> (StatusCode, Value) {
    let (status, _, body) = call(
        h,
        uri,
        SendOptions {
            method,
            identity,
            body: Some(body),
            ..Default::default()
        },
    )
    .await;
    (status, body)
}
