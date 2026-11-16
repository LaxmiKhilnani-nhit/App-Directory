//! Auth integration tests: real PostgreSQL, real HTTP handlers.
//!
//! `#[sqlx::test]` creates and drops a database per test and applies `migrations/`.
//! Settings defaults are deliberately *not* seeded here, so these tests also prove the
//! code falls back to sane built-in limits when a key is missing.

use axum::{body::Body, http::Request, Router};
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

use app_directory_api::config::Config;
use app_directory_api::domain::auth::{self, AuthError, LockoutPolicy};
use app_directory_api::domain::password;
use app_directory_api::state::AppState;
use app_directory_api::build_router;

const PASSWORD: &str = "correct-horse-battery";
const NEW_PASSWORD: &str = "new-horse-battery-staple";

fn app(pool: PgPool) -> Router {
    build_router(AppState::new(
        Config {
            session_ttl_days: 30,
            ..Config::default()
        },
        Some(pool),
    ))
}

async fn insert_user(pool: &PgPool, email: &str, role: &str, active: bool) -> Uuid {
    let hash = password::hash_password(PASSWORD).expect("hash password");

    sqlx::query_scalar(
        "insert into users (email, name, department, role, is_active, password_hash,
                            must_change_password)
         values (lower($1), $2, 'Finance', $3, $4, $5, false)
         returning id",
    )
    .bind(email)
    .bind(email)
    .bind(role)
    .bind(active)
    .bind(hash)
    .fetch_one(pool)
    .await
    .expect("insert user")
}

async fn send(
    app: &Router,
    method: &str,
    uri: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (axum::http::StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(uri);

    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }

    let request = match body {
        Some(value) => builder
            .header("content-type", "application/json")
            .body(Body::from(value.to_string()))
            .expect("build request"),
        None => builder.body(Body::empty()).expect("build request"),
    };

    let response = app.clone().oneshot(request).await.expect("call handler");
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read body");

    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };

    (status, value)
}

async fn login(app: &Router, email: &str, password: &str) -> (axum::http::StatusCode, Value) {
    send(
        app,
        "POST",
        "/api/v1/auth/login",
        None,
        Some(json!({ "email": email, "password": password })),
    )
    .await
}

#[sqlx::test]
async fn login_issues_a_session_and_me_returns_it(pool: PgPool) {
    insert_user(&pool, "person@example.com", "user", true).await;
    let app = app(pool.clone());

    let (status, body) = login(&app, "person@example.com", PASSWORD).await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(body["user"]["email"], "person@example.com");
    assert_eq!(body["mustChangePassword"], false);
    assert!(body["expiresAt"].is_string());

    let token = body["token"].as_str().expect("token").to_string();
    assert_eq!(token.len(), 64, "32 random bytes, hex encoded");

    let (status, me) = send(&app, "GET", "/api/v1/auth/me", Some(&token), None).await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(me["email"], "person@example.com");
    assert_eq!(me["role"], "user");
}

#[sqlx::test]
async fn login_is_case_insensitive_on_email(pool: PgPool) {
    insert_user(&pool, "mixed@example.com", "user", true).await;
    let app = app(pool.clone());

    let (status, _) = login(&app, "  MiXeD@Example.COM ", PASSWORD).await;
    assert_eq!(status, axum::http::StatusCode::OK);
}

#[sqlx::test]
async fn login_failure_uses_the_uniform_error_envelope(pool: PgPool) {
    insert_user(&pool, "person@example.com", "user", true).await;
    let app = app(pool.clone());

    let (status, body) = login(&app, "person@example.com", "wrong-password").await;
    assert_eq!(status, axum::http::StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"]["code"], "invalid_credentials");
    assert!(body["error"]["message"].is_string());
    assert!(body.get("token").is_none());
}

#[sqlx::test]
async fn unknown_email_and_wrong_password_look_identical(pool: PgPool) {
    insert_user(&pool, "person@example.com", "user", true).await;
    let app = app(pool.clone());

    let (missing_status, missing) = login(&app, "nobody@example.com", PASSWORD).await;
    let (wrong_status, wrong) = login(&app, "person@example.com", "wrong-password").await;

    assert_eq!(missing_status, wrong_status);
    assert_eq!(missing["error"]["code"], wrong["error"]["code"]);
}

#[sqlx::test]
async fn repeated_failures_lock_the_account(pool: PgPool) {
    let user = insert_user(&pool, "person@example.com", "user", true).await;
    let policy = LockoutPolicy {
        attempts: 3,
        minutes: 15,
    };

    for attempt in 1..=2 {
        let result = auth::login(&pool, "person@example.com", "wrong", 30, policy, None).await;
        assert!(
            matches!(result, Err(AuthError::InvalidCredentials)),
            "attempt {attempt} should be a plain rejection"
        );
    }

    let third = auth::login(&pool, "person@example.com", "wrong", 30, policy, None).await;
    assert!(matches!(third, Err(AuthError::Locked { .. })));

    // Even the correct password is refused while the lock is in force.
    let locked = auth::login(&pool, "person@example.com", PASSWORD, 30, policy, None).await;
    assert!(matches!(locked, Err(AuthError::Locked { .. })));

    let locked_until: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("select locked_until from users where id = $1")
            .bind(user)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(locked_until.is_some());
}

#[sqlx::test]
async fn inactive_user_is_refused(pool: PgPool) {
    insert_user(&pool, "gone@example.com", "user", false).await;
    let app = app(pool.clone());

    let (status, body) = login(&app, "gone@example.com", PASSWORD).await;
    assert_eq!(status, axum::http::StatusCode::FORBIDDEN);
    assert_eq!(body["error"]["code"], "account_inactive");
}

#[sqlx::test]
async fn me_requires_a_bearer_token(pool: PgPool) {
    let app = app(pool.clone());

    let (status, body) = send(&app, "GET", "/api/v1/auth/me", None, None).await;
    assert_eq!(status, axum::http::StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"]["code"], "unauthorized");

    let (status, _) = send(&app, "GET", "/api/v1/auth/me", Some("not-a-real-token"), None).await;
    assert_eq!(status, axum::http::StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn logout_revokes_only_that_session(pool: PgPool) {
    insert_user(&pool, "person@example.com", "user", true).await;
    let app = app(pool.clone());

    let (_, first) = login(&app, "person@example.com", PASSWORD).await;
    let (_, second) = login(&app, "person@example.com", PASSWORD).await;
    let first_token = first["token"].as_str().unwrap().to_string();
    let second_token = second["token"].as_str().unwrap().to_string();

    let (status, _) = send(&app, "POST", "/api/v1/auth/logout", Some(&first_token), None).await;
    assert_eq!(status, axum::http::StatusCode::NO_CONTENT);

    let (status, _) = send(&app, "GET", "/api/v1/auth/me", Some(&first_token), None).await;
    assert_eq!(status, axum::http::StatusCode::UNAUTHORIZED);

    let (status, _) = send(&app, "GET", "/api/v1/auth/me", Some(&second_token), None).await;
    assert_eq!(status, axum::http::StatusCode::OK);
}

#[sqlx::test]
async fn change_password_keeps_the_current_session_and_drops_the_rest(pool: PgPool) {
    let user = insert_user(&pool, "person@example.com", "user", true).await;
    let app = app(pool.clone());

    let (_, current) = login(&app, "person@example.com", PASSWORD).await;
    let (_, other) = login(&app, "person@example.com", PASSWORD).await;
    let current = current["token"].as_str().unwrap().to_string();
    let other = other["token"].as_str().unwrap().to_string();

    let (status, body) = send(
        &app,
        "POST",
        "/api/v1/auth/change-password",
        Some(&current),
        Some(json!({ "currentPassword": PASSWORD, "newPassword": NEW_PASSWORD })),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::NO_CONTENT, "body: {body}");

    let (status, _) = send(&app, "GET", "/api/v1/auth/me", Some(&current), None).await;
    assert_eq!(status, axum::http::StatusCode::OK, "current session survives");

    let (status, _) = send(&app, "GET", "/api/v1/auth/me", Some(&other), None).await;
    assert_eq!(status, axum::http::StatusCode::UNAUTHORIZED, "other session dropped");

    let (status, _) = login(&app, "person@example.com", NEW_PASSWORD).await;
    assert_eq!(status, axum::http::StatusCode::OK);

    let (status, _) = login(&app, "person@example.com", PASSWORD).await;
    assert_eq!(status, axum::http::StatusCode::UNAUTHORIZED);

    let must_change: bool =
        sqlx::query_scalar("select must_change_password from users where id = $1")
            .bind(user)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(!must_change);
}

#[sqlx::test]
async fn change_password_enforces_the_minimum_length(pool: PgPool) {
    insert_user(&pool, "person@example.com", "user", true).await;
    let app = app(pool.clone());

    let (_, session) = login(&app, "person@example.com", PASSWORD).await;
    let token = session["token"].as_str().unwrap().to_string();

    let (status, body) = send(
        &app,
        "POST",
        "/api/v1/auth/change-password",
        Some(&token),
        Some(json!({ "currentPassword": PASSWORD, "newPassword": "short" })),
    )
    .await;

    assert_eq!(status, axum::http::StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "password_too_short");
}

#[sqlx::test]
async fn change_password_rejects_a_wrong_current_password(pool: PgPool) {
    insert_user(&pool, "person@example.com", "user", true).await;
    let app = app(pool.clone());

    let (_, session) = login(&app, "person@example.com", PASSWORD).await;
    let token = session["token"].as_str().unwrap().to_string();

    let (status, body) = send(
        &app,
        "POST",
        "/api/v1/auth/change-password",
        Some(&token),
        Some(json!({ "currentPassword": "not-my-password", "newPassword": NEW_PASSWORD })),
    )
    .await;

    assert_eq!(status, axum::http::StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "invalid_password");
}

#[sqlx::test]
async fn reset_code_sets_a_new_password_and_is_single_use(pool: PgPool) {
    let user = insert_user(&pool, "person@example.com", "user", true).await;
    let app = app(pool.clone());

    let code = auth::issue_reset_code(&pool, user, "reset", None, 72)
        .await
        .expect("issue code");

    let (status, body) = send(
        &app,
        "POST",
        "/api/v1/auth/reset",
        None,
        Some(json!({ "email": "person@example.com", "code": code, "newPassword": NEW_PASSWORD })),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::NO_CONTENT, "body: {body}");

    let (status, _) = login(&app, "person@example.com", NEW_PASSWORD).await;
    assert_eq!(status, axum::http::StatusCode::OK);

    // The same code cannot be spent twice.
    let (status, body) = send(
        &app,
        "POST",
        "/api/v1/auth/reset",
        None,
        Some(json!({ "email": "person@example.com", "code": code, "newPassword": PASSWORD })),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "invalid_reset_code");
}

#[sqlx::test]
async fn reset_code_accepts_lowercase_and_spaced_input(pool: PgPool) {
    let user = insert_user(&pool, "person@example.com", "user", true).await;
    let app = app(pool.clone());

    let code = auth::issue_reset_code(&pool, user, "reset", None, 72)
        .await
        .expect("issue code");
    let typed = code.to_lowercase().replace('-', " ");

    let (status, _) = send(
        &app,
        "POST",
        "/api/v1/auth/reset",
        None,
        Some(json!({ "email": "person@example.com", "code": typed, "newPassword": NEW_PASSWORD })),
    )
    .await;

    assert_eq!(status, axum::http::StatusCode::NO_CONTENT);
}

#[sqlx::test]
async fn reset_with_unknown_email_or_bad_code_is_rejected(pool: PgPool) {
    insert_user(&pool, "person@example.com", "user", true).await;
    let app = app(pool.clone());

    let (status, body) = send(
        &app,
        "POST",
        "/api/v1/auth/reset",
        None,
        Some(json!({ "email": "nobody@example.com", "code": "AAAA-BBBB", "newPassword": NEW_PASSWORD })),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "invalid_reset_code");

    let (status, body) = send(
        &app,
        "POST",
        "/api/v1/auth/reset",
        None,
        Some(json!({ "email": "person@example.com", "code": "AAAA-BBBB", "newPassword": NEW_PASSWORD })),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "invalid_reset_code");
}

#[sqlx::test]
async fn forgot_answers_202_for_known_and_unknown_addresses(pool: PgPool) {
    insert_user(&pool, "person@example.com", "user", true).await;
    let app = app(pool.clone());

    let (known_status, _) = send(
        &app,
        "POST",
        "/api/v1/auth/forgot",
        None,
        Some(json!({ "email": "person@example.com" })),
    )
    .await;
    let (unknown_status, _) = send(
        &app,
        "POST",
        "/api/v1/auth/forgot",
        None,
        Some(json!({ "email": "nobody@example.com" })),
    )
    .await;

    assert_eq!(known_status, axum::http::StatusCode::ACCEPTED);
    assert_eq!(unknown_status, axum::http::StatusCode::ACCEPTED);

    // Known address queues exactly one open request; a repeat call does not duplicate it.
    send(
        &app,
        "POST",
        "/api/v1/auth/forgot",
        None,
        Some(json!({ "email": "person@example.com" })),
    )
    .await;

    let open: i64 = sqlx::query_scalar("select count(*) from reset_requests where status = 'open'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(open, 1);
}

#[sqlx::test]
async fn login_is_written_to_the_audit_log(pool: PgPool) {
    insert_user(&pool, "person@example.com", "user", true).await;
    let app = app(pool.clone());

    login(&app, "person@example.com", PASSWORD).await;

    let actions: Vec<String> =
        sqlx::query_scalar("select action from audit_log order by id")
            .fetch_all(&pool)
            .await
            .unwrap();

    assert_eq!(actions, vec!["auth.login"]);
}
