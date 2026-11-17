use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{FromRow, PgPool, Postgres};
use uuid::Uuid;

use crate::domain::{audit, auth, is_unique_violation, revision};
use crate::error::ApiError;

pub const ROLES: [&str; 3] = ["user", "app_admin", "super_admin"];

/// Whether an admin may change another account's role to `role`.
pub fn is_role(role: &str) -> bool {
    ROLES.contains(&role)
}

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct AdminUser {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    pub department: Option<String>,
    pub role: String,
    pub admin_scope: Option<serde_json::Value>,
    pub is_active: bool,
    pub must_change_password: bool,
    pub last_login_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub has_password: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserPage {
    pub users: Vec<AdminUser>,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserDetail {
    #[serde(flatten)]
    pub user: AdminUser,
    /// Explicit group memberships. Department-derived groups are not listed here.
    pub group_ids: Vec<Uuid>,
    /// Direct app grants; effective access also includes group and All Users grants.
    pub app_ids: Vec<Uuid>,
}

const SELECT_USER: &str = r#"
select id, email, name, department, role, admin_scope, is_active, must_change_password,
       last_login_at, created_at, updated_at,
       (password_hash is not null) as has_password
  from users
"#;

pub async fn list(
    pool: &PgPool,
    query: Option<&str>,
    role: Option<&str>,
    active: Option<bool>,
    limit: i64,
    offset: i64,
) -> sqlx::Result<UserPage> {
    let pattern = query
        .map(|text| text.trim().to_lowercase())
        .filter(|text| !text.is_empty())
        .map(|text| format!("%{text}%"));

    let users = sqlx::query_as::<_, AdminUser>(&format!(
        "{SELECT_USER}
         where ($1::text is null or lower(email) like $1 or lower(name) like $1)
           and ($2::text is null or role = $2)
           and ($3::bool is null or is_active = $3)
         order by name, email
         limit $4 offset $5"
    ))
    .bind(&pattern)
    .bind(role)
    .bind(active)
    .bind(limit.clamp(1, 500))
    .bind(offset.max(0))
    .fetch_all(pool)
    .await?;

    let total: i64 = sqlx::query_scalar(
        "select count(*) from users
          where ($1::text is null or lower(email) like $1 or lower(name) like $1)
            and ($2::text is null or role = $2)
            and ($3::bool is null or is_active = $3)",
    )
    .bind(&pattern)
    .bind(role)
    .bind(active)
    .fetch_one(pool)
    .await?;

    Ok(UserPage { users, total })
}

async fn load<'e, E>(executor: E, id: Uuid) -> sqlx::Result<Option<AdminUser>>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    sqlx::query_as::<_, AdminUser>(&format!("{SELECT_USER} where id = $1"))
        .bind(id)
        .fetch_optional(executor)
        .await
}

pub async fn get(pool: &PgPool, id: Uuid) -> Result<Option<UserDetail>, ApiError> {
    let Some(user) = load(pool, id).await? else {
        return Ok(None);
    };

    let group_ids: Vec<Uuid> =
        sqlx::query_scalar("select group_id from group_members where user_id = $1 order by created_at")
            .bind(id)
            .fetch_all(pool)
            .await?;

    let app_ids: Vec<Uuid> =
        sqlx::query_scalar("select app_id from user_apps where user_id = $1 order by created_at")
            .bind(id)
            .fetch_all(pool)
            .await?;

    Ok(Some(UserDetail {
        user,
        group_ids,
        app_ids,
    }))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewUser {
    pub email: String,
    pub name: String,
    #[serde(default)]
    pub department: Option<String>,
    #[serde(default = "default_role")]
    pub role: String,
    /// Only meaningful for `app_admin`; ignored (stored as null) for other roles.
    #[serde(default)]
    pub admin_scope: Option<Value>,
}

fn default_role() -> String {
    "user".to_string()
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserPatch {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "crate::domain::de::double_option")]
    pub department: Option<Option<String>>,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub is_active: Option<bool>,
    /// Absent keeps the stored scope, `null` clears it, an object replaces it.
    #[serde(default, deserialize_with = "crate::domain::de::double_option")]
    pub admin_scope: Option<Option<Value>>,
}

fn clean(value: Option<String>) -> Option<String> {
    value
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

pub fn validate_email(email: &str) -> Result<String, ApiError> {
    let trimmed = email.trim().to_lowercase();
    let valid = trimmed.len() >= 3
        && trimmed.len() <= 254
        && trimmed.contains('@')
        && !trimmed.contains(char::is_whitespace)
        && !trimmed.starts_with('@')
        && !trimmed.ends_with('@');

    if !valid {
        return Err(ApiError::bad_request(
            "invalid_email",
            "That email address is not valid.",
        ));
    }

    Ok(trimmed)
}

fn validate_name(name: &str) -> Result<String, ApiError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(ApiError::bad_request("invalid_name", "Name is required."));
    }
    if trimmed.chars().count() > 120 {
        return Err(ApiError::bad_request(
            "invalid_name",
            "Name must be 120 characters or fewer.",
        ));
    }
    Ok(trimmed.to_string())
}

fn validate_department(value: Option<String>) -> Result<Option<String>, ApiError> {
    let cleaned = clean(value);
    if let Some(department) = cleaned.as_deref() {
        if department.chars().count() > 120 {
            return Err(ApiError::bad_request(
                "invalid_department",
                "Department must be 120 characters or fewer.",
            ));
        }
    }
    Ok(cleaned)
}

fn validate_role(role: &str) -> Result<String, ApiError> {
    if !is_role(role) {
        return Err(ApiError::bad_request(
            "invalid_role",
            "Role must be user, app_admin or super_admin.",
        ));
    }
    Ok(role.to_string())
}

/// Normalises an `app_admin` scope to `{"categories": [...]}`.
///
/// Only categories are supported in v1. Values are trimmed, de-duplicated
/// case-insensitively and capped, so scope comparison can stay a plain string compare.
fn validate_scope(value: Value) -> Result<Value, ApiError> {
    let object = value.as_object().ok_or_else(|| {
        ApiError::bad_request(
            "invalid_scope",
            "adminScope must be an object with a categories array.",
        )
    })?;

    let raw = match object.get("categories") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(values)) => values.clone(),
        Some(_) => {
            return Err(ApiError::bad_request(
                "invalid_scope",
                "adminScope.categories must be an array of category names.",
            ))
        }
    };

    if raw.len() > 50 {
        return Err(ApiError::bad_request(
            "invalid_scope",
            "An app admin can be limited to at most 50 categories.",
        ));
    }

    let mut categories: Vec<String> = Vec::with_capacity(raw.len());
    for entry in raw {
        let category = entry.as_str().ok_or_else(|| {
            ApiError::bad_request(
                "invalid_scope",
                "adminScope.categories must contain only strings.",
            )
        })?;
        let category = category.trim();

        if category.is_empty() {
            return Err(ApiError::bad_request(
                "invalid_scope",
                "A category in adminScope cannot be blank.",
            ));
        }
        if category.chars().count() > 80 {
            return Err(ApiError::bad_request(
                "invalid_scope",
                "A category name must be 80 characters or fewer.",
            ));
        }
        if !categories
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(category))
        {
            categories.push(category.to_string());
        }
    }

    Ok(json!({ "categories": categories }))
}

/// The stored scope for a role: `app_admin` keeps categories, everyone else has none.
fn scope_for(role: &str, value: Option<Value>) -> Result<Option<Value>, ApiError> {
    if role != "app_admin" {
        return Ok(None);
    }

    match value {
        Some(value) => validate_scope(value).map(Some),
        None => Ok(Some(json!({ "categories": [] }))),
    }
}

pub async fn create(
    pool: &PgPool,
    actor: Uuid,
    input: NewUser,
    ip: Option<&str>,
) -> Result<AdminUser, ApiError> {
    let email = validate_email(&input.email)?;
    let name = validate_name(&input.name)?;
    let department = validate_department(input.department)?;
    let role = validate_role(&input.role)?;
    let scope = scope_for(&role, input.admin_scope)?;

    let mut transaction = pool.begin().await?;

    // No password: an admin then issues an onboarding code, which is the only way in.
    let id: Uuid = sqlx::query_scalar(
        "insert into users (email, name, department, role, must_change_password, admin_scope)
         values ($1, $2, $3, $4, true, $5)
         returning id",
    )
    .bind(&email)
    .bind(&name)
    .bind(&department)
    .bind(&role)
    .bind(&scope)
    .fetch_one(&mut *transaction)
    .await
    .map_err(|error| {
        if is_unique_violation(&error) {
            ApiError::conflict("email_taken", "An account with that email already exists.")
        } else {
            ApiError::from(error)
        }
    })?;

    let user = load(&mut *transaction, id)
        .await?
        .expect("the row was just inserted");

    audit::record_tx(
        &mut transaction,
        Some(actor),
        "user.create",
        "user",
        Some(id),
        None,
        Some(serde_json::to_value(&user).unwrap_or(serde_json::Value::Null)),
        ip,
    )
    .await;
    revision::bump(&mut transaction).await?;

    transaction.commit().await?;

    Ok(user)
}

/// Counts *other* active super admins, so the last one cannot be demoted or deactivated.
async fn other_super_admins(pool: &PgPool, id: Uuid) -> sqlx::Result<i64> {
    sqlx::query_scalar(
        "select count(*) from users
          where role = 'super_admin' and is_active and id <> $1",
    )
    .bind(id)
    .fetch_one(pool)
    .await
}

pub async fn update(
    pool: &PgPool,
    actor: Uuid,
    id: Uuid,
    patch: UserPatch,
    ip: Option<&str>,
) -> Result<AdminUser, ApiError> {
    let before = load(pool, id)
        .await?
        .ok_or_else(|| ApiError::not_found("user_not_found", "That user no longer exists."))?;

    let name = match patch.name {
        Some(value) => validate_name(&value)?,
        None => before.name.clone(),
    };
    let department = match patch.department {
        Some(value) => validate_department(value)?,
        None => before.department.clone(),
    };
    let role = match patch.role {
        Some(value) => validate_role(&value)?,
        None => before.role.clone(),
    };
    let is_active = patch.is_active.unwrap_or(before.is_active);

    // Demoting an app admin clears their scope; promoting one starts them with no categories.
    let admin_scope = if role != "app_admin" {
        None
    } else {
        match patch.admin_scope {
            Some(Some(value)) => validate_scope(value).map(Some)?,
            Some(None) => Some(json!({ "categories": [] })),
            None => before.admin_scope.clone(),
        }
    };

    let loses_admin =
        (before.role == "super_admin" && role != "super_admin") || (before.is_active && !is_active);

    if id == actor && (role != before.role || !is_active) {
        return Err(ApiError::bad_request(
            "cannot_modify_self",
            "You cannot change your own role or deactivate your own account.",
        ));
    }

    if loses_admin && before.role == "super_admin" && other_super_admins(pool, id).await? == 0 {
        return Err(ApiError::conflict(
            "last_super_admin",
            "At least one active super admin must remain.",
        ));
    }

    let mut transaction = pool.begin().await?;

    sqlx::query(
        "update users
            set name = $2, department = $3, role = $4, is_active = $5, admin_scope = $6
          where id = $1",
    )
    .bind(id)
    .bind(&name)
    .bind(&department)
    .bind(&role)
    .bind(is_active)
    .bind(&admin_scope)
    .execute(&mut *transaction)
    .await?;

    if before.is_active && !is_active {
        sqlx::query(
            "update sessions set revoked_at = now()
              where user_id = $1 and revoked_at is null",
        )
        .bind(id)
        .execute(&mut *transaction)
        .await?;
    }

    let after = load(&mut *transaction, id)
        .await?
        .expect("the row still exists");

    audit::record_tx(
        &mut transaction,
        Some(actor),
        "user.update",
        "user",
        Some(id),
        Some(serde_json::to_value(&before).unwrap_or(serde_json::Value::Null)),
        Some(serde_json::to_value(&after).unwrap_or(serde_json::Value::Null)),
        ip,
    )
    .await;
    revision::bump(&mut transaction).await?;

    transaction.commit().await?;

    Ok(after)
}

/// Soft deactivate: the account and its history stay, but a deactivated user gets no apps.
pub async fn deactivate(
    pool: &PgPool,
    actor: Uuid,
    id: Uuid,
    ip: Option<&str>,
) -> Result<(), ApiError> {
    if id == actor {
        return Err(ApiError::bad_request(
            "cannot_modify_self",
            "You cannot deactivate your own account.",
        ));
    }

    let before = load(pool, id)
        .await?
        .ok_or_else(|| ApiError::not_found("user_not_found", "That user no longer exists."))?;

    if before.role == "super_admin" && other_super_admins(pool, id).await? == 0 {
        return Err(ApiError::conflict(
            "last_super_admin",
            "At least one active super admin must remain.",
        ));
    }

    let mut transaction = pool.begin().await?;

    sqlx::query("update users set is_active = false where id = $1")
        .bind(id)
        .execute(&mut *transaction)
        .await?;

    sqlx::query("update sessions set revoked_at = now() where user_id = $1 and revoked_at is null")
        .bind(id)
        .execute(&mut *transaction)
        .await?;

    let after = load(&mut *transaction, id)
        .await?
        .expect("the row still exists");

    audit::record_tx(
        &mut transaction,
        Some(actor),
        "user.deactivate",
        "user",
        Some(id),
        Some(serde_json::to_value(&before).unwrap_or(serde_json::Value::Null)),
        Some(serde_json::to_value(&after).unwrap_or(serde_json::Value::Null)),
        ip,
    )
    .await;
    revision::bump(&mut transaction).await?;

    transaction.commit().await?;

    Ok(())
}

pub async fn set_apps(
    pool: &PgPool,
    actor: Uuid,
    id: Uuid,
    app_ids: &[Uuid],
    ip: Option<&str>,
) -> Result<UserDetail, ApiError> {
    let mut unique: Vec<Uuid> = Vec::with_capacity(app_ids.len());
    for app_id in app_ids {
        if !unique.contains(app_id) {
            unique.push(*app_id);
        }
    }

    let mut transaction = pool.begin().await?;

    let user = load(&mut *transaction, id)
        .await?
        .ok_or_else(|| ApiError::not_found("user_not_found", "That user no longer exists."))?;

    if !unique.is_empty() {
        let known: i64 = sqlx::query_scalar(
            "select count(*) from apps where id = any($1) and deleted_at is null",
        )
        .bind(&unique)
        .fetch_one(&mut *transaction)
        .await?;
        if known != unique.len() as i64 {
            return Err(ApiError::bad_request(
                "unknown_app",
                "One of those apps no longer exists.",
            ));
        }
    }

    let previous: Vec<Uuid> = sqlx::query_scalar("select app_id from user_apps where user_id = $1")
        .bind(id)
        .fetch_all(&mut *transaction)
        .await?;

    sqlx::query("delete from user_apps where user_id = $1")
        .bind(id)
        .execute(&mut *transaction)
        .await?;

    if !unique.is_empty() {
        sqlx::query(
            "insert into user_apps (app_id, user_id)
             select unnest($1::uuid[]), $2
             on conflict do nothing",
        )
        .bind(&unique)
        .bind(id)
        .execute(&mut *transaction)
        .await?;
    }

    audit::record_tx(
        &mut transaction,
        Some(actor),
        "user.apps",
        "user",
        Some(id),
        Some(json!({ "appIds": previous })),
        Some(json!({ "appIds": unique })),
        ip,
    )
    .await;
    revision::bump(&mut transaction).await?;

    transaction.commit().await?;

    let group_ids: Vec<Uuid> =
        sqlx::query_scalar("select group_id from group_members where user_id = $1")
            .bind(id)
            .fetch_all(pool)
            .await?;

    Ok(UserDetail {
        user,
        group_ids,
        app_ids: unique,
    })
}

// ---------------------------------------------------------------- reset codes

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IssuedCode {
    pub code: String,
    pub expires_at: DateTime<Utc>,
}

pub async fn issue_reset_code(
    pool: &PgPool,
    actor: Uuid,
    id: Uuid,
    purpose: Option<&str>,
    ip: Option<&str>,
) -> Result<IssuedCode, ApiError> {
    let purpose = match purpose {
        Some("onboard") => "onboard",
        Some("reset") | None => "reset",
        Some(_) => {
            return Err(ApiError::bad_request(
                "invalid_purpose",
                "Purpose must be onboard or reset.",
            ))
        }
    };

    let user = load(pool, id)
        .await?
        .ok_or_else(|| ApiError::not_found("user_not_found", "That user no longer exists."))?;

    let mut transaction = pool.begin().await?;

    let (code, expires_at) =
        auth::issue_reset_code_tx(&mut *transaction, id, purpose, Some(actor), 72).await?;

    audit::record_tx(
        &mut transaction,
        Some(actor),
        "user.reset_code",
        "user",
        Some(id),
        None,
        Some(json!({ "email": user.email, "purpose": purpose })),
        ip,
    )
    .await;

    transaction.commit().await?;

    Ok(IssuedCode { code, expires_at })
}

// ---------------------------------------------------------------- CSV import

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CsvRow {
    pub line: usize,
    pub name: String,
    pub email: String,
    pub department: Option<String>,
    pub groups: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportError {
    pub line: usize,
    pub email: Option<String>,
    pub message: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OnboardingCode {
    pub email: String,
    pub code: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub created: i64,
    pub updated: i64,
    pub unchanged: i64,
    pub errors: Vec<ImportError>,
    pub onboarding: Vec<OnboardingCode>,
}

/// Minimal RFC-4180 reader: quoted fields may contain commas and doubled quotes.
fn split_csv_line(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut characters = line.chars().peekable();

    while let Some(character) = characters.next() {
        match character {
            '"' if quoted && characters.peek() == Some(&'"') => {
                current.push('"');
                characters.next();
            }
            '"' => quoted = !quoted,
            ',' if !quoted => {
                fields.push(current.trim().to_string());
                current.clear();
            }
            other => current.push(other),
        }
    }

    fields.push(current.trim().to_string());
    fields
}

pub fn parse_csv(text: &str) -> Result<Vec<CsvRow>, ApiError> {
    let mut lines = text
        .lines()
        .enumerate()
        .map(|(index, line)| (index + 1, line.trim_end_matches('\r')))
        .filter(|(_, line)| !line.trim().is_empty());

    let Some((_, header)) = lines.next() else {
        return Err(ApiError::bad_request("invalid_csv", "The file is empty."));
    };

    let columns: Vec<String> = split_csv_line(header)
        .iter()
        .map(|column| column.trim().to_lowercase().replace(' ', "_"))
        .collect();

    let index_of = |name: &str| columns.iter().position(|column| column == name);

    let (Some(email_index), Some(name_index)) = (index_of("email"), index_of("name")) else {
        return Err(ApiError::bad_request(
            "invalid_csv",
            "The header must include name and email columns.",
        ));
    };
    let department_index = index_of("department");
    let groups_index = index_of("groups");

    let mut rows = Vec::new();

    for (line, raw) in lines {
        let fields = split_csv_line(raw);
        let field = |index: Option<usize>| {
            index
                .and_then(|index| fields.get(index))
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        };

        let email = fields
            .get(email_index)
            .map(|value| value.trim().to_string())
            .unwrap_or_default();
        let name = fields
            .get(name_index)
            .map(|value| value.trim().to_string())
            .unwrap_or_default();

        let groups = field(groups_index)
            .map(|value| {
                value
                    .split(';')
                    .map(|group| group.trim().to_string())
                    .filter(|group| !group.is_empty())
                    .collect()
            })
            .unwrap_or_default();

        rows.push(CsvRow {
            line,
            name,
            email,
            department: field(department_index),
            groups,
        });
    }

    Ok(rows)
}

pub async fn import(
    pool: &PgPool,
    actor: Uuid,
    text: &str,
    ip: Option<&str>,
) -> Result<ImportReport, ApiError> {
    let rows = parse_csv(text)?;

    let mut transaction = pool.begin().await?;

    // Every group the admin portal offers in its picker is valid here too: a plain group, the
    // built-in All Users group, and an auto-by-department group. Names are matched
    // case-insensitively, so "all users" and "All Users" both work.
    let group_rows: Vec<(Uuid, String)> = sqlx::query_as("select id, name from groups")
        .fetch_all(&mut *transaction)
        .await?;
    let group_by_name: HashMap<String, Uuid> = group_rows
        .into_iter()
        .map(|(id, name)| (name.to_lowercase(), id))
        .collect();

    let mut report = ImportReport {
        created: 0,
        updated: 0,
        unchanged: 0,
        errors: Vec::new(),
        onboarding: Vec::new(),
    };
    let mut seen: HashMap<String, usize> = HashMap::new();

    for row in rows {
        if report.errors.len() >= 200 {
            report.errors.push(ImportError {
                line: row.line,
                email: None,
                message: "Too many errors; later rows were not processed.".to_string(),
            });
            break;
        }

        let email = match validate_email(&row.email) {
            Ok(email) => email,
            Err(_) => {
                report.errors.push(ImportError {
                    line: row.line,
                    email: None,
                    message: "Email is missing or not valid.".to_string(),
                });
                continue;
            }
        };

        if let Some(first_line) = seen.insert(email.clone(), row.line) {
            report.errors.push(ImportError {
                line: row.line,
                email: Some(email),
                message: format!("Duplicate email in this file (first seen on line {first_line})."),
            });
            continue;
        }

        let name = match validate_name(&row.name) {
            Ok(name) => name,
            Err(_) => {
                report.errors.push(ImportError {
                    line: row.line,
                    email: Some(email),
                    message: "Name is missing or too long.".to_string(),
                });
                continue;
            }
        };

        let department = match validate_department(row.department.clone()) {
            Ok(department) => department,
            Err(_) => {
                report.errors.push(ImportError {
                    line: row.line,
                    email: Some(email),
                    message: "Department must be 120 characters or fewer.".to_string(),
                });
                continue;
            }
        };

        let mut group_ids = Vec::new();
        let mut group_error = None;
        for group_name in &row.groups {
            match group_by_name.get(&group_name.to_lowercase()) {
                Some(id) => group_ids.push(*id),
                None => {
                    group_error = Some(format!(
                        "Unknown group \"{group_name}\"; create it first or fix the spelling."
                    ));
                    break;
                }
            }
        }

        if let Some(message) = group_error {
            report.errors.push(ImportError {
                line: row.line,
                email: Some(email),
                message,
            });
            continue;
        }

        let existing: Option<Uuid> =
            sqlx::query_scalar("select id from users where lower(email) = $1")
                .bind(&email)
                .fetch_optional(&mut *transaction)
                .await?;

        match existing {
            Some(id) => {
                let changed: bool = sqlx::query_scalar(
                    "update users
                        set name = $2, department = $3
                      where id = $1
                        and (name <> $2 or department is distinct from $3)
                     returning true",
                )
                .bind(id)
                .bind(&name)
                .bind(&department)
                .fetch_optional(&mut *transaction)
                .await?
                .unwrap_or(false);

                if changed {
                    report.updated += 1;
                } else {
                    report.unchanged += 1;
                }

                // CSV import never changes roles or passwords: a spreadsheet mistake
                // should not be able to grant admin rights or lock someone out.
                if !group_ids.is_empty() {
                    sqlx::query(
                        "insert into group_members (group_id, user_id)
                         select unnest($1::uuid[]), $2
                         on conflict do nothing",
                    )
                    .bind(&group_ids)
                    .bind(id)
                    .execute(&mut *transaction)
                    .await?;
                }
            }
            None => {
                let id: Uuid = sqlx::query_scalar(
                    "insert into users (email, name, department, must_change_password)
                     values ($1, $2, $3, true)
                     returning id",
                )
                .bind(&email)
                .bind(&name)
                .bind(&department)
                .fetch_one(&mut *transaction)
                .await?;

                if !group_ids.is_empty() {
                    sqlx::query(
                        "insert into group_members (group_id, user_id)
                         select unnest($1::uuid[]), $2
                         on conflict do nothing",
                    )
                    .bind(&group_ids)
                    .bind(id)
                    .execute(&mut *transaction)
                    .await?;
                }

                let (code, expires_at) =
                    auth::issue_reset_code_tx(&mut *transaction, id, "onboard", Some(actor), 72)
                        .await?;

                report.created += 1;
                report.onboarding.push(OnboardingCode {
                    email,
                    code,
                    expires_at,
                });
            }
        }
    }

    audit::record_tx(
        &mut transaction,
        Some(actor),
        "user.import",
        "user",
        None,
        None,
        Some(json!({
            "created": report.created,
            "updated": report.updated,
            "unchanged": report.unchanged,
            "errors": report.errors.len(),
        })),
        ip,
    )
    .await;

    if report.created > 0 || report.updated > 0 {
        revision::bump(&mut transaction).await?;
    }

    transaction.commit().await?;

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_handles_quotes_and_optional_columns() {
        let rows = parse_csv(
            "name,email,department,groups\n\
             \"Sharma, Rahul\",rahul@example.com,Finance,Finance;Reporting\n\
             Simple,s@example.com,,\n",
        )
        .unwrap();

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].name, "Sharma, Rahul");
        assert_eq!(rows[0].department.as_deref(), Some("Finance"));
        assert_eq!(rows[0].groups, vec!["Finance", "Reporting"]);
        assert_eq!(rows[1].department, None);
        assert!(rows[1].groups.is_empty());
    }

    #[test]
    fn csv_requires_name_and_email() {
        assert!(parse_csv("department,groups\nFinance,Ops\n").is_err());
        assert!(parse_csv("").is_err());
    }

    #[test]
    fn emails_are_lower_cased_and_checked() {
        assert_eq!(validate_email(" Rahul@Example.COM ").unwrap(), "rahul@example.com");
        assert!(validate_email("not-an-email").is_err());
        assert!(validate_email("two words@example.com").is_err());
    }

    #[test]
    fn scopes_are_normalised_and_only_kept_for_app_admins() {
        let scope = validate_scope(json!({
            "categories": [" Finance ", "finance", "HR"],
            "group_ids": "ignored"
        }))
        .unwrap();
        assert_eq!(scope, json!({ "categories": ["Finance", "HR"] }));

        assert!(validate_scope(json!({ "categories": [""] })).is_err());
        assert!(validate_scope(json!({ "categories": "Finance" })).is_err());

        assert!(scope_for("user", Some(scope.clone())).unwrap().is_none());
        assert!(scope_for("super_admin", None).unwrap().is_none());
        assert_eq!(
            scope_for("app_admin", None).unwrap(),
            Some(json!({ "categories": [] }))
        );
    }
}
