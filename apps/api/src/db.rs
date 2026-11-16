use std::time::Duration;

use anyhow::Context;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

use crate::config::Config;

/// Connects to PostgreSQL and applies migrations.
///
/// Returns `None` when no `DATABASE_URL` is configured, which keeps the health endpoint and
/// static admin bundle usable while working on the client. Retries because the database
/// container may still be starting.
pub async fn connect_and_migrate(config: &Config) -> anyhow::Result<Option<PgPool>> {
    let Some(url) = config.database_url.as_deref() else {
        tracing::warn!("DATABASE_URL is not set; starting without a database");
        return Ok(None);
    };

    let pool = PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(10))
        .connect_with_retry(url, 15)
        .await
        .context("connecting to PostgreSQL")?;

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .context("running migrations")?;

    let revision: i64 = sqlx::query_scalar("select revision from sync_state where id = 1")
        .fetch_one(&pool)
        .await
        .context("reading sync revision")?;

    tracing::info!(revision, "database ready");
    Ok(Some(pool))
}

/// Small helper so startup tolerates a database that is still coming up.
trait ConnectWithRetry {
    async fn connect_with_retry(self, url: &str, attempts: u32) -> Result<PgPool, sqlx::Error>;
}

impl ConnectWithRetry for PgPoolOptions {
    async fn connect_with_retry(self, url: &str, attempts: u32) -> Result<PgPool, sqlx::Error> {
        let mut last_error = None;

        for attempt in 1..=attempts {
            match self.clone().connect(url).await {
                Ok(pool) => return Ok(pool),
                Err(error) => {
                    tracing::warn!(attempt, %error, "database connection failed; retrying");
                    last_error = Some(error);
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            }
        }

        Err(last_error.expect("at least one attempt is always made"))
    }
}
