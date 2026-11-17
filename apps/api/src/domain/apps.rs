use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::{FromRow, PgPool, Postgres};
use uuid::Uuid;

use crate::domain::{audit, revision};
use crate::error::ApiError;

pub const MAX_ICON_BYTES: usize = 512 * 1024;

/// Admin view of an app: everything the portal needs, plus its grants. Icon bytes are
/// deliberately not included — the portal loads them from `/api/v1/icons/:id`.
#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct AdminApp {
    pub id: Uuid,
    pub name: String,
    pub url: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub color: Option<String>,
    pub sort_order: i32,
    pub is_enabled: bool,
    pub offline_visible: bool,
    pub icon_source: String,
    pub icon_mime: Option<String>,
    pub icon_fetched_at: Option<DateTime<Utc>>,
    pub icon_etag: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub group_ids: Vec<Uuid>,
    pub user_ids: Vec<Uuid>,
}

const SELECT_APP: &str = r#"
select a.id,
       a.name,
       a.url,
       a.description,
       a.category,
       a.color,
       a.sort_order,
       a.is_enabled,
       a.offline_visible,
       a.icon_source,
       a.icon_mime,
       a.icon_fetched_at,
       a.icon_etag,
       a.created_at,
       a.updated_at,
       a.deleted_at,
       coalesce(
           (select array_agg(ag.group_id) from app_groups ag where ag.app_id = a.id),
           '{}'::uuid[]
       ) as group_ids,
       coalesce(
           (select array_agg(ua.user_id) from user_apps ua where ua.app_id = a.id),
           '{}'::uuid[]
       ) as user_ids
  from apps a
"#;

async fn load<'e, E>(executor: E, id: Uuid) -> sqlx::Result<AdminApp>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    let sql = format!("{SELECT_APP} where a.id = $1");
    sqlx::query_as::<_, AdminApp>(&sql)
        .bind(id)
        .fetch_one(executor)
        .await
}

pub async fn list(pool: &PgPool) -> sqlx::Result<Vec<AdminApp>> {
    let sql = format!("{SELECT_APP} where a.deleted_at is null order by a.sort_order, a.name");
    sqlx::query_as::<_, AdminApp>(&sql).fetch_all(pool).await
}

pub async fn get(pool: &PgPool, id: Uuid) -> sqlx::Result<Option<AdminApp>> {
    load_live(pool, id).await
}

async fn load_live<'e, E>(executor: E, id: Uuid) -> sqlx::Result<Option<AdminApp>>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    let sql = format!("{SELECT_APP} where a.id = $1 and a.deleted_at is null");
    sqlx::query_as::<_, AdminApp>(&sql)
        .bind(id)
        .fetch_optional(executor)
        .await
}

/// Body of `POST /admin/apps` (and the shape a PATCH merges into).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewApp {
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default = "default_sort_order")]
    pub sort_order: i32,
    #[serde(default = "default_true")]
    pub is_enabled: bool,
    #[serde(default = "default_true")]
    pub offline_visible: bool,
}

fn default_sort_order() -> i32 {
    100
}

fn default_true() -> bool {
    true
}

/// Every field optional; `description`/`category`/`color` accept `null` to clear them.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppPatch {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default, deserialize_with = "crate::domain::de::double_option")]
    pub description: Option<Option<String>>,
    #[serde(default, deserialize_with = "crate::domain::de::double_option")]
    pub category: Option<Option<String>>,
    #[serde(default, deserialize_with = "crate::domain::de::double_option")]
    pub color: Option<Option<String>>,
    #[serde(default)]
    pub sort_order: Option<i32>,
    #[serde(default)]
    pub is_enabled: Option<bool>,
    #[serde(default)]
    pub offline_visible: Option<bool>,
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

fn validate_url(url: &str) -> Result<String, ApiError> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Err(ApiError::bad_request("invalid_url", "URL is required."));
    }
    if trimmed.len() > 500 {
        return Err(ApiError::bad_request(
            "invalid_url",
            "URL must be 500 characters or fewer.",
        ));
    }

    let parsed = reqwest::Url::parse(trimmed)
        .map_err(|_| ApiError::bad_request("invalid_url", "URL is not valid."))?;

    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(ApiError::bad_request(
            "invalid_url",
            "URL must start with http:// or https://.",
        ));
    }
    if parsed.host_str().is_none() {
        return Err(ApiError::bad_request("invalid_url", "URL is missing a host."));
    }

    Ok(trimmed.to_string())
}

fn validate_color(color: &str) -> Result<String, ApiError> {
    let trimmed = color.trim();
    let valid = trimmed.len() == 7
        && trimmed.starts_with('#')
        && trimmed[1..].chars().all(|character| character.is_ascii_hexdigit());

    if !valid {
        return Err(ApiError::bad_request(
            "invalid_color",
            "Colour must look like #2F6FED.",
        ));
    }

    Ok(trimmed.to_ascii_uppercase())
}

fn validate_category(category: Option<String>) -> Result<Option<String>, ApiError> {
    let cleaned = clean(category);
    if let Some(value) = cleaned.as_deref() {
        if value.chars().count() > 80 {
            return Err(ApiError::bad_request(
                "invalid_category",
                "Category must be 80 characters or fewer.",
            ));
        }
    }
    Ok(cleaned)
}

fn validate_description(description: Option<String>) -> Result<Option<String>, ApiError> {
    let cleaned = clean(description);
    if let Some(value) = cleaned.as_deref() {
        if value.chars().count() > 500 {
            return Err(ApiError::bad_request(
                "invalid_description",
                "Description must be 500 characters or fewer.",
            ));
        }
    }
    Ok(cleaned)
}

fn validate_sort_order(sort_order: i32) -> Result<i32, ApiError> {
    if !(-10_000..=10_000).contains(&sort_order) {
        return Err(ApiError::bad_request(
            "invalid_sort_order",
            "Sort order must be between -10000 and 10000.",
        ));
    }
    Ok(sort_order)
}

fn validate_new(input: &NewApp) -> Result<NewApp, ApiError> {
    Ok(NewApp {
        name: validate_name(&input.name)?,
        url: validate_url(&input.url)?,
        description: validate_description(input.description.clone())?,
        category: validate_category(input.category.clone())?,
        color: match clean(input.color.clone()) {
            Some(color) => Some(validate_color(&color)?),
            None => None,
        },
        sort_order: validate_sort_order(input.sort_order)?,
        is_enabled: input.is_enabled,
        offline_visible: input.offline_visible,
    })
}

pub async fn create(
    pool: &PgPool,
    actor: Uuid,
    input: NewApp,
    ip: Option<&str>,
) -> Result<AdminApp, ApiError> {
    let input = validate_new(&input)?;

    let mut transaction = pool.begin().await?;

    let id: Uuid = sqlx::query_scalar(
        "insert into apps (name, url, description, category, color, sort_order,
                           is_enabled, offline_visible)
         values ($1, $2, $3, $4, $5, $6, $7, $8)
         returning id",
    )
    .bind(&input.name)
    .bind(&input.url)
    .bind(&input.description)
    .bind(&input.category)
    .bind(&input.color)
    .bind(input.sort_order)
    .bind(input.is_enabled)
    .bind(input.offline_visible)
    .fetch_one(&mut *transaction)
    .await?;

    let app = load(&mut *transaction, id).await?;

    audit::record_tx(
        &mut transaction,
        Some(actor),
        "app.create",
        "app",
        Some(id),
        None,
        Some(serde_json::to_value(&app).unwrap_or(serde_json::Value::Null)),
        ip,
    )
    .await;
    revision::bump(&mut transaction).await?;

    transaction.commit().await?;

    Ok(app)
}

pub async fn update(
    pool: &PgPool,
    actor: Uuid,
    id: Uuid,
    patch: AppPatch,
    ip: Option<&str>,
) -> Result<AdminApp, ApiError> {
    let mut transaction = pool.begin().await?;

    let before = sqlx::query_as::<_, AdminApp>(&format!(
        "{SELECT_APP} where a.id = $1 and a.deleted_at is null"
    ))
    .bind(id)
    .fetch_optional(&mut *transaction)
    .await?
    .ok_or_else(|| ApiError::not_found("app_not_found", "That app no longer exists."))?;

    let name = match patch.name {
        Some(value) => validate_name(&value)?,
        None => before.name.clone(),
    };
    let url = match patch.url {
        Some(value) => validate_url(&value)?,
        None => before.url.clone(),
    };
    let description = match patch.description {
        Some(value) => validate_description(value)?,
        None => before.description.clone(),
    };
    let category = match patch.category {
        Some(value) => validate_category(value)?,
        None => before.category.clone(),
    };
    let color = match patch.color {
        Some(value) => match clean(value) {
            Some(color) => Some(validate_color(&color)?),
            None => None,
        },
        None => before.color.clone(),
    };
    let sort_order = match patch.sort_order {
        Some(value) => validate_sort_order(value)?,
        None => before.sort_order,
    };
    let is_enabled = patch.is_enabled.unwrap_or(before.is_enabled);
    let offline_visible = patch.offline_visible.unwrap_or(before.offline_visible);

    sqlx::query(
        "update apps
            set name = $2, url = $3, description = $4, category = $5, color = $6,
                sort_order = $7, is_enabled = $8, offline_visible = $9
          where id = $1",
    )
    .bind(id)
    .bind(&name)
    .bind(&url)
    .bind(&description)
    .bind(&category)
    .bind(&color)
    .bind(sort_order)
    .bind(is_enabled)
    .bind(offline_visible)
    .execute(&mut *transaction)
    .await?;

    let after = load(&mut *transaction, id).await?;

    audit::record_tx(
        &mut transaction,
        Some(actor),
        "app.update",
        "app",
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

/// Soft delete: the row and its grants stay for audit and for a possible restore.
pub async fn delete(pool: &PgPool, actor: Uuid, id: Uuid, ip: Option<&str>) -> Result<(), ApiError> {
    let mut transaction = pool.begin().await?;

    let before = load_live(&mut *transaction, id)
        .await?
        .ok_or_else(|| ApiError::not_found("app_not_found", "That app no longer exists."))?;

    sqlx::query("update apps set deleted_at = now(), is_enabled = false where id = $1")
        .bind(id)
        .execute(&mut *transaction)
        .await?;

    audit::record_tx(
        &mut transaction,
        Some(actor),
        "app.delete",
        "app",
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

fn validate_ids(ids: &[Uuid]) -> Vec<Uuid> {
    let mut unique: Vec<Uuid> = Vec::with_capacity(ids.len());
    for id in ids {
        if !unique.contains(id) {
            unique.push(*id);
        }
    }
    unique
}

pub async fn set_groups(
    pool: &PgPool,
    actor: Uuid,
    app_id: Uuid,
    group_ids: &[Uuid],
    ip: Option<&str>,
) -> Result<AdminApp, ApiError> {
    let group_ids = validate_ids(group_ids);

    let mut transaction = pool.begin().await?;

    let app = load_live(&mut *transaction, app_id)
        .await?
        .ok_or_else(|| ApiError::not_found("app_not_found", "That app no longer exists."))?;

    if !group_ids.is_empty() {
        let known: i64 = sqlx::query_scalar("select count(*) from groups where id = any($1)")
            .bind(&group_ids)
            .fetch_one(&mut *transaction)
            .await?;
        if known != group_ids.len() as i64 {
            return Err(ApiError::bad_request(
                "unknown_group",
                "One of those groups no longer exists.",
            ));
        }
    }

    sqlx::query("delete from app_groups where app_id = $1")
        .bind(app_id)
        .execute(&mut *transaction)
        .await?;

    if !group_ids.is_empty() {
        sqlx::query(
            "insert into app_groups (app_id, group_id)
             select $1, unnest($2::uuid[])
             on conflict do nothing",
        )
        .bind(app_id)
        .bind(&group_ids)
        .execute(&mut *transaction)
        .await?;
    }

    let after = load(&mut *transaction, app_id).await?;

    audit::record_tx(
        &mut transaction,
        Some(actor),
        "app.groups",
        "app",
        Some(app_id),
        Some(json!({ "groupIds": app.group_ids })),
        Some(json!({ "groupIds": after.group_ids })),
        ip,
    )
    .await;
    revision::bump(&mut transaction).await?;

    transaction.commit().await?;

    Ok(after)
}

pub async fn set_users(
    pool: &PgPool,
    actor: Uuid,
    app_id: Uuid,
    user_ids: &[Uuid],
    ip: Option<&str>,
) -> Result<AdminApp, ApiError> {
    let user_ids = validate_ids(user_ids);

    let mut transaction = pool.begin().await?;

    let app = load_live(&mut *transaction, app_id)
        .await?
        .ok_or_else(|| ApiError::not_found("app_not_found", "That app no longer exists."))?;

    if !user_ids.is_empty() {
        let known: i64 = sqlx::query_scalar("select count(*) from users where id = any($1)")
            .bind(&user_ids)
            .fetch_one(&mut *transaction)
            .await?;
        if known != user_ids.len() as i64 {
            return Err(ApiError::bad_request(
                "unknown_user",
                "One of those users no longer exists.",
            ));
        }
    }

    sqlx::query("delete from user_apps where app_id = $1")
        .bind(app_id)
        .execute(&mut *transaction)
        .await?;

    if !user_ids.is_empty() {
        sqlx::query(
            "insert into user_apps (app_id, user_id)
             select $1, unnest($2::uuid[])
             on conflict do nothing",
        )
        .bind(app_id)
        .bind(&user_ids)
        .execute(&mut *transaction)
        .await?;
    }

    let after = load(&mut *transaction, app_id).await?;

    audit::record_tx(
        &mut transaction,
        Some(actor),
        "app.users",
        "app",
        Some(app_id),
        Some(json!({ "userIds": app.user_ids })),
        Some(json!({ "userIds": after.user_ids })),
        ip,
    )
    .await;
    revision::bump(&mut transaction).await?;

    transaction.commit().await?;

    Ok(after)
}

/// Stores icon bytes and derives the ETag clients validate against.
pub async fn set_icon(
    pool: &PgPool,
    actor: Uuid,
    app_id: Uuid,
    bytes: Vec<u8>,
    mime: &str,
    source: &str,
    ip: Option<&str>,
) -> Result<AdminApp, ApiError> {
    if bytes.is_empty() {
        return Err(ApiError::bad_request("invalid_icon", "The icon file is empty."));
    }
    if bytes.len() > MAX_ICON_BYTES {
        return Err(ApiError::bad_request(
            "icon_too_large",
            "Icons must be 512 KB or smaller.",
        ));
    }

    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let digest = hasher.finalize();
    let etag = format!("W/\"{}\"", hex_prefix(&digest, 16));

    let mut transaction = pool.begin().await?;

    let before = load_live(&mut *transaction, app_id)
        .await?
        .ok_or_else(|| ApiError::not_found("app_not_found", "That app no longer exists."))?;

    sqlx::query(
        "update apps
            set icon_data = $2, icon_mime = $3, icon_source = $4,
                icon_fetched_at = now(), icon_etag = $5
          where id = $1",
    )
    .bind(app_id)
    .bind(&bytes)
    .bind(mime)
    .bind(source)
    .bind(&etag)
    .execute(&mut *transaction)
    .await?;

    let after = load(&mut *transaction, app_id).await?;

    audit::record_tx(
        &mut transaction,
        Some(actor),
        "app.icon",
        "app",
        Some(app_id),
        Some(json!({
            "iconSource": before.icon_source,
            "iconEtag": before.icon_etag,
        })),
        Some(json!({
            "iconSource": after.icon_source,
            "iconMime": after.icon_mime,
            "iconEtag": after.icon_etag,
            "bytes": bytes.len(),
        })),
        ip,
    )
    .await;

    // The icon is part of the snapshot the client caches, but the URL and ETag only change
    // with the app row; bump anyway so clients refresh promptly.
    revision::bump(&mut transaction).await?;

    transaction.commit().await?;

    Ok(after)
}

fn hex_prefix(bytes: &[u8], length: usize) -> String {
    let mut out = String::with_capacity(length * 2);
    for byte in bytes.iter().take(length) {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours_are_validated_and_normalised() {
        assert_eq!(validate_color("#2f6fed").unwrap(), "#2F6FED");
        assert!(validate_color("2F6FED").is_err());
        assert!(validate_color("#GGGGGG").is_err());
        assert!(validate_color("#12345").is_err());
    }

    #[test]
    fn urls_must_be_http() {
        assert!(validate_url("http://172.168.50.50:8080/payroll").is_ok());
        assert!(validate_url("https://example.com").is_ok());
        assert!(validate_url("javascript:alert(1)").is_err());
        assert!(validate_url("file:///c:/temp").is_err());
        assert!(validate_url("example.com").is_err());
    }
}
