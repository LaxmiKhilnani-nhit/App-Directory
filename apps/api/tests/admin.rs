//! Admin API integration tests.
//!
//! Each test owns a disposable database (`#[sqlx::test]`) and drives the real router, so
//! role gates, validation, audit rows and revision bumps are all exercised end to end.

use axum::{body::Body, http::Request, Router};
use chrono::Utc;
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

use app_directory_api::build_router;
use app_directory_api::config::Config;
use app_directory_api::domain::auth;
use app_directory_api::domain::password;
use app_directory_api::state::AppState;

const PASSWORD: &str = "correct-horse-battery";
const NEW_PASSWORD: &str = "brand-new-password-2026";
const PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
];

fn app(pool: PgPool) -> Router {
    build_router(AppState::new(
        Config {
            session_ttl_days: 30,
            ..Config::default()
        },
        Some(pool),
    ))
}

async fn insert_user(pool: &PgPool, email: &str, role: &str) -> Uuid {
    insert_user_with_scope(pool, email, role, None).await
}

async fn insert_user_with_scope(
    pool: &PgPool,
    email: &str,
    role: &str,
    scope: Option<Value>,
) -> Uuid {
    let hash = password::hash_password(PASSWORD).expect("hash password");

    sqlx::query_scalar(
        "insert into users (email, name, department, role, is_active, password_hash,
                            must_change_password, admin_scope)
         values (lower($1), $2, 'Finance', $3, true, $4, false, $5)
         returning id",
    )
    .bind(email)
    .bind(email)
    .bind(role)
    .bind(hash)
    .bind(scope)
    .fetch_one(pool)
    .await
    .expect("insert user")
}

async fn token_for(pool: &PgPool, user_id: Uuid) -> String {
    auth::create_session(pool, user_id, 30, Some("test"))
        .await
        .expect("create session")
        .0
}

async fn send(
    app: &Router,
    method: &str,
    uri: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (axum::http::StatusCode, Value) {
    let (status, _, bytes) = send_raw(app, method, uri, token, body, None).await;

    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };

    (status, value)
}

async fn send_raw(
    app: &Router,
    method: &str,
    uri: &str,
    token: Option<&str>,
    body: Option<Value>,
    content_type: Option<&str>,
) -> (axum::http::StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let mut builder = Request::builder().method(method).uri(uri);

    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }

    let request = match body {
        Some(value) => builder
            .header("content-type", content_type.unwrap_or("application/json"))
            .body(Body::from(match value {
                Value::String(text) => text,
                other => other.to_string(),
            }))
            .expect("build request"),
        None => builder.body(Body::empty()).expect("build request"),
    };

    let response = app.clone().oneshot(request).await.expect("call handler");
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read body")
        .to_vec();

    (status, headers, bytes)
}

/// Same as [`send_raw`], but with an arbitrary byte body so binary uploads survive intact.
async fn send_bytes(
    app: &Router,
    method: &str,
    uri: &str,
    token: Option<&str>,
    bytes: Vec<u8>,
    content_type: Option<&str>,
) -> (axum::http::StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let mut builder = Request::builder().method(method).uri(uri);

    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }

    let request = builder
        .header(
            "content-type",
            content_type.unwrap_or("application/octet-stream"),
        )
        .body(Body::from(bytes))
        .expect("build request");

    let response = app.clone().oneshot(request).await.expect("call handler");
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read body")
        .to_vec();

    (status, headers, bytes)
}

async fn revision(pool: &PgPool) -> i64 {
    sqlx::query_scalar("select revision from sync_state where id = 1")
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn create_app(app: &Router, token: &str, name: &str, category: &str) -> Value {
    let (status, body) = send(
        app,
        "POST",
        "/api/v1/admin/apps",
        Some(token),
        Some(json!({
            "name": name,
            "url": "http://172.168.50.50:8080/app",
            "category": category,
            "color": "#2F6FED",
            "sortOrder": 10,
        })),
    )
    .await;

    assert_eq!(status, axum::http::StatusCode::CREATED, "create app: {body}");
    body
}

#[sqlx::test]
async fn non_admins_cannot_reach_the_admin_api(pool: PgPool) {
    let user = insert_user(&pool, "person@example.com", "user").await;
    let token = token_for(&pool, user).await;
    let app = app(pool.clone());

    let (status, body) = send(&app, "GET", "/api/v1/admin/apps", Some(&token), None).await;
    assert_eq!(status, axum::http::StatusCode::FORBIDDEN);
    assert_eq!(body["error"]["code"], "forbidden");

    let (status, _) = send(&app, "GET", "/api/v1/admin/apps", None, None).await;
    assert_eq!(status, axum::http::StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn app_crud_bumps_revision_and_writes_audit(pool: PgPool) {
    let admin = insert_user(&pool, "admin@example.com", "super_admin").await;
    let token = token_for(&pool, admin).await;
    let app = app(pool.clone());

    assert_eq!(revision(&pool).await, 1);

    let created = create_app(&app, &token, "Payroll", "Finance").await;
    let id = created["id"].as_str().unwrap().to_string();
    assert_eq!(created["category"], "Finance");
    assert_eq!(created["isEnabled"], true);
    assert_eq!(revision(&pool).await, 2);

    let (status, updated) = send(
        &app,
        "PATCH",
        &format!("/api/v1/admin/apps/{id}"),
        Some(&token),
        Some(json!({ "name": "Payroll 2", "description": "Monthly payroll" })),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK, "patch: {updated}");
    assert_eq!(updated["name"], "Payroll 2");
    assert_eq!(updated["description"], "Monthly payroll");
    assert_eq!(revision(&pool).await, 3);

    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/v1/admin/apps/{id}"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::NO_CONTENT);
    assert_eq!(revision(&pool).await, 4);

    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/v1/admin/apps/{id}"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::NOT_FOUND);

    let actions: Vec<String> = sqlx::query_scalar("select action from audit_log order by id")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(actions, vec!["app.create", "app.update", "app.delete"]);
}

#[sqlx::test]
async fn app_validation_rejects_bad_input(pool: PgPool) {
    let admin = insert_user(&pool, "admin@example.com", "super_admin").await;
    let token = token_for(&pool, admin).await;
    let app = app(pool.clone());

    let (status, body) = send(
        &app,
        "POST",
        "/api/v1/admin/apps",
        Some(&token),
        Some(json!({ "name": "Payroll", "url": "not-a-url" })),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "invalid_url");

    let (status, body) = send(
        &app,
        "POST",
        "/api/v1/admin/apps",
        Some(&token),
        Some(json!({
            "name": "Payroll",
            "url": "http://example.com",
            "color": "blue"
        })),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "invalid_color");
}

#[sqlx::test]
async fn groups_crud_and_members(pool: PgPool) {
    let admin = insert_user(&pool, "admin@example.com", "super_admin").await;
    let first = insert_user(&pool, "first@example.com", "user").await;
    let second = insert_user(&pool, "second@example.com", "user").await;
    let token = token_for(&pool, admin).await;
    let app = app(pool.clone());

    let (status, group) = send(
        &app,
        "POST",
        "/api/v1/admin/groups",
        Some(&token),
        Some(json!({ "name": "Finance", "description": "Finance team" })),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::CREATED, "group: {group}");

    let group_id = group["id"].as_str().unwrap().to_string();

    let (status, group) = send(
        &app,
        "PUT",
        &format!("/api/v1/admin/groups/{group_id}/members"),
        Some(&token),
        Some(json!({ "userIds": [first, second, first] })),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(group["memberCount"], 2);
    assert_eq!(group["userIds"].as_array().unwrap().len(), 2);

    let (_, detail) = send(
        &app,
        "GET",
        &format!("/api/v1/admin/groups/{group_id}"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(detail["name"], "Finance");

    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/v1/admin/groups/{group_id}"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::NO_CONTENT);

    let (status, body) = send(
        &app,
        "POST",
        "/api/v1/admin/groups",
        Some(&token),
        Some(json!({ "name": "Finance" })),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::CREATED, "recreated: {body}");
}

#[sqlx::test]
async fn all_users_group_is_protected(pool: PgPool) {
    sqlx::query("insert into groups (name, is_all_users) values ('All Users', true)")
        .execute(&pool)
        .await
        .unwrap();

    let admin = insert_user(&pool, "admin@example.com", "super_admin").await;
    let token = token_for(&pool, admin).await;
    let app = app(pool.clone());

    let (_, groups) = send(&app, "GET", "/api/v1/admin/groups", Some(&token), None).await;
    let all_users = groups
        .as_array()
        .unwrap()
        .iter()
        .find(|group| group["isAllUsers"] == true)
        .expect("All Users group")
        .clone();
    let id = all_users["id"].as_str().unwrap().to_string();

    let (status, body) = send(
        &app,
        "DELETE",
        &format!("/api/v1/admin/groups/{id}"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::CONFLICT);
    assert_eq!(body["error"]["code"], "builtin_group");

    let (status, body) = send(
        &app,
        "PUT",
        &format!("/api/v1/admin/groups/{id}/members"),
        Some(&token),
        Some(json!({ "userIds": [] })),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::CONFLICT);
    assert_eq!(body["error"]["code"], "derived_group");
}

#[sqlx::test]
async fn auto_department_group_counts_users_without_members(pool: PgPool) {
    let admin = insert_user(&pool, "admin@example.com", "super_admin").await;
    insert_user(&pool, "finance@example.com", "user").await;
    let token = token_for(&pool, admin).await;
    let app = app(pool.clone());

    let (status, group) = send(
        &app,
        "POST",
        "/api/v1/admin/groups",
        Some(&token),
        Some(json!({ "name": "All Finance", "autoDepartment": "Finance" })),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::CREATED);

    // The admin is in Finance too, so both accounts count.
    assert_eq!(group["memberCount"], 2);
    assert_eq!(group["autoDepartment"], "Finance");

    let id = group["id"].as_str().unwrap().to_string();
    let (status, body) = send(
        &app,
        "PUT",
        &format!("/api/v1/admin/groups/{id}/members"),
        Some(&token),
        Some(json!({ "userIds": [] })),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::CONFLICT);
    assert_eq!(body["error"]["code"], "derived_group");
}

#[sqlx::test]
async fn user_creation_returns_an_onboarding_code_that_logs_in(pool: PgPool) {
    let admin = insert_user(&pool, "admin@example.com", "super_admin").await;
    let token = token_for(&pool, admin).await;
    let app = app(pool.clone());

    let (status, created) = send(
        &app,
        "POST",
        "/api/v1/admin/users",
        Some(&token),
        Some(json!({
            "email": "New.Person@Example.com",
            "name": "New Person",
            "department": "Finance"
        })),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::CREATED, "create user: {created}");
    assert_eq!(created["email"], "new.person@example.com");
    assert_eq!(created["hasPassword"], false);
    assert_eq!(created["mustChangePassword"], true);

    let user_id = created["id"].as_str().unwrap().to_string();

    let (status, issued) = send(
        &app,
        "POST",
        &format!("/api/v1/admin/users/{user_id}/reset-code"),
        Some(&token),
        Some(json!({ "purpose": "onboard" })),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);
    let code = issued["code"].as_str().unwrap().to_string();
    assert_eq!(code.len(), 9);
    assert!(code.contains('-'));
    assert!(issued["expiresAt"].is_string());

    let (status, body) = send(
        &app,
        "POST",
        "/api/v1/auth/reset",
        None,
        Some(json!({
            "email": "new.person@example.com",
            "code": code,
            "newPassword": NEW_PASSWORD
        })),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::NO_CONTENT, "reset: {body}");

    let (status, session) = send(
        &app,
        "POST",
        "/api/v1/auth/login",
        None,
        Some(json!({ "email": "new.person@example.com", "password": NEW_PASSWORD })),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(session["user"]["role"], "user");

    let user_token = session["token"].as_str().unwrap().to_string();
    let (status, _) = send(&app, "GET", "/api/v1/client/sync", Some(&user_token), None).await;
    assert_eq!(status, axum::http::StatusCode::OK);
}

#[sqlx::test]
async fn grants_drive_the_client_sync_payload(pool: PgPool) {
    let admin = insert_user(&pool, "admin@example.com", "super_admin").await;
    let user = insert_user(&pool, "viewer@example.com", "user").await;
    let admin_token = token_for(&pool, admin).await;
    let user_token = token_for(&pool, user).await;
    let app = app(pool.clone());

    let created = create_app(&app, &admin_token, "Payroll", "Finance").await;
    let app_id = created["id"].as_str().unwrap().to_string();

    let (_, group) = send(
        &app,
        "POST",
        "/api/v1/admin/groups",
        Some(&admin_token),
        Some(json!({ "name": "Finance" })),
    )
    .await;
    let group_id = group["id"].as_str().unwrap().to_string();

    send(
        &app,
        "PUT",
        &format!("/api/v1/admin/groups/{group_id}/members"),
        Some(&admin_token),
        Some(json!({ "userIds": [user] })),
    )
    .await;

    let (_, payload) = send(&app, "GET", "/api/v1/client/sync", Some(&user_token), None).await;
    assert_eq!(payload["apps"].as_array().unwrap().len(), 0);

    let (status, _) = send(
        &app,
        "PUT",
        &format!("/api/v1/admin/apps/{app_id}/groups"),
        Some(&admin_token),
        Some(json!({ "groupIds": [group_id] })),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);

    let (_, payload) = send(&app, "GET", "/api/v1/client/sync", Some(&user_token), None).await;
    let apps = payload["apps"].as_array().unwrap();
    assert_eq!(apps.len(), 1);
    assert_eq!(apps[0]["name"], "Payroll");
    assert_eq!(apps[0]["iconUrl"], Value::Null);
    assert_eq!(payload["snapshotDays"], 7);

    // Direct grants work the same way, without a group.
    send(
        &app,
        "PUT",
        &format!("/api/v1/admin/apps/{app_id}/groups"),
        Some(&admin_token),
        Some(json!({ "groupIds": [] })),
    )
    .await;
    send(
        &app,
        "PUT",
        &format!("/api/v1/admin/users/{user}/apps"),
        Some(&admin_token),
        Some(json!({ "appIds": [app_id] })),
    )
    .await;

    let (_, payload) = send(&app, "GET", "/api/v1/client/sync", Some(&user_token), None).await;
    assert_eq!(payload["apps"].as_array().unwrap().len(), 1);

    // Disabling the app hides it even though the grant is still there.
    send(
        &app,
        "PATCH",
        &format!("/api/v1/admin/apps/{app_id}"),
        Some(&admin_token),
        Some(json!({ "isEnabled": false })),
    )
    .await;

    let (_, payload) = send(&app, "GET", "/api/v1/client/sync", Some(&user_token), None).await;
    assert_eq!(payload["apps"].as_array().unwrap().len(), 0);
}

#[sqlx::test]
async fn settings_are_validated_and_bump_the_revision(pool: PgPool) {
    let admin = insert_user(&pool, "admin@example.com", "super_admin").await;
    let token = token_for(&pool, admin).await;
    let app = app(pool.clone());

    let (status, body) = send(
        &app,
        "PUT",
        "/api/v1/admin/settings",
        Some(&token),
        Some(json!({ "values": { "sync_interval_hours": 999 } })),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "invalid_setting");

    let (status, body) = send(
        &app,
        "PUT",
        "/api/v1/admin/settings",
        Some(&token),
        Some(json!({ "values": { "not_a_setting": 1 } })),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "unknown_setting");

    let (status, body) = send(
        &app,
        "PUT",
        "/api/v1/admin/settings",
        Some(&token),
        Some(json!({
            "values": {
                "sync_interval_hours": 6,
                "announcement": { "text": "Payroll moves Monday", "level": "info" }
            }
        })),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK, "settings: {body}");
    assert_eq!(revision(&pool).await, 2);

    let stored: Value = sqlx::query_scalar("select value from settings where key = 'sync_interval_hours'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(stored, json!(6));

    let (_, payload) = send(&app, "GET", "/api/v1/client/sync", Some(&token), None).await;
    assert_eq!(payload["announcement"]["text"], "Payroll moves Monday");
}

#[sqlx::test]
async fn icons_are_uploaded_and_served_with_etags(pool: PgPool) {
    let admin = insert_user(&pool, "admin@example.com", "super_admin").await;
    let user = insert_user(&pool, "viewer@example.com", "user").await;
    let admin_token = token_for(&pool, admin).await;
    let user_token = token_for(&pool, user).await;
    let app = app(pool.clone());

    let created = create_app(&app, &admin_token, "Payroll", "Finance").await;
    let app_id = created["id"].as_str().unwrap().to_string();

    let (status, _headers, body) = send_bytes(
        &app,
        "POST",
        &format!("/api/v1/admin/apps/{app_id}/icon"),
        Some(&admin_token),
        PNG.to_vec(),
        Some("image/png"),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK, "upload: {:?}", String::from_utf8_lossy(&body));

    let icon: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(icon["iconSource"], "upload");
    assert_eq!(icon["iconMime"], "image/png");
    let etag = icon["iconEtag"].as_str().unwrap().to_string();

    let (status, _headers, bytes) = send_raw(
        &app,
        "GET",
        &format!("/api/v1/icons/{app_id}"),
        Some(&user_token),
        None,
        None,
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::FORBIDDEN, "not entitled yet");
    let error: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(error["error"]["code"], "not_entitled");

    send(
        &app,
        "PUT",
        &format!("/api/v1/admin/users/{user}/apps"),
        Some(&admin_token),
        Some(json!({ "appIds": [app_id] })),
    )
    .await;

    let (status, headers, bytes) = send_raw(
        &app,
        "GET",
        &format!("/api/v1/icons/{app_id}"),
        Some(&user_token),
        None,
        None,
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(bytes, PNG);
    assert_eq!(headers["content-type"], "image/png");
    assert_eq!(headers["etag"], etag.as_str());

    let request = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/icons/{app_id}"))
        .header("authorization", format!("Bearer {user_token}"))
        .header("if-none-match", &etag)
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::NOT_MODIFIED);
}

#[sqlx::test]
async fn csv_import_reports_rows_and_creates_onboarding_codes(pool: PgPool) {
    let admin = insert_user(&pool, "admin@example.com", "super_admin").await;
    let token = token_for(&pool, admin).await;
    let app = app(pool.clone());

    send(
        &app,
        "POST",
        "/api/v1/admin/groups",
        Some(&token),
        Some(json!({ "name": "Finance" })),
    )
    .await;

    let csv = "name,email,department,groups\n\
               Rahul Sharma,rahul@example.com,Finance,Finance\n\
               Unknown Person,unknown@example.com,Finance,Missing Group\n\
               Rahul Again,rahul@example.com,Finance,\n";

    let (status, report) = send(
        &app,
        "POST",
        "/api/v1/admin/users/import",
        Some(&token),
        Some(json!({ "csv": csv })),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK, "import: {report}");
    assert_eq!(report["created"], 1);
    assert_eq!(report["errors"].as_array().unwrap().len(), 2);
    assert_eq!(report["onboarding"].as_array().unwrap().len(), 1);
    assert_eq!(report["onboarding"][0]["email"], "rahul@example.com");

    let members: i64 = sqlx::query_scalar(
        "select count(*) from group_members gm
           join groups g on g.id = gm.group_id
          where g.name = 'Finance'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(members, 1);

    // A second import updates the existing account instead of duplicating it.
    let csv = "name,email,department,groups\nRahul S,rahul@example.com,Operations,\n";
    let (_, report) = send(
        &app,
        "POST",
        "/api/v1/admin/users/import",
        Some(&token),
        Some(json!({ "csv": csv })),
    )
    .await;

    assert_eq!(report["created"], 0);
    assert_eq!(report["updated"], 1);

    let name: String = sqlx::query_scalar("select name from users where email = 'rahul@example.com'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(name, "Rahul S");
}

#[sqlx::test]
async fn grant_updates_reject_unknown_fields(pool: PgPool) {
    // The "set the whole collection" endpoints must refuse a body they do not understand:
    // silently reading it as an empty list would wipe the grants.
    let admin = insert_user(&pool, "admin@example.com", "super_admin").await;
    let token = token_for(&pool, admin).await;
    let app = app(pool.clone());

    let member = insert_user(&pool, "member@example.com", "user").await;
    let group_id: Uuid = {
        let (status, created) = send(
            &app,
            "POST",
            "/api/v1/admin/groups",
            Some(&token),
            Some(json!({ "name": "Reporting" })),
        )
        .await;
        assert_eq!(status, axum::http::StatusCode::CREATED, "{created}");
        Uuid::parse_str(created["id"].as_str().unwrap()).unwrap()
    };

    let (status, _) = send(
        &app,
        "PUT",
        &format!("/api/v1/admin/groups/{group_id}/members"),
        Some(&token),
        Some(json!({ "userIds": [member] })),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);

    let (status, error) = send(
        &app,
        "PUT",
        &format!("/api/v1/admin/groups/{group_id}/members"),
        Some(&token),
        Some(json!({ "memberIds": [member] })),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::BAD_REQUEST, "{error}");
    assert_eq!(error["error"]["code"], "invalid_request");

    let members: i64 = sqlx::query_scalar("select count(*) from group_members where group_id = $1")
        .bind(group_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(members, 1, "the rejected request must not clear membership");
}

#[sqlx::test]
async fn csv_import_accepts_all_users_and_department_groups(pool: PgPool) {
    let admin = insert_user(&pool, "admin@example.com", "super_admin").await;
    let token = token_for(&pool, admin).await;
    let app = app(pool.clone());

    // `#[sqlx::test]` applies migrations only, so reproduce what `seed::run` creates at
    // startup: the built-in All Users group.
    sqlx::query("insert into groups (name, is_all_users) values ('All Users', true)")
        .execute(&pool)
        .await
        .expect("seed the All Users group");

    // The other group kind an admin sees in the picker: auto-by-department.
    send(
        &app,
        "POST",
        "/api/v1/admin/groups",
        Some(&token),
        Some(json!({ "name": "Finance desk", "autoDepartment": "Finance" })),
    )
    .await;

    let csv = "name,email,department,groups\n\
               Priya Nair,priya@example.com,Finance,All Users;Finance desk\n\
               Omar Farouk,omar@example.com,Finance,all users\n";

    let (status, report) = send(
        &app,
        "POST",
        "/api/v1/admin/users/import",
        Some(&token),
        Some(json!({ "csv": csv })),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK, "import: {report}");
    assert_eq!(report["errors"].as_array().unwrap().len(), 0, "{report}");
    assert_eq!(report["created"], 2, "{report}");

    // Priya is in both All Users and the department group; Omar only in All Users.
    let memberships: i64 = sqlx::query_scalar(
        "select count(*) from group_members gm
            join users u on u.id = gm.user_id
            join groups g on g.id = gm.group_id
          where u.email in ('priya@example.com', 'omar@example.com')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(memberships, 3);

    let all_users: i64 = sqlx::query_scalar(
        "select count(*) from group_members gm
            join users u on u.id = gm.user_id
            join groups g on g.id = gm.group_id
          where g.is_all_users and u.email = 'omar@example.com'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(all_users, 1, "a lowercase group name should still resolve");
}

#[sqlx::test]
async fn app_admin_is_limited_to_its_categories(pool: PgPool) {
    let admin = insert_user(&pool, "admin@example.com", "super_admin").await;
    let scoped = insert_user_with_scope(
        &pool,
        "scoped@example.com",
        "app_admin",
        Some(json!({ "categories": ["Finance"] })),
    )
    .await;
    let admin_token = token_for(&pool, admin).await;
    let scoped_token = token_for(&pool, scoped).await;
    let app = app(pool.clone());

    create_app(&app, &admin_token, "Payroll", "Finance").await;
    create_app(&app, &admin_token, "Telecom", "Telecom").await;

    let (status, apps) = send(&app, "GET", "/api/v1/admin/apps", Some(&scoped_token), None).await;
    assert_eq!(status, axum::http::StatusCode::OK);
    let apps = apps.as_array().unwrap();
    assert_eq!(apps.len(), 1);
    assert_eq!(apps[0]["name"], "Payroll");

    let (status, body) = send(
        &app,
        "POST",
        "/api/v1/admin/apps",
        Some(&scoped_token),
        Some(json!({
            "name": "Switchboard",
            "url": "http://example.com",
            "category": "Telecom"
        })),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::FORBIDDEN);
    assert_eq!(body["error"]["code"], "out_of_scope");

    // People and settings are super-admin only.
    let (status, _) = send(&app, "GET", "/api/v1/admin/users", Some(&scoped_token), None).await;
    assert_eq!(status, axum::http::StatusCode::FORBIDDEN);

    let (status, _) = send(
        &app,
        "PUT",
        "/api/v1/admin/settings",
        Some(&scoped_token),
        Some(json!({ "values": { "offline_grace_days": 3 } })),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::FORBIDDEN);
}

#[sqlx::test]
async fn super_admin_assigns_app_admin_role_and_scope(pool: PgPool) {
    let admin = insert_user(&pool, "admin@example.com", "super_admin").await;
    let person = insert_user(&pool, "person@example.com", "user").await;
    let token = token_for(&pool, admin).await;
    let app = app(pool.clone());

    create_app(&app, &token, "Payroll", "Finance").await;
    create_app(&app, &token, "Telecom", "Telecom").await;

    let (status, updated) = send(
        &app,
        "PATCH",
        &format!("/api/v1/admin/users/{person}"),
        Some(&token),
        Some(json!({
            "role": "app_admin",
            "adminScope": { "categories": [" finance ", "Finance", "HR"] }
        })),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK, "promote: {updated}");
    assert_eq!(updated["role"], "app_admin");
    assert_eq!(updated["adminScope"]["categories"], json!(["finance", "HR"]));

    // A scope change applies to the next request; sessions reload the user row.
    let person_token = token_for(&pool, person).await;
    let (status, apps) = send(
        &app,
        "GET",
        "/api/v1/admin/apps",
        Some(&person_token),
        None,
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(apps.as_array().unwrap().len(), 1);
    assert_eq!(apps[0]["name"], "Payroll");

    // Demoting back to a normal account clears the scope entirely.
    let (status, demoted) = send(
        &app,
        "PATCH",
        &format!("/api/v1/admin/users/{person}"),
        Some(&token),
        Some(json!({ "role": "user" })),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(demoted["adminScope"], Value::Null);

    let (status, _) = send(
        &app,
        "GET",
        "/api/v1/admin/apps",
        Some(&person_token),
        None,
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::FORBIDDEN);
}

#[sqlx::test]
async fn invalid_admin_scope_is_rejected(pool: PgPool) {
    let admin = insert_user(&pool, "admin@example.com", "super_admin").await;
    let person = insert_user(&pool, "person@example.com", "user").await;
    let token = token_for(&pool, admin).await;
    let app = app(pool.clone());

    let (status, body) = send(
        &app,
        "PATCH",
        &format!("/api/v1/admin/users/{person}"),
        Some(&token),
        Some(json!({
            "role": "app_admin",
            "adminScope": { "categories": ["Finance", "   "] }
        })),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "invalid_scope");

    let (status, body) = send(
        &app,
        "PATCH",
        &format!("/api/v1/admin/users/{person}"),
        Some(&token),
        Some(json!({ "role": "app_admin", "adminScope": "Finance" })),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "invalid_scope");

    // Nothing was applied by the rejected requests.
    let role: String = sqlx::query_scalar("select role from users where id = $1")
        .bind(person)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(role, "user");
}

#[sqlx::test]
async fn admins_cannot_deactivate_themselves(pool: PgPool) {
    let admin = insert_user(&pool, "admin@example.com", "super_admin").await;
    let token = token_for(&pool, admin).await;
    let app = app(pool.clone());

    let (status, body) = send(
        &app,
        "PATCH",
        &format!("/api/v1/admin/users/{admin}"),
        Some(&token),
        Some(json!({ "isActive": false })),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "cannot_modify_self");

    let (status, body) = send(
        &app,
        "DELETE",
        &format!("/api/v1/admin/users/{admin}"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "cannot_modify_self");
}

#[sqlx::test]
async fn reset_request_queue_issues_a_working_code(pool: PgPool) {
    let admin = insert_user(&pool, "admin@example.com", "super_admin").await;
    let user = insert_user(&pool, "person@example.com", "user").await;
    let token = token_for(&pool, admin).await;
    let app = app(pool.clone());

    let (status, _) = send(
        &app,
        "POST",
        "/api/v1/auth/forgot",
        None,
        Some(json!({ "email": "person@example.com" })),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::ACCEPTED);

    let (_, queue) = send(
        &app,
        "GET",
        "/api/v1/admin/reset-requests?status=open",
        Some(&token),
        None,
    )
    .await;
    let queue = queue.as_array().unwrap();
    assert_eq!(queue.len(), 1);
    assert_eq!(queue[0]["userId"], json!(user));

    let request_id = queue[0]["id"].as_str().unwrap().to_string();

    let (status, issued) = send(
        &app,
        "POST",
        &format!("/api/v1/admin/reset-requests/{request_id}/issue"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);
    let code = issued["code"].as_str().unwrap().to_string();
    assert!(issued["expiresAt"].is_string());

    let status: String = sqlx::query_scalar("select status from reset_requests where id = $1")
        .bind(Uuid::parse_str(&request_id).unwrap())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "issued");

    // The issued code works, exactly like a code from the user screen.
    let (status, _) = send(
        &app,
        "POST",
        "/api/v1/auth/reset",
        None,
        Some(json!({
            "email": "person@example.com",
            "code": code,
            "newPassword": NEW_PASSWORD
        })),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::NO_CONTENT);

    // Resolving twice is a conflict, not a silent success.
    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/v1/admin/reset-requests/{request_id}/dismiss"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::CONFLICT);
    assert_eq!(body["error"]["code"], "reset_request_closed");
}

#[sqlx::test]
async fn audit_query_filters_by_action_entity_and_actor(pool: PgPool) {
    let admin = insert_user(&pool, "admin@example.com", "super_admin").await;
    let token = token_for(&pool, admin).await;
    let app = app(pool.clone());

    create_app(&app, &token, "Payroll", "Finance").await;
    create_app(&app, &token, "Telecom", "Telecom").await;

    let (_, page) = send(
        &app,
        "GET",
        "/api/v1/admin/audit?entityType=app&action=app.create",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(page["total"], 2);
    assert_eq!(page["entries"].as_array().unwrap().len(), 2);

    let (_, page) = send(
        &app,
        "GET",
        "/api/v1/admin/audit?actor=admin@example.com",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(page["total"], 2);
    assert_eq!(page["entries"][0]["actorEmail"], "admin@example.com");

    let (_, page) = send(
        &app,
        "GET",
        "/api/v1/admin/audit?actor=nobody@example.com",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(page["total"], 0);
}

#[sqlx::test]
async fn sync_requires_auth_and_returns_the_revision(pool: PgPool) {
    let user = insert_user(&pool, "person@example.com", "user").await;
    let token = token_for(&pool, user).await;
    let app = app(pool.clone());

    let (status, _) = send(&app, "GET", "/api/v1/client/sync", None, None).await;
    assert_eq!(status, axum::http::StatusCode::UNAUTHORIZED);

    let (status, payload) = send(&app, "GET", "/api/v1/client/sync?cursor=5", Some(&token), None).await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(payload["revision"], 1);
    assert_eq!(payload["user"]["name"], "person@example.com");

    let server_time = payload["serverTime"].as_str().expect("serverTime string");
    let parsed = chrono::DateTime::parse_from_rfc3339(server_time).expect("valid timestamp");
    assert!(parsed.with_timezone(&Utc) <= Utc::now());
}
