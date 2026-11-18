use std::collections::HashMap;

use axum::{
    extract::{rejection::JsonRejection, State},
    Json,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::{json, Map, Value};

use crate::domain::models::Setting;
use crate::error::{ApiError, ApiResult};
use crate::http::identity::TrustedUser;
use crate::http::json_body;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct UpdateRequest {
    pub values: Value,
}

fn require_int(value: &Value, key: &str, min: i64, max: i64) -> Result<Value, ApiError> {
    let number = value.as_i64().ok_or_else(|| {
        ApiError::bad_request("invalid_setting", format!("{key} must be a whole number."))
    })?;
    if !(min..=max).contains(&number) {
        return Err(ApiError::bad_request(
            "invalid_setting",
            format!("{key} must be between {min} and {max}."),
        ));
    }
    Ok(json!(number))
}

fn require_flag(value: &Value, key: &str) -> Result<Value, ApiError> {
    match value {
        Value::Bool(flag) => Ok(json!(*flag)),
        Value::Number(number) if number.as_i64() == Some(0) => Ok(json!(0)),
        Value::Number(number) if number.as_i64() == Some(1) => Ok(json!(1)),
        Value::String(text) if matches!(text.trim(), "0" | "1" | "true" | "false") => {
            Ok(json!(matches!(text.trim(), "1" | "true")))
        }
        _ => Err(ApiError::bad_request(
            "invalid_setting",
            format!("{key} must be true or false."),
        )),
    }
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

fn optional_string(value: &Value, key: &str, max: usize) -> Result<Value, ApiError> {
    let text = value
        .as_str()
        .ok_or_else(|| ApiError::bad_request("invalid_setting", format!("{key} must be text.")))?;
    if text.chars().count() > max {
        return Err(ApiError::bad_request(
            "invalid_setting",
            format!("{key} must be {max} characters or fewer."),
        ));
    }
    Ok(json!(text.trim()))
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
    for source in ["activeFrom", "activeTo"] {
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
                normalised.insert(source.to_string(), json!(parsed.with_timezone(&Utc)));
            }
        }
    }
    Ok(Value::Object(normalised))
}

fn validate(key: &str, value: &Value) -> Result<Value, ApiError> {
    match key {
        "app_name" => require_string(value, key, 80),
        "sync_interval_hours" => require_int(value, key, 1, 168),
        "offline_grace_days" => require_int(value, key, 0, 90),
        "announcement" => validate_announcement(value),
        "mail_domain" => require_string(value, key, 253),
        "default_identity" => optional_string(value, key, 253),
        "update_enabled" => require_flag(value, key),
        "update_version" => optional_string(value, key, 120),
        "update_sha256" => optional_string(value, key, 128),
        "update_url" => optional_string(value, key, 500),
        other => Err(ApiError::bad_request(
            "unknown_setting",
            format!("{other} is not a setting this server knows."),
        )),
    }
}

pub async fn list(
    State(state): State<AppState>,
    TrustedUser(_actor): TrustedUser,
) -> ApiResult<Json<Vec<Setting>>> {
    let data = state.store.read();
    let mut settings: Vec<Setting> = data.settings.iter().map(Setting::from).collect();
    settings.sort_by(|left, right| left.key.cmp(&right.key));
    Ok(Json(settings))
}

pub async fn update(
    State(state): State<AppState>,
    TrustedUser(actor): TrustedUser,
    body: Result<Json<UpdateRequest>, JsonRejection>,
) -> ApiResult<Json<Vec<Setting>>> {
    let request = json_body(body)?;
    let incoming = request.values.as_object().ok_or_else(|| {
        ApiError::bad_request("invalid_request", "values must be an object of settings.")
    })?;
    if incoming.is_empty() {
        return Err(ApiError::bad_request(
            "invalid_request",
            "No settings were provided.",
        ));
    }

    let mut validated: Vec<(String, Value)> = Vec::with_capacity(incoming.len());
    for (key, value) in incoming {
        validated.push((key.clone(), validate(key, value)?));
    }

    let actor_id = actor.id;
    let actor_email = actor.email.clone();
    let before: HashMap<String, Value> = {
        let data = state.store.read();
        data.settings
            .iter()
            .map(|setting| (setting.key.clone(), setting.value.clone()))
            .collect()
    };

    state.store.mutate(|data| {
        let now = Utc::now();
        let mut after = HashMap::new();
        for (key, value) in &validated {
            if let Some(setting) = data.settings.iter_mut().find(|setting| setting.key == *key) {
                after.insert(setting.key.clone(), setting.value.clone());
                setting.value = value.clone();
                setting.updated_at = now;
                setting.updated_by = Some(actor_id);
            } else {
                data.settings.push(crate::store::SettingRow {
                    key: key.clone(),
                    value: value.clone(),
                    updated_at: now,
                    updated_by: Some(actor_id),
                });
                after.insert(key.clone(), Value::Null);
            }
        }

        data.bump_revision();
        data.audit(
            Some(actor_id),
            Some(actor_email.clone()),
            "settings.update",
            "settings",
            None,
            Some(serde_json::to_value(before).unwrap_or_default()),
            Some(serde_json::to_value(after).unwrap_or_default()),
        );
        Ok(())
    })?;

    list(State(state), TrustedUser(actor.clone())).await
}
