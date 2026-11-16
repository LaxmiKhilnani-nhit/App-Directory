use std::collections::HashMap;

use axum::{
    extract::{rejection::JsonRejection, State},
    Json,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::{json, Map, Value};

use crate::domain::models::Setting;
use crate::domain::{audit, revision, settings as settings_domain};
use crate::error::{ApiError, ApiResult};
use crate::http::admin::{pool_of, AdminUser, SuperAdmin};
use crate::http::extract::ClientIp;
use crate::http::json_body;
use crate::state::AppState;

pub async fn list(
    State(state): State<AppState>,
    AdminUser(_session): AdminUser,
) -> ApiResult<Json<Vec<Setting>>> {
    let pool = pool_of(&state)?;
    Ok(Json(load(pool).await?))
}

async fn load(pool: &sqlx::PgPool) -> sqlx::Result<Vec<Setting>> {
    sqlx::query_as::<_, Setting>(
        "select key, value, updated_at, updated_by from settings order by key",
    )
    .fetch_all(pool)
    .await
}

#[derive(Debug, Deserialize)]
pub struct UpdateRequest {
    pub values: Value,
}

fn require_int(value: &Value, key: &str, min: i64, max: i64) -> Result<Value, ApiError> {
    let number = value.as_i64().ok_or_else(|| {
        ApiError::bad_request("invalid_setting", format!("{key} must be a whole number."))
    })?;

    if number < min || number > max {
        return Err(ApiError::bad_request(
            "invalid_setting",
            format!("{key} must be between {min} and {max}."),
        ));
    }

    Ok(json!(number))
}

fn require_string(value: &Value, key: &str, max: usize) -> Result<Value, ApiError> {
    let text = value.as_str().map(str::trim).unwrap_or("");

    if text.is_empty() {
        return Err(ApiError::bad_request(
            "invalid_setting",
            format!("{key} cannot be empty."),
        ));
    }
    if text.chars().count() > max {
        return Err(ApiError::bad_request(
            "invalid_setting",
            format!("{key} must be {max} characters or fewer."),
        ));
    }

    Ok(json!(text))
}

fn validate_announcement(value: &Value) -> Result<Value, ApiError> {
    if value.is_null() {
        return Ok(Value::Null);
    }

    let object = value.as_object().ok_or_else(|| {
        ApiError::bad_request(
            "invalid_setting",
            "announcement must be null or an object with text and level.",
        )
    })?;

    let text = object
        .get("text")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("");

    if text.is_empty() || text.chars().count() > 300 {
        return Err(ApiError::bad_request(
            "invalid_setting",
            "announcement.text must be between 1 and 300 characters.",
        ));
    }

    let level = object
        .get("level")
        .and_then(Value::as_str)
        .unwrap_or("info");

    if !matches!(level, "info" | "warning" | "critical") {
        return Err(ApiError::bad_request(
            "invalid_setting",
            "announcement.level must be info, warning or critical.",
        ));
    }

    let mut normalised = Map::new();
    normalised.insert("text".to_string(), json!(text));
    normalised.insert("level".to_string(), json!(level));

    for (source, target) in [("activeFrom", "activeFrom"), ("activeTo", "activeTo")] {
        match object.get(source) {
            None | Some(Value::Null) => {}
            Some(raw) => {
                let raw = raw.as_str().ok_or_else(|| {
                    ApiError::bad_request(
                        "invalid_setting",
                        format!("announcement.{source} must be an ISO-8601 timestamp."),
                    )
                })?;
                let parsed = DateTime::parse_from_rfc3339(raw).map_err(|_| {
                    ApiError::bad_request(
                        "invalid_setting",
                        format!("announcement.{source} must be an ISO-8601 timestamp."),
                    )
                })?;
                normalised.insert(target.to_string(), json!(parsed.with_timezone(&Utc)));
            }
        }
    }

    Ok(Value::Object(normalised))
}

fn validate(key: &str, value: &Value) -> Result<Value, ApiError> {
    match key {
        settings_domain::APP_NAME => require_string(value, key, 80),
        settings_domain::SYNC_INTERVAL_HOURS => require_int(value, key, 1, 168),
        settings_domain::OFFLINE_GRACE_DAYS => require_int(value, key, 0, 90),
        settings_domain::ANNOUNCEMENT => validate_announcement(value),
        settings_domain::PASSWORD_MIN_LENGTH => require_int(value, key, 8, 200),
        settings_domain::LOGIN_LOCKOUT_ATTEMPTS => require_int(value, key, 0, 100),
        settings_domain::LOGIN_LOCKOUT_MINUTES => require_int(value, key, 1, 1440),
        other => Err(ApiError::bad_request(
            "unknown_setting",
            format!("{other} is not a setting this server knows."),
        )),
    }
}

pub async fn update(
    State(state): State<AppState>,
    SuperAdmin(session): SuperAdmin,
    ClientIp(ip): ClientIp,
    body: Result<Json<UpdateRequest>, JsonRejection>,
) -> ApiResult<Json<Vec<Setting>>> {
    let request = json_body(body)?;
    let pool = pool_of(&state)?;

    let incoming = request.values.as_object().ok_or_else(|| {
        ApiError::bad_request("invalid_request", "values must be an object of settings.")
    })?;

    if incoming.is_empty() {
        return Err(ApiError::bad_request(
            "invalid_request",
            "No settings were provided.",
        ));
    }

    let before = load(pool).await?;
    let before_map: HashMap<String, Value> = before
        .into_iter()
        .map(|setting| (setting.key, setting.value))
        .collect();

    let mut validated: Vec<(String, Value)> = Vec::with_capacity(incoming.len());
    for (key, value) in incoming {
        validated.push((key.clone(), validate(key, value)?));
    }

    let mut transaction = pool.begin().await?;

    for (key, value) in &validated {
        settings_domain::put_with(&mut *transaction, key, value, Some(session.user.id)).await?;
    }

    let after: HashMap<String, Value> = validated.iter().cloned().collect();

    audit::record_tx(
        &mut transaction,
        Some(session.user.id),
        "settings.update",
        "settings",
        None,
        Some(json!(before_map)),
        Some(json!(after)),
        ip.map(|a| a.to_string()).as_deref(),
    )
    .await;

    // An announcement change alters the client snapshot, and app_name is part of it too, so
    // every settings write bumps the revision.
    revision::bump(&mut transaction).await?;

    transaction.commit().await?;

    Ok(Json(load(pool).await?))
}
