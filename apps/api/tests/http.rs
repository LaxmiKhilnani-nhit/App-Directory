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
    assert_eq!(body["token"], json!("csv"));
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
    assert_eq!(error["error"]["code"], json!("unknown_user"));
}

#[tokio::test]
async fn normal_users_cannot_use_the_admin_portal() {
    let h = setup();
    add_user(&h, "laxmikhilnani", Some("IT"), true);

    let (status, error) = json(
        &h,
        "POST",
        "/api/v1/auth/login",
        None,
        json!({ "email": "laxmikhilnani@nhit.co.in", "password": "anything" }),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(error["error"]["code"], json!("admin_only"));

    let (status, error) = get(&h, "/api/v1/admin/me", Some("ad\\laxmikhilnani")).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(error["error"]["code"], json!("admin_only"));

    let (status, _) = get(&h, "/api/v1/admin/apps", Some("ad\\laxmikhilnani")).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // The desktop client still works for the same user.
    let (status, _) = get(&h, "/api/v1/client/sync", Some("ad\\laxmikhilnani")).await;
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
