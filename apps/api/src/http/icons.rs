use axum::{
    body::{Body, Bytes},
    extract::{Path, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use uuid::Uuid;

use crate::domain::entitlements;
use crate::error::{ApiError, ApiResult};
use crate::http::admin::pool_of;
use crate::http::extract::AuthUser;
use crate::state::AppState;

/// Serves the stored icon with a strong-ish ETag so the client can revalidate instead of
/// re-downloading. Requires a session, and a normal user must be entitled to the app.
pub async fn get(
    State(state): State<AppState>,
    AuthUser(session): AuthUser,
    Path(app_id): Path<Uuid>,
    headers: HeaderMap,
) -> ApiResult<Response> {
    let pool = pool_of(&state)?;

    let row: Option<(Option<Vec<u8>>, Option<String>, Option<String>)> = sqlx::query_as(
        "select icon_data, icon_mime, icon_etag
           from apps
          where id = $1 and deleted_at is null and is_enabled",
    )
    .bind(app_id)
    .fetch_optional(pool)
    .await?;

    let Some((data, mime, etag)) = row else {
        return Err(ApiError::not_found("icon_not_found", "No icon for that app."));
    };

    if session.user.role == "user"
        && !entitlements::is_entitled(pool, session.user.id, app_id).await?
    {
        return Err(ApiError::forbidden(
            "not_entitled",
            "You do not have access to that app.",
        ));
    }

    let Some(bytes) = data else {
        return Err(ApiError::not_found("icon_not_found", "No icon for that app."));
    };

    if let Some(etag) = etag.as_deref() {
        let matches = headers
            .get(header::IF_NONE_MATCH)
            .and_then(|value| value.to_str().ok())
            .map(str::trim)
            .is_some_and(|candidate| candidate == etag || candidate == etag.trim_start_matches("W/"));

        if matches {
            let mut response = StatusCode::NOT_MODIFIED.into_response();
            if let Ok(value) = HeaderValue::from_str(etag) {
                response.headers_mut().insert(header::ETAG, value);
            }
            return Ok(response);
        }
    }

    let mime = mime.unwrap_or_else(|| "application/octet-stream".to_string());

    let mut response = Response::new(Body::from(Bytes::from(bytes)));
    let response_headers = response.headers_mut();
    response_headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&mime)
            .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream")),
    );
    response_headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, max-age=86400"),
    );
    if let Some(etag) = etag {
        if let Ok(value) = HeaderValue::from_str(&etag) {
            response_headers.insert(header::ETAG, value);
        }
    }

    Ok(response)
}
