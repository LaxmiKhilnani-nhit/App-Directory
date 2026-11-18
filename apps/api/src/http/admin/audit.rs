use axum::{extract::Query, Json};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use uuid::Uuid;

use crate::domain::models::{AuditPage, AuditRow};
use crate::error::{ApiError, ApiResult};
use crate::http::identity::TrustedUser;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditQuery {
    pub actor: Option<String>,
    #[serde(alias = "entity_type")]
    pub entity_type: Option<String>,
    pub action: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
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

pub async fn list(
    axum::extract::State(state): axum::extract::State<crate::state::AppState>,
    TrustedUser(_actor): TrustedUser,
    Query(params): Query<AuditQuery>,
) -> ApiResult<Json<AuditPage>> {
    let from = parse_time(params.from.as_deref(), "from")?;
    let to = parse_time(params.to.as_deref(), "to")?;

    let actor_id = match params.actor.as_deref() {
        None | Some("") => None,
        Some(raw) => match Uuid::parse_str(raw.trim()) {
            Ok(id) => Some(id),
            Err(_) => {
                let data = state.store.read();
                let needle = raw.trim().to_lowercase();
                match data
                    .users
                    .iter()
                    .find(|user| user.email.to_lowercase() == needle)
                {
                    Some(user) => Some(user.id),
                    None => return Ok(Json(AuditPage { entries: Vec::new(), total: 0 })),
                }
            }
        },
    };

    let data = state.store.read();
    let mut entries: Vec<AuditRow> = data
        .audit
        .iter()
        .filter(|row| actor_id.is_none_or(|id| row.actor_user_id == Some(id)))
        .filter(|row| {
            params
                .entity_type
                .as_deref()
                .is_none_or(|value| row.entity_type == value)
        })
        .filter(|row| params.action.as_deref().is_none_or(|value| row.action == value))
        .filter(|row| from.is_none_or(|value| row.created_at >= value))
        .filter(|row| to.is_none_or(|value| row.created_at <= value))
        .map(AuditRow::from)
        .collect();
    entries.sort_by(|left, right| right.id.cmp(&left.id));

    let total = entries.len() as i64;
    let offset = params.offset.unwrap_or(0).max(0) as usize;
    let limit = params.limit.unwrap_or(100).clamp(1, 500) as usize;

    Ok(Json(AuditPage {
        entries: entries.into_iter().skip(offset).take(limit).collect(),
        total,
    }))
}
