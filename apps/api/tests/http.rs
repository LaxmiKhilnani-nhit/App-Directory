//! HTTP contract tests: health, trusted login, unknown users, admin guard-free CRUD and icons.

mod support;

use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use serde_json::{json, Value};
use support::*;
use tower::ServiceExt;

const PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
];

async fn send_raw(
    app: &Router,
    method: &str,
    uri: &str,
    identity: Option<&str>,
    bytes: Vec<u8>,
    content_type: &str,
) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", content_type);
    if let Some(identity) = identity {
        builder = builder.header("x-windows-identity", identity);
    }
    let request = builder.body(Body::from(bytes)).unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap()
        .to_vec();
    (status, headers, body)
}

#[tokio::test]
async fn healthz_reports_csv_database() {
    let h = setup();
    let (status, body) = get(&h, "/api/v1/healthz", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], json!("ok"));
    assert_eq!(body["database"], json!("csv"));
    assert!(body["appName"].is_string());
    assert!(body["version"].is_string());
}

#[tokio::test]
async fn login_uses_trusted_identity_and_requires_a_known_user() {
    let h = setup();
    add_admin_user(&h, "laxmikhilnani", "super_admin");

    let (status, body) = json(
        &h,
        "POST",
        "/api/v1/auth/login",
        None,
        json!({ "email": "laxmikhilnani@nhit.co.in", "password": "anything" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let issued = body["token"].as_str().unwrap();
    assert!(issued.contains("laxmikhilnani@nhit.co.in"), "{issued}");
    assert!(issued.split('|').count() == 3, "{issued}");
    assert_eq!(body["mustChangePassword"], json!(false));
    assert_eq!(body["user"]["email"], json!("laxmikhilnani@nhit.co.in"));

    let token = body["token"].as_str().unwrap();
    let (status, _, me) = call(
        &h,
        "/api/v1/auth/me",
        SendOptions {
            method: "GET",
            token: Some(token),
            identity: Some("ad\\laxmikhilnani"),
            ..Default::default()
        },
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(me["email"], json!("laxmikhilnani@nhit.co.in"));

    let (status, error) = json(
        &h,
        "POST",
        "/api/v1/auth/login",
        None,
        json!({ "email": "nobody@nhit.co.in", "password": "" }),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(error["error"]["code"], json!("invalid_credentials"));
}

#[tokio::test]
async fn csv_import_cannot_grant_admin_roles() {
    let h = setup();
    add_admin_user(&h, "admin", "super_admin");

    let csv =
        "name,email,department,groups,role\nFake Admin,fakeadmin@nhit.co.in,IT,,super_admin\n";
    let (status, body) = json(
        &h,
        "POST",
        "/api/v1/admin/users/import",
        Some("ad\\admin"),
        json!({ "csv": csv }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (_, page) = get(
        &h,
        "/api/v1/admin/users?query=fakeadmin&limit=1",
        Some("ad\\admin"),
    )
    .await;
    assert_eq!(page["users"][0]["role"], json!("user"));

    // Importing must not change an existing administrator either.
    let csv = "name,email,department,groups,role\nRenamed Admin,admin@nhit.co.in,IT,,user\n";
    let (_, _) = json(
        &h,
        "POST",
        "/api/v1/admin/users/import",
        Some("ad\\admin"),
        json!({ "csv": csv }),
    )
    .await;
    let (_, page) = get(
        &h,
        "/api/v1/admin/users?query=admin&limit=20",
        Some("ad\\admin"),
    )
    .await;
    let admins: Vec<&serde_json::Value> = page["users"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|user| user["email"] == json!("admin@nhit.co.in"))
        .collect();
    assert_eq!(admins[0]["role"], json!("super_admin"));
}

#[tokio::test]
async fn csv_import_creates_unknown_departments() {
    let h = setup();
    add_admin_user(&h, "admin", "super_admin");

    let csv = "name,email,department,groups\nNew Person,newperson@nhit.co.in,Brand New Dept,\n";
    let (status, body) = json(
        &h,
        "POST",
        "/api/v1/admin/users/import",
        Some("ad\\admin"),
        json!({ "csv": csv }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (_, departments) = get(&h, "/api/v1/admin/departments", Some("ad\\admin")).await;
    let names: Vec<&str> = departments
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|row| row["name"].as_str())
        .collect();
    assert!(names.contains(&"Brand New Dept"), "{names:?}");

    let (_, sync) = get(&h, "/api/v1/client/sync", Some("newperson@nhit.co.in")).await;
    assert_eq!(sync["user"]["department"], json!("Brand New Dept"));
}

#[tokio::test]
async fn csv_import_accepts_excel_bom_and_semicolons() {
    let h = setup();
    add_admin_user(&h, "admin", "super_admin");

    let csv = "\u{feff}name;email;department;groups\nLaxmi;laxmi@nhit.co.in;IT;\n";
    let (status, body) = json(
        &h,
        "POST",
        "/api/v1/admin/users/import",
        Some("ad\\admin"),
        json!({ "csv": csv }),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["created"], json!(1), "{body}");
    assert_eq!(body["errors"].as_array().unwrap().len(), 0, "{body}");
}

#[tokio::test]
async fn normal_users_cannot_use_the_admin_portal() {
    let h = setup();
    add_user(&h, "plainuser", Some("IT"), true);

    let (status, error) = json(
        &h,
        "POST",
        "/api/v1/auth/login",
        None,
        json!({ "email": "plainuser@nhit.co.in", "password": "anything" }),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(error["error"]["code"], json!("invalid_credentials"));

    let (status, error) = get(&h, "/api/v1/admin/me", Some("ad\\plainuser")).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(error["error"]["code"], json!("admin_only"));

    let (status, _) = get(&h, "/api/v1/admin/apps", Some("ad\\plainuser")).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // No identity at all must not fall back to the first imported account.
    let (status, _) = get(&h, "/api/v1/admin/apps", None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // The desktop client still works for the same user.
    let (status, _) = get(&h, "/api/v1/client/sync", Some("ad\\plainuser")).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn unknown_identity_is_rejected_by_sync() {
    let h = setup();
    add_user(&h, "known", None, true);

    let (status, body) = get(&h, "/api/v1/client/sync", Some("ad\\known")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["user"]["name"], json!("known"));
    assert_eq!(body["snapshotDays"], json!(7));
    assert_eq!(body["syncIntervalHours"], json!(4));

    let (status, error) = get(&h, "/api/v1/client/sync?identity=ad%5Cunknown", None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(error["error"]["code"], json!("unknown_user"));
}

#[tokio::test]
async fn admin_me_is_always_super_admin() {
    let h = setup();
    add_admin_user(&h, "anyone", "super_admin");
    let (status, body) = get(&h, "/api/v1/admin/me", Some("ad\\anyone")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["isSuperAdmin"], json!(true));
    assert_eq!(body["user"]["role"], json!("super_admin"));
    assert_eq!(body["user"]["mustChangePassword"], json!(false));
}

#[tokio::test]
async fn app_crud_and_icon_round_trip() {
    let h = setup();
    add_admin_user(&h, "admin", "super_admin");
    let identity = "ad\\admin";

    let (status, created) = json(
        &h,
        "POST",
        "/api/v1/admin/apps",
        Some(identity),
        json!({ "name": "Payroll", "url": "http://127.0.0.1:8080/payroll", "category": "Finance" }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let id = created["id"].as_str().unwrap();

    let (status, updated) = json(
        &h,
        "PATCH",
        &format!("/api/v1/admin/apps/{id}"),
        Some(identity),
        json!({ "name": "Payroll 2", "description": "Monthly" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(updated["name"], json!("Payroll 2"));

    let (status, _, body) = send_raw(
        &h.app,
        "POST",
        &format!("/api/v1/admin/apps/{id}/icon"),
        Some(identity),
        PNG.to_vec(),
        "image/png",
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    let icon: Value = serde_json::from_slice(&body).unwrap();
    let etag = icon["iconEtag"].as_str().unwrap().to_string();
    assert_eq!(icon["iconMime"], json!("image/png"));

    let (status, headers, bytes) = send_raw(
        &h.app,
        "GET",
        &format!("/api/v1/icons/{id}"),
        None,
        Vec::new(),
        "",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(bytes, PNG);
    assert_eq!(headers["content-type"], "image/png");
    assert_eq!(headers["etag"], etag.as_str());

    let request = Request::builder()
        .uri(format!("/api/v1/icons/{id}"))
        .header("if-none-match", &etag)
        .body(Body::empty())
        .unwrap();
    let response = h.app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_MODIFIED);

    let (delete_status, _, _) = send_raw(
        &h.app,
        "DELETE",
        &format!("/api/v1/admin/apps/{id}"),
        Some(identity),
        Vec::new(),
        "application/json",
    )
    .await;
    assert_eq!(delete_status, StatusCode::NO_CONTENT);
    let (status, _) = get(&h, &format!("/api/v1/admin/apps/{id}"), Some(identity)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn purge_user_archives_to_delete_agent_csv() {
    let h = setup();
    add_admin_user(&h, "admin", "super_admin");
    let identity = "ad\\admin";
    let user_id = add_user(&h, "leaving", Some("IT"), true);
    let app_id = add_app(&h, "Payroll", 0);
    grant_user(&h, user_id, app_id);

    let (status, _) = get(&h, &format!("/api/v1/admin/users/{user_id}"), Some(identity)).await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = json(
        &h,
        "DELETE",
        &format!("/api/v1/admin/users/{user_id}/purge"),
        Some(identity),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // Gone from the portal and from users.csv.
    let (status, _) = get(&h, &format!("/api/v1/admin/users/{user_id}"), Some(identity)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let users_csv = std::fs::read_to_string(h.dir.join("users.csv")).unwrap();
    assert!(!users_csv.contains("leaving@nhit.co.in"), "{users_csv}");

    // Archived in delete_agent.csv with the actor and a timestamp.
    let deleted_csv = std::fs::read_to_string(h.dir.join("delete_agent.csv")).unwrap();
    assert!(deleted_csv.contains("leaving@nhit.co.in"), "{deleted_csv}");
    assert!(deleted_csv.contains("admin@nhit.co.in"), "{deleted_csv}");
    assert_eq!(deleted_csv.lines().count(), 2, "{deleted_csv}");

    // Direct app grants were cleaned up.
    let grants = std::fs::read_to_string(h.dir.join("user_apps.csv")).unwrap();
    assert!(!grants.contains(&user_id.to_string()), "{grants}");
}

#[tokio::test]
async fn sync_payload_carries_local_apps_flags_from_settings() {
    let h = setup();
    add_admin_user(&h, "admin", "super_admin");
    add_user(&h, "plainuser", Some("IT"), true);

    // Defaults: local apps enabled, system apps hidden.
    let (status, sync) = get(&h, "/api/v1/client/sync", Some("ad\\plainuser")).await;
    assert_eq!(status, StatusCode::OK, "{sync}");
    assert_eq!(sync["localAppsEnabled"], json!(true));
    assert_eq!(sync["localAppsShowSystem"], json!(false));

    let (status, body) = json(
        &h,
        "PUT",
        "/api/v1/admin/settings",
        Some("ad\\admin"),
        json!({ "values": {
            "local_apps_enabled": 0,
            "local_apps_show_system": 1
        }}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, sync) = get(&h, "/api/v1/client/sync", Some("ad\\plainuser")).await;
    assert_eq!(status, StatusCode::OK, "{sync}");
    assert_eq!(sync["localAppsEnabled"], json!(false));
    assert_eq!(sync["localAppsShowSystem"], json!(true));
}