//! Access-control integration tests.
//!
//! These run against a real PostgreSQL instance (`DATABASE_URL`); `#[sqlx::test]` creates and
//! drops a throw-away database per test and applies `migrations/` to it.

use sqlx::PgPool;
use uuid::Uuid;

use app_directory_api::domain::entitlements;

async fn insert_user(pool: &PgPool, email: &str, department: Option<&str>, active: bool) -> Uuid {
    sqlx::query_scalar(
        "insert into users (email, name, department, is_active, must_change_password)
         values (lower($1), $1, $2, $3, false)
         returning id",
    )
    .bind(email)
    .bind(department)
    .bind(active)
    .fetch_one(pool)
    .await
    .expect("insert user")
}

async fn insert_app(pool: &PgPool, name: &str, sort_order: i32) -> Uuid {
    sqlx::query_scalar(
        "insert into apps (name, url, sort_order) values ($1, $2, $3) returning id",
    )
    .bind(name)
    .bind(format!("http://127.0.0.1:8080/{name}"))
    .bind(sort_order)
    .fetch_one(pool)
    .await
    .expect("insert app")
}

async fn insert_group(pool: &PgPool, name: &str, auto_department: Option<&str>) -> Uuid {
    sqlx::query_scalar(
        "insert into groups (name, auto_department) values ($1, $2) returning id",
    )
    .bind(name)
    .bind(auto_department)
    .fetch_one(pool)
    .await
    .expect("insert group")
}

async fn all_users_group(pool: &PgPool) -> Uuid {
    sqlx::query_scalar(
        "insert into groups (name, is_all_users) values ('All Users', true)
         on conflict (name) do update set is_all_users = true
         returning id",
    )
    .fetch_one(pool)
    .await
    .expect("insert all-users group")
}

async fn grant_to_user(pool: &PgPool, app: Uuid, user: Uuid) {
    sqlx::query("insert into user_apps (app_id, user_id) values ($1, $2)")
        .bind(app)
        .bind(user)
        .execute(pool)
        .await
        .expect("grant to user");
}

async fn add_member(pool: &PgPool, group: Uuid, user: Uuid) {
    sqlx::query("insert into group_members (group_id, user_id) values ($1, $2)")
        .bind(group)
        .bind(user)
        .execute(pool)
        .await
        .expect("add member");
}

async fn grant_to_group(pool: &PgPool, app: Uuid, group: Uuid) {
    sqlx::query("insert into app_groups (app_id, group_id) values ($1, $2)")
        .bind(app)
        .bind(group)
        .execute(pool)
        .await
        .expect("grant to group");
}

async fn names(pool: &PgPool, user: Uuid) -> Vec<String> {
    entitlements::apps_for_user(pool, user)
        .await
        .expect("query entitlements")
        .into_iter()
        .map(|app| app.name)
        .collect()
}

#[sqlx::test]
async fn direct_grant_only(pool: PgPool) {
    let user = insert_user(&pool, "direct@example.com", Some("Finance"), true).await;
    let granted = insert_app(&pool, "Payroll", 10).await;
    let other = insert_app(&pool, "Crm", 20).await;

    grant_to_user(&pool, granted, user).await;

    assert_eq!(names(&pool, user).await, vec!["Payroll"]);
    assert!(entitlements::is_entitled(&pool, user, granted).await.unwrap());
    assert!(!entitlements::is_entitled(&pool, user, other).await.unwrap());
}

#[sqlx::test]
async fn manual_group_grant(pool: PgPool) {
    let user = insert_user(&pool, "member@example.com", None, true).await;
    let group = insert_group(&pool, "Finance", None).await;
    let app = insert_app(&pool, "Payroll", 10).await;

    add_member(&pool, group, user).await;
    grant_to_group(&pool, app, group).await;

    assert_eq!(names(&pool, user).await, vec!["Payroll"]);
}

#[sqlx::test]
async fn auto_department_group_grant(pool: PgPool) {
    let finance_user = insert_user(&pool, "fin@example.com", Some("Finance"), true).await;
    let ops_user = insert_user(&pool, "ops@example.com", Some("Operations"), true).await;
    let group = insert_group(&pool, "Finance team", Some("Finance")).await;
    let app = insert_app(&pool, "Payroll", 10).await;

    grant_to_group(&pool, app, group).await;

    assert_eq!(names(&pool, finance_user).await, vec!["Payroll"]);
    assert!(names(&pool, ops_user).await.is_empty());
}

#[sqlx::test]
async fn auto_department_ignores_null_department(pool: PgPool) {
    let user = insert_user(&pool, "nodept@example.com", None, true).await;
    let group = insert_group(&pool, "Finance team", Some("Finance")).await;
    let app = insert_app(&pool, "Payroll", 10).await;

    grant_to_group(&pool, app, group).await;

    assert!(names(&pool, user).await.is_empty());
}

#[sqlx::test]
async fn all_users_group_grants_everyone(pool: PgPool) {
    let first = insert_user(&pool, "one@example.com", None, true).await;
    let second = insert_user(&pool, "two@example.com", Some("HR"), true).await;
    let group = all_users_group(&pool).await;
    let app = insert_app(&pool, "Leave portal", 10).await;

    grant_to_group(&pool, app, group).await;

    assert_eq!(names(&pool, first).await, vec!["Leave portal"]);
    assert_eq!(names(&pool, second).await, vec!["Leave portal"]);
}

#[sqlx::test]
async fn union_of_direct_and_group_does_not_duplicate(pool: PgPool) {
    let user = insert_user(&pool, "both@example.com", Some("Finance"), true).await;
    let group = insert_group(&pool, "Finance", None).await;
    let app = insert_app(&pool, "Payroll", 10).await;

    grant_to_user(&pool, app, user).await;
    add_member(&pool, group, user).await;
    grant_to_group(&pool, app, group).await;

    assert_eq!(names(&pool, user).await, vec!["Payroll"]);
}

#[sqlx::test]
async fn disabled_app_is_hidden(pool: PgPool) {
    let user = insert_user(&pool, "disabled@example.com", None, true).await;
    let app = insert_app(&pool, "Retired", 10).await;

    grant_to_user(&pool, app, user).await;
    sqlx::query("update apps set is_enabled = false where id = $1")
        .bind(app)
        .execute(&pool)
        .await
        .unwrap();

    assert!(names(&pool, user).await.is_empty());
}

#[sqlx::test]
async fn soft_deleted_app_is_hidden(pool: PgPool) {
    let user = insert_user(&pool, "deleted@example.com", None, true).await;
    let app = insert_app(&pool, "Gone", 10).await;

    grant_to_user(&pool, app, user).await;
    sqlx::query("update apps set deleted_at = now() where id = $1")
        .bind(app)
        .execute(&pool)
        .await
        .unwrap();

    assert!(names(&pool, user).await.is_empty());
}

#[sqlx::test]
async fn deactivated_user_gets_nothing(pool: PgPool) {
    let user = insert_user(&pool, "inactive@example.com", None, false).await;
    let app = insert_app(&pool, "Payroll", 10).await;

    grant_to_user(&pool, app, user).await;

    assert!(names(&pool, user).await.is_empty());
}

#[sqlx::test]
async fn grant_to_another_group_does_not_leak(pool: PgPool) {
    let user = insert_user(&pool, "outside@example.com", None, true).await;
    let their_group = insert_group(&pool, "Operations", None).await;
    let other_group = insert_group(&pool, "Finance", None).await;
    let finance_app = insert_app(&pool, "Payroll", 10).await;

    add_member(&pool, their_group, user).await;
    grant_to_group(&pool, finance_app, other_group).await;

    assert!(names(&pool, user).await.is_empty());
}

#[sqlx::test]
async fn results_are_ordered_by_sort_then_name(pool: PgPool) {
    let user = insert_user(&pool, "sorted@example.com", None, true).await;
    let later = insert_app(&pool, "Zebra", 20).await;
    let first = insert_app(&pool, "Beta", 10).await;
    let tied = insert_app(&pool, "Alpha", 10).await;

    for app in [later, first, tied] {
        grant_to_user(&pool, app, user).await;
    }

    assert_eq!(names(&pool, user).await, vec!["Alpha", "Beta", "Zebra"]);
}
