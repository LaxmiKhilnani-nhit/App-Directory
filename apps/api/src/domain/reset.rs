use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::json;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::domain::auth;
use crate::domain::users::IssuedCode;
use crate::domain::audit;
use crate::error::ApiError;

/// One row of the admin "forgot password" queue, joined with the account it belongs to.
#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ResetRequestRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub email: String,
    pub name: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
}

pub async fn list(pool: &PgPool, status: Option<&str>) -> sqlx::Result<Vec<ResetRequestRow>> {
    sqlx::query_as::<_, ResetRequestRow>(
        "select r.id,
                r.user_id,
                u.email,
                u.name,
                r.status,
                r.created_at,
                r.resolved_at
           from reset_requests r
           join users u on u.id = r.user_id
          where ($1::text is null or r.status = $1)
          order by r.created_at desc
          limit 500",
    )
    .bind(status)
    .fetch_all(pool)
    .await
}

async fn load_open(pool: &PgPool, id: Uuid) -> Result<ResetRequestRow, ApiError> {
    let request = sqlx::query_as::<_, ResetRequestRow>(
        "select r.id, r.user_id, u.email, u.name, r.status, r.created_at, r.resolved_at
           from reset_requests r
           join users u on u.id = r.user_id
          where r.id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| ApiError::not_found("reset_request_not_found", "That request no longer exists."))?;

    if request.status != "open" {
        return Err(ApiError::conflict(
            "reset_request_closed",
            "That request has already been handled.",
        ));
    }

    Ok(request)
}

pub async fn issue(
    pool: &PgPool,
    actor: Uuid,
    id: Uuid,
    ip: Option<&str>,
) -> Result<IssuedCode, ApiError> {
    let request = load_open(pool, id).await?;

    let mut transaction = pool.begin().await?;

    let (code, expires_at) =
        auth::issue_reset_code_tx(&mut *transaction, request.user_id, "reset", Some(actor), 72).await?;

    sqlx::query(
        "update reset_requests
            set status = 'issued', resolved_at = now(), resolved_by = $2
          where id = $1",
    )
    .bind(id)
    .bind(actor)
    .execute(&mut *transaction)
    .await?;

    audit::record_tx(
        &mut transaction,
        Some(actor),
        "reset_request.issue",
        "reset_request",
        Some(id),
        Some(json!({ "status": "open" })),
        Some(json!({ "status": "issued", "email": request.email })),
        ip,
    )
    .await;

    transaction.commit().await?;

    Ok(IssuedCode { code, expires_at })
}

pub async fn dismiss(pool: &PgPool, actor: Uuid, id: Uuid, ip: Option<&str>) -> Result<(), ApiError> {
    let request = load_open(pool, id).await?;

    let mut transaction = pool.begin().await?;

    sqlx::query(
        "update reset_requests
            set status = 'dismissed', resolved_at = now(), resolved_by = $2
          where id = $1",
    )
    .bind(id)
    .bind(actor)
    .execute(&mut *transaction)
    .await?;

    audit::record_tx(
        &mut transaction,
        Some(actor),
        "reset_request.dismiss",
        "reset_request",
        Some(id),
        Some(json!({ "status": request.status })),
        Some(json!({ "status": "dismissed", "email": request.email })),
        ip,
    )
    .await;

    transaction.commit().await?;

    Ok(())
}
