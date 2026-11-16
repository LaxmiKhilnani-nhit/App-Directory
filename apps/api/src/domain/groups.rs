use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{FromRow, PgPool, Postgres};
use uuid::Uuid;

use crate::domain::{audit, is_unique_violation, revision};
use crate::error::ApiError;

/// Group plus the numbers the portal shows in its list.
#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct AdminGroup {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub auto_department: Option<String>,
    pub is_all_users: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub member_count: i64,
    pub app_count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupDetail {
    #[serde(flatten)]
    pub group: AdminGroup,
    pub user_ids: Vec<Uuid>,
}

const SELECT_GROUP: &str = r#"
select g.id,
       g.name,
       g.description,
       g.auto_department,
       g.is_all_users,
       g.created_at,
       g.updated_at,
       case
           when g.is_all_users then (select count(*) from users u where u.is_active)
           when g.auto_department is not null then
               (select count(*) from users u
                 where u.is_active and u.department = g.auto_department)
           else (select count(*) from group_members gm where gm.group_id = g.id)
       end as member_count,
       (select count(*) from app_groups ag where ag.group_id = g.id) as app_count
  from groups g
"#;

async fn load<'e, E>(executor: E, id: Uuid) -> sqlx::Result<AdminGroup>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    let sql = format!("{SELECT_GROUP} where g.id = $1");
    sqlx::query_as::<_, AdminGroup>(&sql)
        .bind(id)
        .fetch_one(executor)
        .await
}

pub async fn list(pool: &PgPool) -> sqlx::Result<Vec<AdminGroup>> {
    let sql = format!("{SELECT_GROUP} order by g.is_all_users desc, g.name");
    sqlx::query_as::<_, AdminGroup>(&sql).fetch_all(pool).await
}

pub async fn get(pool: &PgPool, id: Uuid) -> Result<Option<GroupDetail>, ApiError> {
    let group = sqlx::query_as::<_, AdminGroup>(&format!("{SELECT_GROUP} where g.id = $1"))
        .bind(id)
        .fetch_optional(pool)
        .await?;

    let Some(group) = group else {
        return Ok(None);
    };

    let user_ids: Vec<Uuid> =
        sqlx::query_scalar("select user_id from group_members where group_id = $1 order by created_at")
            .bind(id)
            .fetch_all(pool)
            .await?;

    Ok(Some(GroupDetail { group, user_ids }))
}

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

fn clean(value: Option<String>) -> Option<String> {
    value
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
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

pub async fn create(
    pool: &PgPool,
    actor: Uuid,
    input: NewGroup,
    ip: Option<&str>,
) -> Result<AdminGroup, ApiError> {
    let name = validate_name(&input.name)?;
    let description = clean(input.description);
    let auto_department = validate_department(input.auto_department)?;

    let mut transaction = pool.begin().await?;

    let id: Uuid = sqlx::query_scalar(
        "insert into groups (name, description, auto_department)
         values ($1, $2, $3)
         returning id",
    )
    .bind(&name)
    .bind(&description)
    .bind(&auto_department)
    .fetch_one(&mut *transaction)
    .await
    .map_err(|error| {
        if is_unique_violation(&error) {
            ApiError::conflict("group_name_taken", "A group with that name already exists.")
        } else {
            ApiError::from(error)
        }
    })?;

    let group = load(&mut *transaction, id).await?;

    audit::record_tx(
        &mut transaction,
        Some(actor),
        "group.create",
        "group",
        Some(id),
        None,
        Some(serde_json::to_value(&group).unwrap_or(serde_json::Value::Null)),
        ip,
    )
    .await;
    revision::bump(&mut transaction).await?;

    transaction.commit().await?;

    Ok(group)
}

pub async fn update(
    pool: &PgPool,
    actor: Uuid,
    id: Uuid,
    patch: GroupPatch,
    ip: Option<&str>,
) -> Result<AdminGroup, ApiError> {
    let mut transaction = pool.begin().await?;

    let before = sqlx::query_as::<_, AdminGroup>(&format!("{SELECT_GROUP} where g.id = $1"))
        .bind(id)
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or_else(|| ApiError::not_found("group_not_found", "That group no longer exists."))?;

    // The built-in group is structural: membership and department are implicit, so only its
    // description is editable.
    if before.is_all_users && (patch.name.is_some() || patch.auto_department.is_some()) {
        return Err(ApiError::conflict(
            "builtin_group",
            "The All Users group can only have its description changed.",
        ));
    }

    let name = match patch.name {
        Some(value) => validate_name(&value)?,
        None => before.name.clone(),
    };
    let description = match patch.description {
        Some(value) => clean(value),
        None => before.description.clone(),
    };
    let auto_department = if before.is_all_users {
        None
    } else {
        match patch.auto_department {
            Some(value) => validate_department(value)?,
            None => before.auto_department.clone(),
        }
    };

    // Switching a group to auto-department makes explicit membership meaningless; clearing
    // `auto_department` leaves the old members in place.
    sqlx::query(
        "update groups
            set name = $2, description = $3, auto_department = $4
          where id = $1",
    )
    .bind(id)
    .bind(&name)
    .bind(&description)
    .bind(&auto_department)
    .execute(&mut *transaction)
    .await
    .map_err(|error| {
        if is_unique_violation(&error) {
            ApiError::conflict("group_name_taken", "A group with that name already exists.")
        } else {
            ApiError::from(error)
        }
    })?;

    if auto_department.is_some() {
        sqlx::query("delete from group_members where group_id = $1")
            .bind(id)
            .execute(&mut *transaction)
            .await?;
    }

    let after = load(&mut *transaction, id).await?;

    audit::record_tx(
        &mut transaction,
        Some(actor),
        "group.update",
        "group",
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

pub async fn delete(pool: &PgPool, actor: Uuid, id: Uuid, ip: Option<&str>) -> Result<(), ApiError> {
    let mut transaction = pool.begin().await?;

    let before = sqlx::query_as::<_, AdminGroup>(&format!("{SELECT_GROUP} where g.id = $1"))
        .bind(id)
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or_else(|| ApiError::not_found("group_not_found", "That group no longer exists."))?;

    if before.is_all_users {
        return Err(ApiError::conflict(
            "builtin_group",
            "The All Users group cannot be deleted.",
        ));
    }

    sqlx::query("delete from groups where id = $1")
        .bind(id)
        .execute(&mut *transaction)
        .await?;

    audit::record_tx(
        &mut transaction,
        Some(actor),
        "group.delete",
        "group",
        Some(id),
        Some(serde_json::to_value(&before).unwrap_or(serde_json::Value::Null)),
        None,
        ip,
    )
    .await;
    revision::bump(&mut transaction).await?;

    transaction.commit().await?;

    Ok(())
}

/// Replaces explicit membership. Refused for derived groups because their membership is
/// computed from `users.department` or from being every active user.
pub async fn set_members(
    pool: &PgPool,
    actor: Uuid,
    id: Uuid,
    user_ids: &[Uuid],
    ip: Option<&str>,
) -> Result<GroupDetail, ApiError> {
    let mut unique: Vec<Uuid> = Vec::with_capacity(user_ids.len());
    for user_id in user_ids {
        if !unique.contains(user_id) {
            unique.push(*user_id);
        }
    }

    let mut transaction = pool.begin().await?;

    let before = sqlx::query_as::<_, AdminGroup>(&format!("{SELECT_GROUP} where g.id = $1"))
        .bind(id)
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or_else(|| ApiError::not_found("group_not_found", "That group no longer exists."))?;

    if before.is_all_users || before.auto_department.is_some() {
        return Err(ApiError::conflict(
            "derived_group",
            "Membership of this group is derived automatically.",
        ));
    }

    if !unique.is_empty() {
        let known: i64 = sqlx::query_scalar("select count(*) from users where id = any($1)")
            .bind(&unique)
            .fetch_one(&mut *transaction)
            .await?;
        if known != unique.len() as i64 {
            return Err(ApiError::bad_request(
                "unknown_user",
                "One of those users no longer exists.",
            ));
        }
    }

    let previous: Vec<Uuid> =
        sqlx::query_scalar("select user_id from group_members where group_id = $1")
            .bind(id)
            .fetch_all(&mut *transaction)
            .await?;

    sqlx::query("delete from group_members where group_id = $1")
        .bind(id)
        .execute(&mut *transaction)
        .await?;

    if !unique.is_empty() {
        sqlx::query(
            "insert into group_members (group_id, user_id)
             select $1, unnest($2::uuid[])
             on conflict do nothing",
        )
        .bind(id)
        .bind(&unique)
        .execute(&mut *transaction)
        .await?;
    }

    let group = load(&mut *transaction, id).await?;

    audit::record_tx(
        &mut transaction,
        Some(actor),
        "group.members",
        "group",
        Some(id),
        Some(json!({ "userIds": previous })),
        Some(json!({ "userIds": unique })),
        ip,
    )
    .await;
    revision::bump(&mut transaction).await?;

    transaction.commit().await?;

    Ok(GroupDetail {
        group,
        user_ids: unique,
    })
}
