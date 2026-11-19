use std::collections::HashMap;

use axum::{
    extract::{rejection::JsonRejection, Path, Query, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use serde::Deserialize;
use uuid::Uuid;

use crate::domain::models::{AdminUser, ImportErrorRow, ImportReport, UserDetail, UserPage};
use crate::domain::validate;
use crate::error::{ApiError, ApiResult};
use crate::http::identity::TrustedUser;
use crate::http::json_body;
use crate::state::AppState;
use crate::store::{UserAppRow, UserRow};

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserQuery {
    pub query: Option<String>,
    pub role: Option<String>,
    pub active: Option<bool>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
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
    #[serde(default)]
    pub admin_scope: Option<serde_json::Value>,
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
    #[serde(default, deserialize_with = "crate::domain::de::double_option")]
    pub admin_scope: Option<Option<serde_json::Value>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppGrants {
    #[serde(default)]
    pub app_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct ImportRequest {
    pub csv: String,
}

pub async fn list(
    State(state): State<AppState>,
    TrustedUser(_actor): TrustedUser,
    Query(params): Query<UserQuery>,
) -> ApiResult<Json<UserPage>> {
    if let Some(role) = params.role.as_deref() {
        if !validate::is_role(role) {
            return Err(ApiError::bad_request(
                "invalid_role",
                "Role must be user, app_admin or super_admin.",
            ));
        }
    }

    let needle = params
        .query
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_lowercase);

    let data = state.store.read();
    let mut users: Vec<&UserRow> = data
        .users
        .iter()
        .filter(|user| match &needle {
            Some(text) => {
                user.email.to_lowercase().contains(text) || user.name.to_lowercase().contains(text)
            }
            None => true,
        })
        .filter(|user| params.role.as_deref().is_none_or(|role| user.role == role))
        .filter(|user| params.active.is_none_or(|active| user.is_active == active))
        .collect();
    users.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.email.cmp(&right.email))
    });

    let total = users.len() as i64;
    let offset = params.offset.unwrap_or(0).max(0) as usize;
    let limit = params.limit.unwrap_or(100).clamp(1, 500) as usize;

    Ok(Json(UserPage {
        users: users
            .into_iter()
            .skip(offset)
            .take(limit)
            .map(AdminUser::from)
            .collect(),
        total,
    }))
}

pub async fn get(
    State(state): State<AppState>,
    TrustedUser(_actor): TrustedUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<UserDetail>> {
    let data = state.store.read();
    let user = data
        .user_by_id(id)
        .ok_or_else(|| ApiError::not_found("user_not_found", "That user no longer exists."))?;
    Ok(Json(UserDetail {
        user: AdminUser::from(user),
        group_ids: data.group_ids_for_user(id),
        app_ids: data.app_ids_for_user(id),
    }))
}

pub async fn create(
    State(state): State<AppState>,
    TrustedUser(actor): TrustedUser,
    body: Result<Json<NewUser>, JsonRejection>,
) -> ApiResult<(StatusCode, Json<AdminUser>)> {
    let input = json_body(body)?;
    let email = validate::validate_email(&input.email)?;
    let name = validate::validate_name(&input.name)?;
    let department = validate::validate_department(input.department)?;
    let role = validate::validate_role(&input.role)?;
    let admin_scope = validate::scope_for(&role, input.admin_scope)?;

    let now = Utc::now();
    let id = Uuid::new_v4();
    let row = UserRow {
        id,
        email,
        name,
        department,
        role,
        admin_scope,
        is_active: true,
        must_change_password: false,
        last_login_at: None,
        created_at: now,
        updated_at: now,
    };

    let view = state.store.mutate(|data| {
        if data.user_by_email(&row.email).is_some() {
            return Err(ApiError::conflict(
                "email_taken",
                "An account with that email already exists.",
            ));
        }
        data.ensure_department(row.department.as_deref().unwrap_or_default());
        data.users.push(row);
        data.bump_revision();
        let view = AdminUser::from(data.user_by_id(id).expect("exists"));
        data.audit(
            Some(actor.id),
            Some(actor.email.clone()),
            "user.create",
            "user",
            Some(id),
            None,
            Some(serde_json::to_value(&view).unwrap_or_default()),
        );
        Ok(view)
    })?;

    Ok((StatusCode::CREATED, Json(view)))
}

pub async fn update(
    State(state): State<AppState>,
    TrustedUser(actor): TrustedUser,
    Path(id): Path<Uuid>,
    body: Result<Json<UserPatch>, JsonRejection>,
) -> ApiResult<Json<AdminUser>> {
    let patch = json_body(body)?;

    state.store.mutate(|data| {
        let before = data
            .user_by_id(id)
            .ok_or_else(|| ApiError::not_found("user_not_found", "That user no longer exists."))?
            .clone();

        let name = match patch.name {
            Some(value) => validate::validate_name(&value)?,
            None => before.name.clone(),
        };
        let department = match patch.department {
            Some(value) => validate::validate_department(value)?,
            None => before.department.clone(),
        };
        let role = match patch.role {
            Some(value) => validate::validate_role(&value)?,
            None => before.role.clone(),
        };
        let is_active = patch.is_active.unwrap_or(before.is_active);
        let admin_scope = if role != "app_admin" {
            None
        } else {
            match patch.admin_scope {
                Some(Some(value)) => Some(validate::validate_scope(value)?),
                Some(None) => Some(serde_json::json!({ "categories": [] })),
                None => before.admin_scope.clone(),
            }
        };

        let before_view = AdminUser::from(&before);
        data.ensure_department(department.as_deref().unwrap_or_default());
        {
            let user = data
                .users
                .iter_mut()
                .find(|user| user.id == id)
                .expect("exists");
            user.name = name;
            user.department = department;
            user.role = role;
            user.is_active = is_active;
            user.admin_scope = admin_scope;
            user.updated_at = Utc::now();
        }

        data.bump_revision();
        let view = AdminUser::from(data.user_by_id(id).expect("exists"));
        data.audit(
            Some(actor.id),
            Some(actor.email.clone()),
            "user.update",
            "user",
            Some(id),
            Some(serde_json::to_value(&before_view).unwrap_or_default()),
            Some(serde_json::to_value(&view).unwrap_or_default()),
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
        let before = data
            .user_by_id(id)
            .ok_or_else(|| ApiError::not_found("user_not_found", "That user no longer exists."))?
            .clone();
        let before_view = AdminUser::from(&before);
        let user = data
            .users
            .iter_mut()
            .find(|user| user.id == id)
            .expect("exists");
        user.is_active = false;
        user.updated_at = Utc::now();
        let after_view = AdminUser::from(data.user_by_id(id).expect("exists"));

        data.bump_revision();
        data.audit(
            Some(actor.id),
            Some(actor.email.clone()),
            "user.deactivate",
            "user",
            Some(id),
            Some(serde_json::to_value(&before_view).unwrap_or_default()),
            Some(serde_json::to_value(&after_view).unwrap_or_default()),
        );
        Ok(())
    })?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn set_apps(
    State(state): State<AppState>,
    TrustedUser(actor): TrustedUser,
    Path(id): Path<Uuid>,
    body: Result<Json<AppGrants>, JsonRejection>,
) -> ApiResult<Json<UserDetail>> {
    let request = json_body(body)?;
    let mut app_ids = Vec::with_capacity(request.app_ids.len());
    for app_id in request.app_ids {
        if !app_ids.contains(&app_id) {
            app_ids.push(app_id);
        }
    }

    state.store.mutate(|data| {
        let user = data
            .user_by_id(id)
            .ok_or_else(|| ApiError::not_found("user_not_found", "That user no longer exists."))?
            .clone();
        if !app_ids
            .iter()
            .all(|app_id| data.live_app_by_id(*app_id).is_some())
        {
            return Err(ApiError::bad_request(
                "unknown_app",
                "One of those apps no longer exists.",
            ));
        }

        let before: Vec<Uuid> = data.app_ids_for_user(id);
        data.user_apps.retain(|grant| grant.user_id != id);
        let now = Utc::now();
        data.user_apps
            .extend(app_ids.iter().map(|app_id| UserAppRow {
                user_id: id,
                app_id: *app_id,
                created_at: now,
            }));

        data.bump_revision();
        let view = UserDetail {
            user: AdminUser::from(&user),
            group_ids: data.group_ids_for_user(id),
            app_ids: app_ids.clone(),
        };
        data.audit(
            Some(actor.id),
            Some(actor.email.clone()),
            "user.apps",
            "user",
            Some(id),
            Some(serde_json::json!({ "appIds": before })),
            Some(serde_json::json!({ "appIds": app_ids })),
        );
        Ok(Json(view))
    })
}

/// Codes are intentionally not generated any more. The route stays so the admin portal's
/// "issue code" button does not produce a network error.
struct CsvRow {
    line: usize,
    name: String,
    email: String,
    department: Option<String>,
}

fn parse_csv(text: &str) -> Result<Vec<CsvRow>, ApiError> {
    // Excel writes a UTF-8 BOM, which otherwise becomes part of the first header column.
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut lines = text
        .lines()
        .enumerate()
        .map(|(index, line)| (index + 1, line))
        .filter(|(_, line)| !line.trim().is_empty());

    let Some((_, header)) = lines.next() else {
        return Err(ApiError::bad_request("invalid_csv", "The file is empty."));
    };

    // Excel with Indian regional settings uses ';' as the separator.
    let delimiter = if header.matches(';').count() > header.matches(',').count() {
        ';'
    } else {
        ','
    };

    let columns: Vec<String> = validate::split_csv_line_delimited(header, delimiter)
        .iter()
        .map(|column| column.trim().to_lowercase().replace(' ', "_"))
        .collect();
    let index_of_any = |names: &[&str]| {
        columns
            .iter()
            .position(|column| names.contains(&column.as_str()))
    };
    let email_index = index_of_any(&["email", "email_address", "work_email", "e_mail"]);
    let name_index = index_of_any(&["name", "full_name", "employee_name", "person_name"]);
    let (Some(email_index), Some(name_index)) = (email_index, name_index) else {
        return Err(ApiError::bad_request(
            "invalid_csv",
            format!(
                "The header must include name and email columns. Found: {}",
                columns.join(", ")
            ),
        ));
    };
    let department_index = index_of_any(&["department", "dept"]);

    let mut rows = Vec::new();
    for (line, raw) in lines {
        let fields = validate::split_csv_line_delimited(raw, delimiter);
        let field = |index: Option<usize>| {
            index
                .and_then(|index| fields.get(index))
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        };
        rows.push(CsvRow {
            line,
            email: fields.get(email_index).cloned().unwrap_or_default(),
            name: fields.get(name_index).cloned().unwrap_or_default(),
            department: field(department_index),
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
    let rows = parse_csv(&request.csv)?;

    state.store.mutate(|data| {
        let mut report = ImportReport {
            created: 0,
            updated: 0,
            unchanged: 0,
            errors: Vec::new(),
        };
        let mut seen: HashMap<String, usize> = HashMap::new();

        for row in rows {
            if report.errors.len() >= 200 {
                report.errors.push(ImportErrorRow {
                    line: row.line,
                    email: None,
                    message: "Too many errors; later rows were not processed.".to_string(),
                });
                break;
            }

            let email = match validate::validate_email(&row.email) {
                Ok(email) => email,
                Err(_) => {
                    report.errors.push(ImportErrorRow {
                        line: row.line,
                        email: None,
                        message: "Email is missing or not valid.".to_string(),
                    });
                    continue;
                }
            };
            if let Some(first_line) = seen.insert(email.clone(), row.line) {
                report.errors.push(ImportErrorRow {
                    line: row.line,
                    email: Some(email),
                    message: format!(
                        "Duplicate email in this file (first seen on line {first_line})."
                    ),
                });
                continue;
            }
            let name = match validate::validate_name(&row.name) {
                Ok(name) => name,
                Err(_) => {
                    report.errors.push(ImportErrorRow {
                        line: row.line,
                        email: Some(email),
                        message: "Name is missing or too long.".to_string(),
                    });
                    continue;
                }
            };
            let department = match validate::validate_department(row.department.clone()) {
                Ok(department) => department,
                Err(_) => {
                    report.errors.push(ImportErrorRow {
                        line: row.line,
                        email: Some(email),
                        message: "Department must be 120 characters or fewer.".to_string(),
                    });
                    continue;
                }
            };

            match data.user_by_email(&email).cloned() {
                Some(existing) => {
                    if existing.name != name || existing.department != department {
                        data.ensure_department(department.as_deref().unwrap_or_default());
                        let user = data
                            .users
                            .iter_mut()
                            .find(|user| user.id == existing.id)
                            .expect("exists");
                        user.name = name;
                        user.department = department;
                        user.updated_at = Utc::now();
                        report.updated += 1;
                    } else {
                        report.unchanged += 1;
                    }
                }
                None => {
                    let now = Utc::now();
                    let id = Uuid::new_v4();
                    data.ensure_department(department.as_deref().unwrap_or_default());
                    data.users.push(UserRow {
                        id,
                        email,
                        name,
                        department,
                        role: "user".to_string(),
                        admin_scope: None,
                        is_active: true,
                        must_change_password: false,
                        last_login_at: None,
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
            "user.import",
            "user",
            None,
            None,
            Some(serde_json::json!({
                "created": report.created,
                "updated": report.updated,
                "unchanged": report.unchanged,
                "errors": report.errors.len(),
            })),
        );
        Ok(Json(report))
    })
}
