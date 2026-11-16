use serde::Serialize;
use sqlx::{FromRow, PgPool};

/// Dashboard counters. One round trip, all sub-selects so the numbers are consistent.
#[derive(Debug, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub users: i64,
    pub active_users: i64,
    pub apps: i64,
    pub enabled_apps: i64,
    pub groups: i64,
    pub open_reset_requests: i64,
    pub logins_today: i64,
    pub revision: i64,
}

pub async fn get(pool: &PgPool) -> sqlx::Result<Stats> {
    sqlx::query_as::<_, Stats>(
        "select
            (select count(*) from users) as users,
            (select count(*) from users where is_active) as active_users,
            (select count(*) from apps where deleted_at is null) as apps,
            (select count(*) from apps where deleted_at is null and is_enabled) as enabled_apps,
            (select count(*) from groups) as groups,
            (select count(*) from reset_requests where status = 'open') as open_reset_requests,
            (select count(*) from users
              where last_login_at >= date_trunc('day', now())) as logins_today,
            (select revision from sync_state where id = 1) as revision",
    )
    .fetch_one(pool)
    .await
}
