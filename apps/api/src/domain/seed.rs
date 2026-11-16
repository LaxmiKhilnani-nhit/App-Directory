use sqlx::PgPool;

use crate::config::Config;
use crate::domain::{password, settings};

pub const ALL_USERS_GROUP: &str = "All Users";

/// Idempotent startup seeding: settings defaults, the built-in All Users group, and the
/// first administrator when no super admin exists yet.
pub async fn run(pool: &PgPool, config: &Config) -> anyhow::Result<()> {
    settings::ensure_defaults(pool).await?;
    ensure_all_users_group(pool).await?;
    ensure_bootstrap_admin(pool, config).await?;
    Ok(())
}

async fn ensure_all_users_group(pool: &PgPool) -> anyhow::Result<()> {
    sqlx::query(
        "insert into groups (name, description, is_all_users)
         values ($1, 'Every active user', true)
         on conflict (name) do nothing",
    )
    .bind(ALL_USERS_GROUP)
    .execute(pool)
    .await?;

    Ok(())
}

async fn ensure_bootstrap_admin(pool: &PgPool, config: &Config) -> anyhow::Result<()> {
    let Some(email) = config.bootstrap_admin_email.as_deref() else {
        return Ok(());
    };
    let Some(password_plain) = config.bootstrap_admin_password.as_deref() else {
        tracing::warn!(
            "BOOTSTRAP_ADMIN_EMAIL is set but BOOTSTRAP_ADMIN_PASSWORD is not; skipping seeding"
        );
        return Ok(());
    };

    let super_admins: i64 =
        sqlx::query_scalar("select count(*) from users where role = 'super_admin'")
            .fetch_one(pool)
            .await?;

    if super_admins > 0 {
        return Ok(());
    }

    let exists: bool = sqlx::query_scalar(
        "select exists (select 1 from users where lower(email) = lower($1))",
    )
    .bind(email)
    .fetch_one(pool)
    .await?;

    if exists {
        tracing::warn!(
            email = %email,
            "no super admin exists and the bootstrap email is already taken by a non-admin user"
        );
        return Ok(());
    }

    let hash = password::hash_password(password_plain)?;

    sqlx::query(
        "insert into users (email, name, role, password_hash, must_change_password)
         values (lower($1), 'Administrator', 'super_admin', $2, true)",
    )
    .bind(email)
    .bind(hash)
    .execute(pool)
    .await?;

    tracing::info!(email = %email, "bootstrap super admin created; password change required at first login");

    Ok(())
}
