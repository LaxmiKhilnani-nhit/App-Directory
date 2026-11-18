use axum::{
    extract::{rejection::JsonRejection, Path, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use serde::Deserialize;
use uuid::Uuid;

use crate::domain::models::{AdminGroup, GroupDetail};
use crate::domain::validate;
use crate::error::{ApiError, ApiResult};
use crate::http::identity::TrustedUser;
use crate::http::json_body;
use crate::state::AppState;
use crate::store::{GroupMemberRow, GroupRow};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewGroup {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub auto_department: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupPatch {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "crate::domain::de::double_option")]
    pub description: Option<Option<String>>,
    #[serde(default, deserialize_with = "crate::domain::de::double_option")]
    pub auto_department: Option<Option<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MembersRequest {
    #[serde(default)]
    pub user_ids: Vec<Uuid>,
}

pub async fn list(
    State(state): State<AppState>,
    TrustedUser(_actor): TrustedUser,
) -> ApiResult<Json<Vec<AdminGroup>>> {
    let data = state.store.read();
    let mut groups: Vec<&GroupRow> = data.groups.iter().collect();
    groups.sort_by(|left, right| {
        right
            .is_all_users
            .cmp(&left.is_all_users)
            .then_with(|| left.name.cmp(&right.name))
    });
    Ok(Json(
        groups
            .into_iter()
            .map(|group| AdminGroup::from_row(group, &data))
            .collect(),
    ))
}

pub async fn get(
    State(state): State<AppState>,
    TrustedUser(_actor): TrustedUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<GroupDetail>> {
    let data = state.store.read();
    let group = data
        .group_by_id(id)
        .ok_or_else(|| ApiError::not_found("group_not_found", "That group no longer exists."))?;
    Ok(Json(GroupDetail {
        group: AdminGroup::from_row(group, &data),
        user_ids: data
            .group_members
            .iter()
            .filter(|member| member.group_id == id)
            .map(|member| member.user_id)
            .collect(),
    }))
}

pub async fn create(
    State(state): State<AppState>,
    TrustedUser(actor): TrustedUser,
    body: Result<Json<NewGroup>, JsonRejection>,
) -> ApiResult<(StatusCode, Json<AdminGroup>)> {
    let input = json_body(body)?;
    let name = validate::validate_name(&input.name)?;
    let description = validate::clean(input.description);
    let auto_department = validate::validate_department(input.auto_department)?;

    if name.eq_ignore_ascii_case(crate::store::ALL_USERS_GROUP) {
        return Err(ApiError::conflict(
            "group_name_taken",
            "A group with that name already exists.",
        ));
    }

    let now = Utc::now();
    let id = Uuid::new_v4();
    let row = GroupRow {
        id,
        name,
        description,
        auto_department,
        is_all_users: false,
        created_at: now,
        updated_at: now,
    };

    let view = state.store.mutate(|data| {
        if data
            .groups
            .iter()
            .any(|group| group.name.eq_ignore_ascii_case(&row.name))
        {
            return Err(ApiError::conflict(
                "group_name_taken",
                "A group with that name already exists.",
            ));
        }
        data.groups.push(row);
        data.bump_revision();
        let view = AdminGroup::from_row(data.group_by_id(id).expect("exists"), data);
        data.audit(
            Some(actor.id),
            Some(actor.email.clone()),
            "group.create",
            "group",
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
    body: Result<Json<GroupPatch>, JsonRejection>,
) -> ApiResult<Json<AdminGroup>> {
    let patch = json_body(body)?;

    state.store.mutate(|data| {
        let before = data
            .group_by_id(id)
            .ok_or_else(|| ApiError::not_found("group_not_found", "That group no longer exists."))?
            .clone();

        if before.is_all_users && (patch.name.is_some() || patch.auto_department.is_some()) {
            return Err(ApiError::conflict(
                "builtin_group",
                "The All Users group can only have its description changed.",
            ));
        }

        let name = match patch.name {
            Some(value) => validate::validate_name(&value)?,
            None => before.name.clone(),
        };
        if data.groups.iter().any(|group| {
            group.id != id && group.name.eq_ignore_ascii_case(&name)
        }) {
            return Err(ApiError::conflict(
                "group_name_taken",
                "A group with that name already exists.",
            ));
        }
        let description = match patch.description {
            Some(value) => validate::clean(value),
            None => before.description.clone(),
        };
        let auto_department = if before.is_all_users {
            None
        } else {
            match patch.auto_department {
                Some(value) => validate::validate_department(value)?,
                None => before.auto_department.clone(),
            }
        };

        let before_view = AdminGroup::from_row(&before, data);
        {
            let group = data.groups.iter_mut().find(|group| group.id == id).expect("exists");
            group.name = name;
            group.description = description;
            group.auto_department = auto_department.clone();
            group.updated_at = Utc::now();
        }
        if auto_department.is_some() {
            data.group_members.retain(|member| member.group_id != id);
        }

        data.bump_revision();
        let view = AdminGroup::from_row(data.group_by_id(id).expect("exists"), data);
        data.audit(
            Some(actor.id),
            Some(actor.email.clone()),
            "group.update",
            "group",
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
            .group_by_id(id)
            .ok_or_else(|| ApiError::not_found("group_not_found", "That group no longer exists."))?
            .clone();
        if before.is_all_users {
            return Err(ApiError::conflict(
                "builtin_group",
                "The All Users group cannot be deleted.",
            ));
        }
        data.groups.retain(|group| group.id != id);
        data.group_members.retain(|member| member.group_id != id);
        data.app_groups.retain(|grant| grant.group_id != id);
        data.bump_revision();
        data.audit(
            Some(actor.id),
            Some(actor.email.clone()),
            "group.delete",
            "group",
            Some(id),
            Some(serde_json::to_value(AdminGroup::from_row(&before, data)).unwrap_or_default()),
            None,
        );
        Ok(())
    })?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn set_members(
    State(state): State<AppState>,
    TrustedUser(actor): TrustedUser,
    Path(id): Path<Uuid>,
    body: Result<Json<MembersRequest>, JsonRejection>,
) -> ApiResult<Json<GroupDetail>> {
    let request = json_body(body)?;
    let mut user_ids = Vec::with_capacity(request.user_ids.len());
    for user_id in request.user_ids {
        if !user_ids.contains(&user_id) {
            user_ids.push(user_id);
        }
    }

    state.store.mutate(|data| {
        let group = data
            .group_by_id(id)
            .ok_or_else(|| ApiError::not_found("group_not_found", "That group no longer exists."))?
            .clone();
        if group.is_all_users || group.auto_department.is_some() {
            return Err(ApiError::conflict(
                "derived_group",
                "Membership of this group is derived automatically.",
            ));
        }
        if !user_ids.iter().all(|user_id| data.user_by_id(*user_id).is_some()) {
            return Err(ApiError::bad_request(
                "unknown_user",
                "One of those users no longer exists.",
            ));
        }

        let before: Vec<Uuid> = data
            .group_members
            .iter()
            .filter(|member| member.group_id == id)
            .map(|member| member.user_id)
            .collect();
        data.group_members.retain(|member| member.group_id != id);
        let now = Utc::now();
        data.group_members.extend(user_ids.iter().map(|user_id| GroupMemberRow {
            group_id: id,
            user_id: *user_id,
            created_at: now,
        }));

        data.bump_revision();
        let view = GroupDetail {
            group: AdminGroup::from_row(data.group_by_id(id).expect("exists"), data),
            user_ids: user_ids.clone(),
        };
        data.audit(
            Some(actor.id),
            Some(actor.email.clone()),
            "group.members",
            "group",
            Some(id),
            Some(serde_json::json!({ "userIds": before })),
            Some(serde_json::json!({ "userIds": user_ids })),
        );
        Ok(Json(view))
    })
}
