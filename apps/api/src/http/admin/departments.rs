//! Department management. A department is a first-class entity: users belong to one by its
//! name, and apps can be granted to it directly.

use axum::{
    extract::{rejection::JsonRejection, Path, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use serde::Deserialize;
use uuid::Uuid;

use crate::domain::models::Department;
use crate::domain::validate;
use crate::error::{ApiError, ApiResult};
use crate::http::identity::TrustedUser;
use crate::http::json_body;
use crate::state::AppState;
use crate::store::DepartmentRow;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NewDepartment {
    pub name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DepartmentPatch {
    pub name: String,
}

pub async fn list(
    State(state): State<AppState>,
    TrustedUser(_actor): TrustedUser,
) -> ApiResult<Json<Vec<Department>>> {
    let data = state.store.read();
    let mut rows: Vec<&DepartmentRow> = data.departments.iter().collect();
    rows.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(Json(
        rows.into_iter()
            .map(|row| Department::from_row(row, &data))
            .collect(),
    ))
}

pub async fn create(
    State(state): State<AppState>,
    TrustedUser(actor): TrustedUser,
    body: Result<Json<NewDepartment>, JsonRejection>,
) -> ApiResult<(StatusCode, Json<Department>)> {
    let input = json_body(body)?;
    let name = validate::validate_name(&input.name)?;
    let now = Utc::now();
    let row = DepartmentRow {
        id: Uuid::new_v4(),
        name: name.clone(),
        created_at: now,
        updated_at: now,
    };

    let view = state.store.mutate(|data| {
        if data
            .departments
            .iter()
            .any(|department| department.name.eq_ignore_ascii_case(&name))
        {
            return Err(ApiError::conflict(
                "department_name_taken",
                "A department with that name already exists.",
            ));
        }
        let id = row.id;
        data.departments.push(row);
        data.bump_revision();
        let view = Department::from_row(data.department_by_id(id).expect("exists"), data);
        data.audit(
            Some(actor.id),
            Some(actor.email.clone()),
            "department.create",
            "department",
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
    body: Result<Json<DepartmentPatch>, JsonRejection>,
) -> ApiResult<Json<Department>> {
    let input = json_body(body)?;
    let name = validate::validate_name(&input.name)?;

    state.store.mutate(|data| {
        let before = data
            .department_by_id(id)
            .ok_or_else(|| ApiError::not_found("department_not_found", "No such department."))?
            .clone();

        if data
            .departments
            .iter()
            .any(|department| department.id != id && department.name.eq_ignore_ascii_case(&name))
        {
            return Err(ApiError::conflict(
                "department_name_taken",
                "A department with that name already exists.",
            ));
        }

        // Keep people attached when a department is renamed: membership is stored by name.
        for user in &mut data.users {
            if user.department.as_deref() == Some(before.name.as_str()) {
                user.department = Some(name.clone());
            }
        }
        if let Some(department) = data.departments.iter_mut().find(|row| row.id == id) {
            department.name = name.clone();
            department.updated_at = Utc::now();
        }

        data.bump_revision();
        let view = Department::from_row(data.department_by_id(id).expect("exists"), data);
        data.audit(
            Some(actor.id),
            Some(actor.email.clone()),
            "department.update",
            "department",
            Some(id),
            Some(serde_json::json!({ "name": before.name })),
            Some(serde_json::json!({ "name": name })),
        );
        Ok(Json(view))
    })
}

/// App ids currently granted to a department, for the edit form.
pub async fn get_apps(
    State(state): State<AppState>,
    TrustedUser(_actor): TrustedUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Vec<Uuid>>> {
    let data = state.store.read();
    if data.department_by_id(id).is_none() {
        return Err(ApiError::not_found(
            "department_not_found",
            "No such department.",
        ));
    }
    Ok(Json(
        data.app_departments
            .iter()
            .filter(|grant| grant.department_id == id)
            .map(|grant| grant.app_id)
            .collect(),
    ))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DepartmentApps {
    #[serde(default)]
    pub app_ids: Vec<Uuid>,
}

/// Replaces the apps granted to one department. This is the department-centric counterpart
/// to `/admin/apps/{id}/departments`.
pub async fn set_apps(
    State(state): State<AppState>,
    TrustedUser(actor): TrustedUser,
    Path(id): Path<Uuid>,
    body: Result<Json<DepartmentApps>, JsonRejection>,
) -> ApiResult<Json<Department>> {
    let request = json_body(body)?;
    let mut app_ids = request.app_ids;
    app_ids.dedup();

    state.store.mutate(|data| {
        let department = data
            .department_by_id(id)
            .ok_or_else(|| ApiError::not_found("department_not_found", "No such department."))?
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

        let before: Vec<Uuid> = data
            .app_departments
            .iter()
            .filter(|grant| grant.department_id == id)
            .map(|grant| grant.app_id)
            .collect();

        data.app_departments
            .retain(|grant| grant.department_id != id);
        data.app_departments
            .extend(app_ids.iter().map(|app_id| crate::store::AppDepartmentRow {
                app_id: *app_id,
                department_id: id,
            }));

        data.bump_revision();
        let view = Department::from_row(data.department_by_id(id).expect("exists"), data);
        data.audit(
            Some(actor.id),
            Some(actor.email.clone()),
            "department.apps",
            "department",
            Some(id),
            Some(serde_json::json!({ "name": department.name, "appIds": before })),
            Some(serde_json::json!({ "name": department.name, "appIds": app_ids })),
        );
        Ok(Json(view))
    })
}

pub async fn delete(
    State(state): State<AppState>,
    TrustedUser(actor): TrustedUser,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    let name = state.store.mutate(|data| {
        let before = data
            .department_by_id(id)
            .ok_or_else(|| ApiError::not_found("department_not_found", "No such department."))?
            .clone();

        data.departments.retain(|department| department.id != id);
        data.app_departments
            .retain(|grant| grant.department_id != id);
        for user in &mut data.users {
            if user.department.as_deref() == Some(before.name.as_str()) {
                user.department = None;
            }
        }

        data.bump_revision();
        data.audit(
            Some(actor.id),
            Some(actor.email.clone()),
            "department.delete",
            "department",
            Some(id),
            Some(serde_json::json!({ "id": before.id, "name": before.name })),
            None,
        );
        Ok(before.name)
    })?;

    tracing::info!(%name, "department deleted");
    Ok(StatusCode::NO_CONTENT)
}
