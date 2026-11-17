use sqlx::{PgConnection, Postgres};
use uuid::Uuid;

/// Inserts one audit row through the caller's executor.
async fn insert<'e, E>(
    executor: E,
    actor_user_id: Option<Uuid>,
    action: &str,
    entity_type: &str,
    entity_id: Option<Uuid>,
    before: Option<serde_json::Value>,
    after: Option<serde_json::Value>,
    ip: Option<&str>,
) -> sqlx::Result<()>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    sqlx::query(
        "insert into audit_log
             (actor_user_id, action, entity_type, entity_id, before, after, ip)
         values ($1, $2, $3, $4, $5, $6, nullif($7::text, '')::inet)",
    )
    .bind(actor_user_id)
    .bind(action)
    .bind(entity_type)
    .bind(entity_id)
    .bind(before)
    .bind(after)
    .bind(ip)
    .execute(executor)
    .await?;

    Ok(())
}

/// Fire-and-forget audit write for the few places that are not already in a transaction
/// (sign-in, sign-out, password changes). Losing a log row must not fail the request the
/// user just completed, so errors are logged and swallowed.
#[allow(clippy::too_many_arguments)]
pub async fn record<'e, E>(
    executor: E,
    actor_user_id: Option<Uuid>,
    action: &str,
    entity_type: &str,
    entity_id: Option<Uuid>,
    before: Option<serde_json::Value>,
    after: Option<serde_json::Value>,
    ip: Option<&str>,
) where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    if let Err(error) = insert(
        executor,
        actor_user_id,
        action,
        entity_type,
        entity_id,
        before,
        after,
        ip,
    )
    .await
    {
        tracing::warn!(%error, action, entity_type, "failed to write audit log entry");
    }
}

/// Audit write joined to the caller's transaction so the change and its log entry commit
/// or roll back together.
///
/// The log insert runs inside a savepoint. Swallowing an error without one would leave the
/// outer transaction aborted, and the caller's commit would then fail — turning a lost log
/// row into a failed admin change.
#[allow(clippy::too_many_arguments)]
pub async fn record_tx(
    transaction: &mut PgConnection,
    actor_user_id: Option<Uuid>,
    action: &str,
    entity_type: &str,
    entity_id: Option<Uuid>,
    before: Option<serde_json::Value>,
    after: Option<serde_json::Value>,
    ip: Option<&str>,
) {
    const SAVEPOINT: &str = "audit_log_entry";

    if let Err(error) = sqlx::query(&format!("savepoint {SAVEPOINT}"))
        .execute(&mut *transaction)
        .await
    {
        tracing::warn!(%error, action, "could not open the audit savepoint");
        return;
    }

    match insert(
        &mut *transaction,
        actor_user_id,
        action,
        entity_type,
        entity_id,
        before,
        after,
        ip,
    )
    .await
    {
        Ok(()) => {
            if let Err(error) = sqlx::query(&format!("release savepoint {SAVEPOINT}"))
                .execute(&mut *transaction)
                .await
            {
                tracing::warn!(%error, action, "could not release the audit savepoint");
            }
        }
        Err(error) => {
            tracing::warn!(%error, action, entity_type, "failed to write audit log entry");

            if let Err(error) =
                sqlx::query(&format!("rollback to savepoint {SAVEPOINT}"))
                    .execute(&mut *transaction)
                    .await
            {
                tracing::warn!(%error, action, "could not roll back the audit savepoint");
                return;
            }

            let _ = sqlx::query(&format!("release savepoint {SAVEPOINT}"))
                .execute(&mut *transaction)
                .await;
        }
    }
}
