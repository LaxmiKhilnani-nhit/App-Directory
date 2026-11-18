//! Entitlement tests over a temporary CSV store, driven through the real HTTP router.

mod support;

use support::*;

async fn app_names(h: &Harness, local: &str) -> Vec<String> {
    let (status, payload) = get(
        h,
        "/api/v1/client/sync",
        Some(&format!("ad\\{local}")),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK, "{payload}");
    payload["apps"]
        .as_array()
        .expect("apps array")
        .iter()
        .map(|app| app["name"].as_str().unwrap().to_string())
        .collect()
}

#[tokio::test]
async fn direct_grant_only() {
    let h = setup();
    let user = add_user(&h, "direct", Some("Finance"), true);
    let granted = add_app(&h, "Payroll", 10);
    let other = add_app(&h, "Crm", 20);
    grant_user(&h, user, granted);

    assert_eq!(app_names(&h, "direct").await, vec!["Payroll"]);
    assert!(!app_names(&h, "direct").await.contains(&"Crm".to_string()));
    assert!(!other.is_nil());
}

#[tokio::test]
async fn manual_group_grant() {
    let h = setup();
    let user = add_user(&h, "member", None, true);
    let group = add_group(&h, "Finance", None);
    let app = add_app(&h, "Payroll", 10);
    add_member(&h, group, user);
    grant_group(&h, group, app);

    assert_eq!(app_names(&h, "member").await, vec!["Payroll"]);
}

#[tokio::test]
async fn auto_department_group_and_null_department() {
    let h = setup();
    let finance = add_user(&h, "fin", Some("Finance"), true);
    let ops = add_user(&h, "ops", Some("Operations"), true);
    let none = add_user(&h, "nodept", None, true);
    let group = add_group(&h, "Finance team", Some("Finance"));
    let app = add_app(&h, "Payroll", 10);
    grant_group(&h, group, app);

    assert_eq!(app_names(&h, "fin").await, vec!["Payroll"]);
    assert!(app_names(&h, "ops").await.is_empty());
    assert!(app_names(&h, "nodept").await.is_empty());
    assert!(!finance.is_nil() && !ops.is_nil() && !none.is_nil());
}

#[tokio::test]
async fn all_users_group_grants_everyone() {
    let h = setup();
    add_user(&h, "one", None, true);
    add_user(&h, "two", Some("HR"), true);
    let app = add_app(&h, "Leave portal", 10);
    grant_group(&h, app_directory_api::store::ALL_USERS_GROUP_ID, app);

    assert_eq!(app_names(&h, "one").await, vec!["Leave portal"]);
    assert_eq!(app_names(&h, "two").await, vec!["Leave portal"]);
}

#[tokio::test]
async fn union_deduplicates_and_orders_by_sort_then_name() {
    let h = setup();
    let user = add_user(&h, "both", Some("Finance"), true);
    let group = add_group(&h, "Finance", None);
    add_member(&h, group, user);

    let zebra = add_app(&h, "Zebra", 20);
    let beta = add_app(&h, "Beta", 10);
    let alpha = add_app(&h, "Alpha", 10);
    for app in [zebra, beta, alpha] {
        grant_user(&h, user, app);
        grant_group(&h, group, app);
        grant_group(&h, app_directory_api::store::ALL_USERS_GROUP_ID, app);
    }

    assert_eq!(
        app_names(&h, "both").await,
        vec!["Alpha", "Beta", "Zebra"]
    );
}

#[tokio::test]
async fn disabled_deleted_apps_and_inactive_user_are_hidden() {
    let h = setup();
    let active = add_user(&h, "active", None, true);
    let inactive = add_user(&h, "inactive", None, false);

    let disabled = add_app(&h, "Disabled", 10);
    let deleted = add_app(&h, "Deleted", 20);
    let live = add_app(&h, "Live", 30);
    for app in [disabled, deleted, live] {
        grant_user(&h, active, app);
        grant_user(&h, inactive, app);
    }
    set_app_enabled(&h, disabled, false);
    set_app_deleted(&h, deleted);

    assert_eq!(app_names(&h, "active").await, vec!["Live"]);
    assert!(app_names(&h, "inactive").await.is_empty());
}

#[tokio::test]
async fn grant_to_another_group_does_not_leak() {
    let h = setup();
    let user = add_user(&h, "outside", None, true);
    let theirs = add_group(&h, "Operations", None);
    let other = add_group(&h, "Finance", None);
    let app = add_app(&h, "Payroll", 10);
    add_member(&h, theirs, user);
    grant_group(&h, other, app);

    assert!(app_names(&h, "outside").await.is_empty());
}
