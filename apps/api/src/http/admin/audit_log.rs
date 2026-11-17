use axum::{
    extract::{Query, State},
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::error::{ApiError, ApiResult};
use crate::http::admin::{pool_of, AdminUser};
use crate::state::AppState;

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct AuditRow {
    pub id: i64,
    pub actor_user_id: Option<Uuid>,
    pub actor_email: Option<String>,
    pub action: String,
    pub entity_type: String,
    pub entity_id: Option<Uuid>,
    pub before: Option<serde_json::Value>,
    pub after: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditQuery {
    /// Email address or UUID of the acting account.
    pub actor: Option<String>,
    /// Accepts both `entityType` (camelCase, used by the SPA) and `entity_type` (PLAN §5).
    #[serde(alias = "entity_type")]
    pub entity_type: Option<String>,
    pub action: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditPage {
    pub entries: Vec<AuditRow>,
    pub total: i64,
}

fn parse_time(raw: Option<&str>, name: &str) -> Result<Option<DateTime<Utc>>, ApiError> {
    match raw {
        None => Ok(None),
        Some(value) => DateTime::parse_from_rfc3339(value)
            .map(|parsed| Some(parsed.with_timezone(&Utc)))
            .map_err(|_| {
                ApiError::bad_request(
                    "invalid_time",
                    format!("{name} must be an ISO-8601 timestamp."),
                )
            }),
    }
}

const AUDIT_SQL: &str = r#"
select a.id,
       a.actor_user_id,
       u.email as actor_email,
       a.action,
       a.entity_type,
       a.entity_id,
       a.before,
       a.after,
       a.created_at
  from audit_log a
  left join users u on u.id = a.actor_user_id
 where ($1::uuid is null or a.actor_user_id = $1)
   and ($2::text is null or a.entity_type = $2)
   and ($3::text is null or a.action = $3)
   and ($4::timestamptz is null or a.created_at >= $4)
   and ($5::timestamptz is null or a.created_at <= $5)
"#;

pub async fn list(
    State(state): State<AppState>,
    AdminUser(_session): AdminUser,
    Query(params): Query<AuditQuery>,
) -> ApiResult<Json<AuditPage>> {
    let pool: &PgPool = pool_of(&state)?;

    let from = parse_time(params.from.as_deref(), "from")?;
    let to = parse_time(params.to.as_deref(), "to")?;

    let actor_id = match params.actor.as_deref() {
        None => None,
        Some(raw) if raw.trim().is_empty() => None,
        Some(raw) => match Uuid::parse_str(raw.trim()) {
            Ok(id) => Some(id),
            Err(_) => {
                let found: Option<Uuid> =
                    sqlx::query_scalar("select id from users where lower(email) = lower($1)")
                        .bind(raw.trim())
                        .fetch_optional(pool)
                        .await?;

                match found {
                    Some(id) => Some(id),
                    // An email that matches nobody can match no entries.
                    None => {
                        return Ok(Json(AuditPage {
                            entries: Vec::new(),
                            total: 0,
                        }))
                    }
                }
            }
        },
    };

    let limit = params.limit.unwrap_or(100).clamp(1, 500);
    let offset = params.offset.unwrap_or(0).max(0);

    let entries = sqlx::query_as::<_, AuditRow>(&format!(
        "{AUDIT_SQL} order by a.id desc limit $6 offset $7"
    ))
    .bind(actor_id)
    .bind(params.entity_type.as_deref())
    .bind(params.action.as_deref())
    .bind(from)
    .bind(to)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    let total: i64 = sqlx::query_scalar(&format!(
        "select count(*) from ({AUDIT_SQL}) filtered"
    ))
    .bind(actor_id)
    .bind(params.entity_type.as_deref())
    .bind(params.action.as_deref())
    .bind(from)
    .bind(to)
    .fetch_one(pool)
    .await?;

    Ok(Json(AuditPage { entries, total }))
}
