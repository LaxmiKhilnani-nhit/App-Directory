//! CSV store round-trip tests: settings, update flag, atomic files and hand-edit reload.

mod support;

use std::fs;

use serde_json::json;
use support::*;

#[tokio::test]
async fn settings_round_trip_and_revision_bump() {
    let h = setup();
    add_admin_user(&h, "admin", "super_admin");
    let identity = "ad\\admin";
    let (_, before) = get(&h, "/api/v1/admin/stats", Some(identity)).await;
    let revision_before = before["revision"].as_i64().unwrap();

    let (status, body) = json(
        &h,
        "PUT",
        "/api/v1/admin/settings",
        Some(identity),
        json!({ "values": {
            "sync_interval_hours": 6,
            "announcement": { "text": "Payroll moves Monday", "level": "info" },
            "mail_domain": "nhit.co.in"
        }}),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK, "{body}");
    assert!(body.as_array().is_some());

    let (status, settings) = get(&h, "/api/v1/admin/settings", Some(identity)).await;
    assert_eq!(status, axum::http::StatusCode::OK, "settings: {settings}");
    let values: serde_json::Map<String, serde_json::Value> = settings
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            (
                row["key"].as_str().unwrap().to_string(),
                row["value"].clone(),
            )
        })
        .collect();
    assert_eq!(values["sync_interval_hours"], json!(6));
    assert_eq!(values["mail_domain"], json!("nhit.co.in"));
    assert_eq!(
        values["announcement"]["text"],
        json!("Payroll moves Monday")
    );

    let (_, after) = get(&h, "/api/v1/admin/stats", Some(identity)).await;
    assert_eq!(after["revision"].as_i64().unwrap(), revision_before + 1);

    // Every required CSV file exists and settings were persisted to disk.
    for name in [
        "users.csv",
        "groups.csv",
        "group_members.csv",
        "apps.csv",
        "app_groups.csv",
        "user_apps.csv",
        "settings.csv",
        "audit.csv",
        "revision.csv",
    ] {
        assert!(h.dir.join(name).is_file(), "missing {name}");
    }
    let text = fs::read_to_string(h.dir.join("settings.csv")).unwrap();
    assert!(text.contains("sync_interval_hours"), "{text}");
    assert!(text.contains("nhit.co.in"), "{text}");
}

#[tokio::test]
async fn hand_edited_csv_is_reloaded() {
    let h = setup();
    add_admin_user(&h, "admin", "super_admin");
    let identity = "ad\\admin";

    let path = h.dir.join("settings.csv");
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    let original = fs::read_to_string(&path).unwrap();
    let edited = original
        .replace("\"sync_interval_hours\",4", "\"sync_interval_hours\",12")
        .replace("sync_interval_hours,4", "sync_interval_hours,12");
    assert_ne!(original, edited, "test fixture did not change the file");
    fs::write(&path, edited).unwrap();

    let (_, settings) = get(&h, "/api/v1/admin/settings", Some(identity)).await;
    let value = settings
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["key"] == "sync_interval_hours")
        .unwrap()["value"]
        .clone();
    assert_eq!(value, json!(12));
}

#[tokio::test]
async fn update_flag_is_driven_by_settings() {
    let h = setup();
    add_admin_user(&h, "admin", "super_admin");
    let identity = "ad\\admin";

    let (status, initial) = get(&h, "/api/v1/client/update", None).await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(initial["available"], json!(false));

    let (status, published) = json(
        &h,
        "POST",
        "/api/v1/admin/update/publish",
        Some(identity),
        json!({ "version": "1.2.3", "sha256": "abc123" }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK, "{published}");
    assert_eq!(published["available"], json!(true));
    assert_eq!(published["version"], json!("1.2.3"));
    assert_eq!(published["sha256"], json!("abc123"));

    let (_, fetched) = get(&h, "/api/v1/client/update", None).await;
    assert_eq!(fetched["available"], json!(true));
    assert_eq!(fetched["version"], json!("1.2.3"));

    // Toggling through the ordinary settings endpoint switches it back off.
    json(
        &h,
        "PUT",
        "/api/v1/admin/settings",
        Some(identity),
        json!({ "values": { "update_enabled": 0 } }),
    )
    .await;
    let (_, disabled) = get(&h, "/api/v1/client/update", None).await;
    assert_eq!(disabled["available"], json!(false));
}
