use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::auth::LockoutPolicy;

pub const APP_NAME: &str = "app_name";
pub const SYNC_INTERVAL_HOURS: &str = "sync_interval_hours";
pub const OFFLINE_GRACE_DAYS: &str = "offline_grace_days";
pub const ANNOUNCEMENT: &str = "announcement";
pub const PASSWORD_MIN_LENGTH: &str = "password_min_length";
pub const LOGIN_LOCKOUT_ATTEMPTS: &str = "login_lockout_attempts";
pub const LOGIN_LOCKOUT_MINUTES: &str = "login_lockout_minutes";

/// Values inserted on first start. Existing rows are never overwritten, so restarting the
/// container cannot undo an admin's changes.
pub fn defaults() -> Vec<(&'static str, Value)> {
    vec![
        (APP_NAME, json!("App Directory")),
        (SYNC_INTERVAL_HOURS, json!(4)),
        (OFFLINE_GRACE_DAYS, json!(7)),
        (ANNOUNCEMENT, Value::Null),
        (PASSWORD_MIN_LENGTH, json!(12)),
        (LOGIN_LOCKOUT_ATTEMPTS, json!(5)),
        (LOGIN_LOCKOUT_MINUTES, json!(15)),
    ]
}

pub async fn ensure_defaults(pool: &PgPool) -> anyhow::Result<()> {
    for (key, value) in defaults() {
        sqlx::query(
            "insert into settings (key, value) values ($1, $2) on conflict (key) do nothing",
        )
        .bind(key)
        .bind(value)
        .execute(pool)
        .await?;
    }

    Ok(())
}

pub async fn get<T: DeserializeOwned>(pool: &PgPool, key: &str) -> anyhow::Result<Option<T>> {
    let raw: Option<Value> = sqlx::query_scalar("select value from settings where key = $1")
        .bind(key)
        .fetch_optional(pool)
        .await?;

    match raw {
        Some(value) => Ok(Some(serde_json::from_value(value)?)),
        None => Ok(None),
    }
}

pub async fn put(
    pool: &PgPool,
    key: &str,
    value: &Value,
    actor: Option<Uuid>,
) -> sqlx::Result<()> {
    put_with(pool, key, value, actor).await
}

/// Same as [`put`], through any executor so a batch of settings and its audit row can share
/// one transaction.
pub async fn put_with<'e, E>(
    executor: E,
    key: &str,
    value: &Value,
    actor: Option<Uuid>,
) -> sqlx::Result<()>
where
    E: sqlx::Executor<'e, Database = sqlx::Postgres>,
{
    sqlx::query(
        "insert into settings (key, value, updated_by) values ($1, $2, $3)
         on conflict (key) do update set value = excluded.value, updated_by = excluded.updated_by",
    )
    .bind(key)
    .bind(value)
    .bind(actor)
    .execute(executor)
    .await?;

    Ok(())
}

pub async fn sync_interval_hours(pool: &PgPool) -> i64 {
    get::<i64>(pool, SYNC_INTERVAL_HOURS)
        .await
        .ok()
        .flatten()
        .unwrap_or(4)
}

pub async fn offline_grace_days(pool: &PgPool) -> i64 {
    get::<i64>(pool, OFFLINE_GRACE_DAYS)
        .await
        .ok()
        .flatten()
        .unwrap_or(7)
}

/// Failed-attempt budget before an account is temporarily locked.
pub async fn lockout_policy(pool: &PgPool) -> LockoutPolicy {
    let attempts = get::<i64>(pool, LOGIN_LOCKOUT_ATTEMPTS)
        .await
        .ok()
        .flatten()
        .unwrap_or(5);
    let minutes = get::<i64>(pool, LOGIN_LOCKOUT_MINUTES)
        .await
        .ok()
        .flatten()
        .unwrap_or(15);

    LockoutPolicy { attempts, minutes }
}

pub async fn password_min_length(pool: &PgPool) -> usize {
    get::<i64>(pool, PASSWORD_MIN_LENGTH)
        .await
        .ok()
        .flatten()
        .unwrap_or(12)
        .clamp(1, 200) as usize
}
