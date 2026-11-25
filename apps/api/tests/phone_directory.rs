//! Phone directory: client sync payload and admin CRUD/import contract.

mod support;

use std::fs;

use axum::http::StatusCode;
use serde_json::{json, Value};
use support::*;

const ADMIN: &str = "ad\\admin";

#[tokio::test]
async fn sync_includes_the_full_phone_directory() {
    let h = setup();
    add_user(&h, "known", None, true);

    let (status, payload) = get(&h, "/api/v1/client/sync", Some("ad\\known")).await;
    assert_eq!(status, StatusCode::OK, "{payload}");

    let entries = payload["phoneDirectory"]
        .as_array()
        .expect("phoneDirectory must be an array");
    assert!(!entries.is_empty(), "phone directory must be seeded");
    assert!(
        entries
            .iter()
            .any(|entry| entry["email"] == json!("frontoffice@nhit.co.in")),
        "the embedded seed should have been imported"
    );

    for entry in entries {
        assert!(entry["id"].as_str().is_some(), "{entry}");
        assert!(entry["firstName"].is_string(), "{entry}");
        assert!(entry["lastName"].is_string(), "{entry}");
        assert!(entry["email"].is_string(), "{entry}");
        assert!(entry["phoneNo"].is_string(), "{entry}");
    }

    let numbers: Vec<u64> = entries
        .iter()
        .filter_map(|entry| entry["phoneNo"].as_str()?.parse().ok())
        .collect();
    let mut sorted = numbers.clone();
    sorted.sort_unstable();
    assert_eq!(numbers, sorted, "entries must be sorted by phone number");
}

#[tokio::test]
async fn phone_directory_csv_round_trips_and_hand_edits_reload() {
    let h = setup();
    add_user(&h, "known", None, true);

    let path = h.dir.join("phone_directory.csv");
    let original = fs::read_to_string(&path).expect("seeded phone directory file");
    assert_eq!(
        original.lines().next().unwrap(),
        "id,firstName,lastName,email,phoneNo,createdAt,updatedAt"
    );
    assert_eq!(original.lines().count(), 184, "header + 183 seed rows");

    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    let edited = original.replace("frontoffice@nhit.co.in,3000", "frontoffice@nhit.co.in,3999");
    assert_ne!(original, edited, "test fixture did not change the file");
    fs::write(&path, edited).unwrap();

    let (_, payload) = get(&h, "/api/v1/client/sync", Some("ad\\known")).await;
    let entry = payload["phoneDirectory"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["email"] == json!("frontoffice@nhit.co.in"))
        .expect("edited entry")
        .clone();
    assert_eq!(entry["phoneNo"], json!("3999"));
}

#[tokio::test]
async fn admin_can_create_list_update_and_delete_an_entry() {
    let h = setup();

    let (status, _) = get(&h, "/api/v1/admin/phone-directory", None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, created) = json(
        &h,
        "POST",
        "/api/v1/admin/phone-directory",
        Some(ADMIN),
        json!({
            "firstName": "Test",
            "lastName": "Reception",
            "email": "test.reception@nhit.co.in",
            "phoneNo": "990001"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    assert_eq!(created["firstName"], json!("Test"));
    assert_eq!(created["lastName"], json!("Reception"));
    assert_eq!(created["email"], json!("test.reception@nhit.co.in"));
    assert_eq!(created["phoneNo"], json!("990001"));
    let id = created["id"].as_str().unwrap().to_string();

    let (status, listed) = get(&h, "/api/v1/admin/phone-directory", Some(ADMIN)).await;
    assert_eq!(status, StatusCode::OK, "{listed}");
    assert!(listed
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry["id"] == json!(id)));

    let (status, updated) = json(
        &h,
        "PATCH",
        &format!("/api/v1/admin/phone-directory/{id}"),
        Some(ADMIN),
        json!({ "lastName": "Front Desk", "phoneNo": "990002" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(updated["lastName"], json!("Front Desk"));
    assert_eq!(updated["phoneNo"], json!("990002"));
    assert_eq!(updated["email"], json!("test.reception@nhit.co.in"));

    let (status, _) = json(
        &h,
        "DELETE",
        &format!("/api/v1/admin/phone-directory/{id}"),
        Some(ADMIN),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (_, listed) = get(&h, "/api/v1/admin/phone-directory?x=1", Some(ADMIN)).await;
    assert!(!listed
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry["id"] == json!(id)));
}

#[tokio::test]
async fn import_upserts_rows_and_reports_counts() {
    let h = setup();

    // TSV with a header, a quoted CSV field on the next import, and a blank phone to skip.
    let tsv = "First Name\tLast Name\tEmail Address\tExtension Number\n\
               Alice\tImport\talice.import@nhit.co.in\t990010\n\
               Bob\tImport\tbob.import@nhit.co.in\t990011\n\
               Nobody\tBlank\t\t\n";
    let (status, report) = json(
        &h,
        "POST",
        "/api/v1/admin/phone-directory/import",
        Some(ADMIN),
        json!({ "csv": tsv }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{report}");
    assert_eq!(report["created"], json!(2), "{report}");
    assert_eq!(report["updated"], json!(0), "{report}");
    assert_eq!(report["errors"], json!([]), "{report}");

    // Re-import: one row matches by email, another by phone number with a blank email.
    let csv = "First Name,Last Name,Email Address,Extension Number\n\
               Alicia,Import,alice.import@nhit.co.in,990010\n\
               Bob,Updated,,990011\n";
    let (status, report) = json(
        &h,
        "POST",
        "/api/v1/admin/phone-directory/import",
        Some(ADMIN),
        json!({ "csv": csv }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{report}");
    assert_eq!(report["created"], json!(0), "{report}");
    assert_eq!(report["updated"], json!(2), "{report}");

    let (_, listed) = get(&h, "/api/v1/admin/phone-directory", Some(ADMIN)).await;
    let find = |email: &str| -> Value {
        listed
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["email"] == json!(email))
            .cloned()
            .expect("imported entry")
    };
    assert_eq!(
        find("alice.import@nhit.co.in")["firstName"],
        json!("Alicia")
    );

    let bob = listed
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["phoneNo"] == json!("990011"))
        .expect("entry matched by phone number");
    assert_eq!(bob["lastName"], json!("Updated"));
    assert_eq!(bob["email"], json!(""));
}
