//! Super-admin management of the shared phone directory. One CSV file holds every entry;
//! client sync hands the whole list to every user, so no entitlement checks happen here.

use axum::{
    extract::{rejection::JsonRejection, Path, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::domain::models::PhoneEntry;
use crate::domain::validate;
use crate::error::{ApiError, ApiResult};
use crate::http::identity::TrustedUser;
use crate::http::json_body;
use crate::state::AppState;
use crate::store::PhoneDirectoryRow;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewPhoneEntry {
    #[serde(default)]
    pub first_name: String,
    #[serde(default)]
    pub last_name: String,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub phone_no: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoneEntryPatch {
    #[serde(default)]
    pub first_name: Option<String>,
    #[serde(default)]
    pub last_name: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub phone_no: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ImportRequest {
    pub csv: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportError {
    pub row: usize,
    pub message: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub created: i64,
    pub updated: i64,
    pub errors: Vec<ImportError>,
}

fn clean(value: &str) -> String {
    validate::sanitize(Some(value.to_string())).unwrap_or_default()
}

fn clean_email(value: &str) -> String {
    clean(value).to_lowercase()
}

/// Extensions are normally numeric, but accept the punctuation people paste in from Outlook.
fn valid_phone(value: &str) -> bool {
    !value.is_empty()
        && value.chars().count() <= 60
        && value.chars().any(|character| character.is_ascii_digit())
        && value.chars().all(|character| {
            character.is_ascii_digit() || matches!(character, ' ' | '-' | '+' | '(' | ')' | '.')
        })
}

fn check_email(email: &str) -> ApiResult<()> {
    if email.len() <= 254 && email.contains('@') && !email.starts_with('@') {
        Ok(())
    } else {
        Err(ApiError::bad_request(
            "invalid_email",
            "Email must be blank or a valid address.",
        ))
    }
}

fn has_identity(first_name: &str, last_name: &str, email: &str) -> bool {
    !first_name.is_empty() || !last_name.is_empty() || !email.is_empty()
}

pub async fn list(
    State(state): State<AppState>,
    TrustedUser(_actor): TrustedUser,
) -> ApiResult<Json<Vec<PhoneEntry>>> {
    let data = state.store.read();
    Ok(Json(PhoneEntry::sorted_from(&data)))
}

pub async fn create(
    State(state): State<AppState>,
    TrustedUser(actor): TrustedUser,
    body: Result<Json<NewPhoneEntry>, JsonRejection>,
) -> ApiResult<(StatusCode, Json<PhoneEntry>)> {
    let input = json_body(body)?;
    let first_name = clean(&input.first_name);
    let last_name = clean(&input.last_name);
    let email = clean_email(&input.email);
    let phone_no = clean(&input.phone_no);

    if !valid_phone(&phone_no) {
        return Err(ApiError::bad_request(
            "invalid_phone_no",
            "phoneNo is required and must be a numeric extension.",
        ));
    }
    check_email(&email)?;
    if !has_identity(&first_name, &last_name, &email) {
        return Err(ApiError::bad_request(
            "invalid_entry",
            "At least a name or an email address is required.",
        ));
    }

    let id = Uuid::new_v4();
    let now = Utc::now();
    let view = state.store.mutate(|data| {
        data.phone_directory.push(PhoneDirectoryRow {
            id,
            first_name,
            last_name,
            email,
            phone_no,
            created_at: now,
            updated_at: now,
        });
        data.bump_revision();
        let view = PhoneEntry::from_row(
            data.phone_directory
                .iter()
                .find(|row| row.id == id)
                .expect("just inserted"),
        );
        data.audit(
            Some(actor.id),
            Some(actor.email.clone()),
            "phone_directory.create",
            "phone_directory",
            Some(id),
            None,
            Some(json!(view)),
        );
        Ok(view)
    })?;

    Ok((StatusCode::CREATED, Json(view)))
}

pub async fn update(
    State(state): State<AppState>,
    TrustedUser(actor): TrustedUser,
    Path(id): Path<Uuid>,
    body: Result<Json<PhoneEntryPatch>, JsonRejection>,
) -> ApiResult<Json<PhoneEntry>> {
    let patch = json_body(body)?;

    state.store.mutate(|data| {
        let before_row = data
            .phone_directory
            .iter()
            .find(|row| row.id == id)
            .ok_or_else(|| {
                ApiError::not_found(
                    "phone_entry_not_found",
                    "That directory entry no longer exists.",
                )
            })?
            .clone();
        let before = PhoneEntry::from_row(&before_row);
        let mut next = before_row.clone();

        if let Some(value) = patch.first_name {
            next.first_name = clean(&value);
        }
        if let Some(value) = patch.last_name {
            next.last_name = clean(&value);
        }
        if let Some(value) = patch.email {
            next.email = clean_email(&value);
            check_email(&next.email)?;
        }
        if let Some(value) = patch.phone_no {
            next.phone_no = clean(&value);
            if !valid_phone(&next.phone_no) {
                return Err(ApiError::bad_request(
                    "invalid_phone_no",
                    "phoneNo must be a numeric extension.",
                ));
            }
        }
        if !has_identity(&next.first_name, &next.last_name, &next.email) {
            return Err(ApiError::bad_request(
                "invalid_entry",
                "At least a name or an email address is required.",
            ));
        }

        next.updated_at = Utc::now();
        let row = data
            .phone_directory
            .iter_mut()
            .find(|row| row.id == id)
            .expect("exists");
        *row = next;

        data.bump_revision();
        let view = PhoneEntry::from_row(
            data.phone_directory
                .iter()
                .find(|r| r.id == id)
                .expect("exists"),
        );
        data.audit(
            Some(actor.id),
            Some(actor.email.clone()),
            "phone_directory.update",
            "phone_directory",
            Some(id),
            Some(json!(before)),
            Some(json!(view)),
        );
        Ok(Json(view))
    })
}

pub async fn delete(
    State(state): State<AppState>,
    TrustedUser(actor): TrustedUser,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    state.store.mutate(|data| {
        let before_row = data
            .phone_directory
            .iter()
            .find(|row| row.id == id)
            .ok_or_else(|| {
                ApiError::not_found(
                    "phone_entry_not_found",
                    "That directory entry no longer exists.",
                )
            })?
            .clone();
        data.phone_directory.retain(|row| row.id != id);

        data.bump_revision();
        data.audit(
            Some(actor.id),
            Some(actor.email.clone()),
            "phone_directory.delete",
            "phone_directory",
            Some(id),
            Some(json!(PhoneEntry::from_row(&before_row))),
            None,
        );
        Ok(())
    })?;
    Ok(StatusCode::NO_CONTENT)
}

// ------------------------------------------------------------------ bulk import

struct ParsedRow {
    line: usize,
    first_name: String,
    last_name: String,
    email: String,
    phone_no: String,
}

/// Pasted spreadsheets arrive either comma- or tab-separated. Decide per line, then use the
/// same RFC-4180-aware splitter as the rest of the API so quoted commas are preserved.
fn split_row(line: &str) -> Vec<String> {
    let delimiter = if line.contains('\t') { '\t' } else { ',' };
    validate::split_csv_line_delimited(line, delimiter)
}

fn normalized_header(value: &str) -> String {
    value
        .trim()
        .to_lowercase()
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .collect()
}

fn looks_like_header(fields: &[String]) -> bool {
    fields.iter().any(|field| {
        matches!(
            normalized_header(field).as_str(),
            "firstname"
                | "lastname"
                | "email"
                | "emailaddress"
                | "workemail"
                | "extensionnumber"
                | "extension"
                | "phonenumber"
                | "phoneno"
                | "phone"
        )
    })
}

fn parse_import(text: &str) -> Result<Vec<ParsedRow>, ApiError> {
    // Excel writes a UTF-8 BOM, which would otherwise become part of the first header.
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut lines: Vec<(usize, &str)> = text
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(index, line)| (index + 1, line))
        .collect();
    if lines.is_empty() {
        return Err(ApiError::bad_request("invalid_csv", "The file is empty."));
    }

    let mut skip = 0;
    let mut indices = [0usize, 1, 2, 3];
    if looks_like_header(&split_row(lines[0].1)) {
        let headers: Vec<String> = split_row(lines[0].1)
            .iter()
            .map(|field| normalized_header(field))
            .collect();
        let index_of = |names: &[&str], fallback: usize| {
            headers
                .iter()
                .position(|header| names.iter().any(|name| *name == header))
                .unwrap_or(fallback)
        };
        indices = [
            index_of(&["firstname", "first", "givenname"], 0),
            index_of(&["lastname", "last", "surname"], 1),
            index_of(&["email", "emailaddress", "workemail", "emailid"], 2),
            index_of(
                &[
                    "extensionnumber",
                    "extension",
                    "phonenumber",
                    "phoneno",
                    "phone",
                ],
                3,
            ),
        ];
        skip = 1;
    }

    let mut rows = Vec::new();
    for (line, raw) in lines.drain(..).skip(skip) {
        let fields = split_row(raw);
        let field = |index: usize| clean(fields.get(index).map(String::as_str).unwrap_or(""));
        rows.push(ParsedRow {
            line,
            first_name: field(indices[0]),
            last_name: field(indices[1]),
            email: field(indices[2]),
            phone_no: field(indices[3]),
        });
    }
    Ok(rows)
}

pub async fn import(
    State(state): State<AppState>,
    TrustedUser(actor): TrustedUser,
    body: Result<Json<ImportRequest>, JsonRejection>,
) -> ApiResult<Json<ImportReport>> {
    let request = json_body(body)?;
    let rows = parse_import(&request.csv)?;

    state.store.mutate(|data| {
        let mut report = ImportReport {
            created: 0,
            updated: 0,
            errors: Vec::new(),
        };
        let now = Utc::now();

        for row in rows {
            // Blank phone numbers are the commonest artefact of pasting a whole sheet.
            if row.phone_no.is_empty() {
                continue;
            }
            let email = clean_email(&row.email);
            if !email.is_empty() && !email.contains('@') {
                report.errors.push(ImportError {
                    row: row.line,
                    message: "Email address is not valid.".to_string(),
                });
                continue;
            }

            match data.phone_directory.iter().position(|entry| {
                if !email.is_empty() {
                    entry.email.trim().eq_ignore_ascii_case(&email)
                } else {
                    entry.phone_no.trim() == row.phone_no
                }
            }) {
                Some(index) => {
                    let entry = &mut data.phone_directory[index];
                    entry.first_name = row.first_name;
                    entry.last_name = row.last_name;
                    entry.email = email;
                    entry.phone_no = row.phone_no;
                    entry.updated_at = now;
                    report.updated += 1;
                }
                None => {
                    data.phone_directory.push(PhoneDirectoryRow {
                        id: Uuid::new_v4(),
                        first_name: row.first_name,
                        last_name: row.last_name,
                        email,
                        phone_no: row.phone_no,
                        created_at: now,
                        updated_at: now,
                    });
                    report.created += 1;
                }
            }
        }

        if report.created > 0 || report.updated > 0 {
            data.bump_revision();
        }
        data.audit(
            Some(actor.id),
            Some(actor.email.clone()),
            "phone_directory.import",
            "phone_directory",
            None,
            None,
            Some(json!({
                "created": report.created,
                "updated": report.updated,
                "errors": report.errors.len(),
            })),
        );
        Ok(Json(report))
    })
}
