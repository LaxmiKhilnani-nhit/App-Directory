use serde_json::{json, Value};

use crate::error::ApiError;

/// CSV fields must not contain raw line breaks. Everything entering the store is sanitised.
pub fn sanitize(value: Option<String>) -> Option<String> {
    value.map(|text| {
        text.trim()
            .replace(['\r', '\n'], " ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    })
}

pub fn clean(value: Option<String>) -> Option<String> {
    sanitize(value).filter(|text| !text.is_empty())
}

pub fn validate_name(name: &str) -> Result<String, ApiError> {
    let trimmed = sanitize(Some(name.to_string())).unwrap_or_default();
    if trimmed.is_empty() {
        return Err(ApiError::bad_request("invalid_name", "Name is required."));
    }
    if trimmed.chars().count() > 120 {
        return Err(ApiError::bad_request(
            "invalid_name",
            "Name must be 120 characters or fewer.",
        ));
    }
    Ok(trimmed)
}

pub fn validate_email(email: &str) -> Result<String, ApiError> {
    let trimmed = email.trim().to_lowercase();
    let valid = trimmed.len() >= 3
        && trimmed.len() <= 254
        && trimmed.contains('@')
        && !trimmed.contains(char::is_whitespace)
        && !trimmed.starts_with('@')
        && !trimmed.ends_with('@');

    if !valid {
        return Err(ApiError::bad_request(
            "invalid_email",
            "That email address is not valid.",
        ));
    }
    Ok(trimmed)
}

pub fn validate_url(url: &str) -> Result<String, ApiError> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Err(ApiError::bad_request("invalid_url", "URL is required."));
    }
    if trimmed.len() > 500 {
        return Err(ApiError::bad_request(
            "invalid_url",
            "URL must be 500 characters or fewer.",
        ));
    }

    let parsed = reqwest::Url::parse(trimmed)
        .map_err(|_| ApiError::bad_request("invalid_url", "URL is not valid."))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(ApiError::bad_request(
            "invalid_url",
            "URL must start with http:// or https://.",
        ));
    }
    if parsed.host_str().is_none() {
        return Err(ApiError::bad_request(
            "invalid_url",
            "URL is missing a host.",
        ));
    }
    Ok(trimmed.to_string())
}

pub fn validate_color(color: &str) -> Result<String, ApiError> {
    let trimmed = color.trim();
    let valid = trimmed.len() == 7
        && trimmed.starts_with('#')
        && trimmed[1..]
            .chars()
            .all(|character| character.is_ascii_hexdigit());
    if !valid {
        return Err(ApiError::bad_request(
            "invalid_color",
            "Colour must look like #2F6FED.",
        ));
    }
    Ok(trimmed.to_ascii_uppercase())
}

pub fn validate_category(category: Option<String>) -> Result<Option<String>, ApiError> {
    let cleaned = clean(category);
    if let Some(value) = cleaned.as_deref() {
        if value.chars().count() > 80 {
            return Err(ApiError::bad_request(
                "invalid_category",
                "Category must be 80 characters or fewer.",
            ));
        }
    }
    Ok(cleaned)
}

pub fn validate_description(description: Option<String>) -> Result<Option<String>, ApiError> {
    let cleaned = clean(description);
    if let Some(value) = cleaned.as_deref() {
        if value.chars().count() > 500 {
            return Err(ApiError::bad_request(
                "invalid_description",
                "Description must be 500 characters or fewer.",
            ));
        }
    }
    Ok(cleaned)
}

pub fn validate_department(value: Option<String>) -> Result<Option<String>, ApiError> {
    let cleaned = clean(value);
    if let Some(department) = cleaned.as_deref() {
        if department.chars().count() > 120 {
            return Err(ApiError::bad_request(
                "invalid_department",
                "Department must be 120 characters or fewer.",
            ));
        }
    }
    Ok(cleaned)
}

pub fn validate_sort_order(sort_order: i32) -> Result<i32, ApiError> {
    if !(-10_000..=10_000).contains(&sort_order) {
        return Err(ApiError::bad_request(
            "invalid_sort_order",
            "Sort order must be between -10000 and 10000.",
        ));
    }
    Ok(sort_order)
}

pub fn is_role(role: &str) -> bool {
    matches!(role, "user" | "app_admin" | "super_admin")
}

pub fn validate_role(role: &str) -> Result<String, ApiError> {
    if !is_role(role) {
        return Err(ApiError::bad_request(
            "invalid_role",
            "Role must be user, app_admin or super_admin.",
        ));
    }
    Ok(role.to_string())
}

/// Normalises an `app_admin` scope to `{"categories": [...]}`.
pub fn validate_scope(value: Value) -> Result<Value, ApiError> {
    let object = value.as_object().ok_or_else(|| {
        ApiError::bad_request(
            "invalid_scope",
            "adminScope must be an object with a categories array.",
        )
    })?;

    let raw = match object.get("categories") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(values)) => values.clone(),
        Some(_) => {
            return Err(ApiError::bad_request(
                "invalid_scope",
                "adminScope.categories must be an array of category names.",
            ))
        }
    };

    if raw.len() > 50 {
        return Err(ApiError::bad_request(
            "invalid_scope",
            "An app admin can be limited to at most 50 categories.",
        ));
    }

    let mut categories: Vec<String> = Vec::with_capacity(raw.len());
    for entry in raw {
        let category = entry.as_str().ok_or_else(|| {
            ApiError::bad_request(
                "invalid_scope",
                "adminScope.categories must contain only strings.",
            )
        })?;
        let category = category.trim();
        if category.is_empty() {
            return Err(ApiError::bad_request(
                "invalid_scope",
                "A category in adminScope cannot be blank.",
            ));
        }
        if category.chars().count() > 80 {
            return Err(ApiError::bad_request(
                "invalid_scope",
                "A category name must be 80 characters or fewer.",
            ));
        }
        if !categories
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(category))
        {
            categories.push(category.to_string());
        }
    }

    Ok(json!({ "categories": categories }))
}

pub fn scope_for(role: &str, value: Option<Value>) -> Result<Option<Value>, ApiError> {
    if role != "app_admin" {
        return Ok(None);
    }
    match value {
        Some(value) => validate_scope(value).map(Some),
        None => Ok(Some(json!({ "categories": [] }))),
    }
}

/// Minimal RFC-4180 reader used by the bulk user import endpoint.
pub fn split_csv_line(line: &str) -> Vec<String> {
    split_csv_line_delimited(line, ',')
}

/// Splits one CSV line using the given separator (Excel sometimes writes `;`).
pub fn split_csv_line_delimited(line: &str, delimiter: char) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut characters = line.trim_end_matches('\r').chars().peekable();

    while let Some(character) = characters.next() {
        match character {
            '"' if quoted && characters.peek() == Some(&'"') => {
                current.push('"');
                characters.next();
            }
            '"' => quoted = !quoted,
            character if character == delimiter && !quoted => {
                fields.push(current.trim().to_string());
                current.clear();
            }
            other => current.push(other),
        }
    }
    fields.push(current.trim().to_string());
    fields
}
