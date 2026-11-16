use sqlx::{PgConnection, PgPool};

/// Advances the snapshot revision and returns the new value.
///
/// Every change that alters what a client sees calls this inside the same transaction as the
/// mutation, so a client can never observe a half-applied change. The counter lives in
/// `sync_state` (single row) and the SQL function is defined in the initial migration.
pub async fn bump(transaction: &mut PgConnection) -> sqlx::Result<i64> {
    sqlx::query_scalar("select bump_revision()")
        .fetch_one(transaction)
        .await
}

pub async fn current(pool: &PgPool) -> sqlx::Result<i64> {
    sqlx::query_scalar("select revision from sync_state where id = 1")
        .fetch_one(pool)
        .await
}
