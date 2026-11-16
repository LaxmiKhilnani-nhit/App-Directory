use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

/// Appends one audit row.
///
/// Audit failures must never fail the request they describe, so errors are logged and
/// swallowed: a user changing their password should not get a 500 because the log insert
/// timed out. Phase 3's admin mutations use this same helper.
#[allow(clippy::too_many_arguments)]
pub async fn record(
    pool: &PgPool,
    actor_user_id: Option<Uuid>,
    action: &str,
    entity_type: &str,
    entity_id: Option<Uuid>,
    before: Option<Value>,
    after: Option<Value>,
    ip: Option<&str>,
) {
    let result = sqlx::query(
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
    .execute(pool)
    .await;

    if let Err(error) = result {
        tracing::warn!(%error, action, entity_type, "failed to write audit log entry");
    }
}
