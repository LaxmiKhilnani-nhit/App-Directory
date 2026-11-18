use axum::{
    body::{Body, Bytes},
    extract::{Path, State},
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use uuid::Uuid;

use crate::domain::icon;
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

/// Serves a stored icon with a weak ETag so clients can revalidate instead of
/// re-downloading.
pub async fn get(
    State(state): State<AppState>,
    Path(app_id): Path<Uuid>,
    headers: axum::http::HeaderMap,
) -> ApiResult<Response> {
    let data = state.store.read();
    let app = data
        .live_app_by_id(app_id)
        .filter(|app| app.is_enabled)
        .ok_or_else(|| ApiError::not_found("icon_not_found", "No icon for that app."))?;

    let Some(bytes) = state.store.read_icon(app_id) else {
        return Err(ApiError::not_found("icon_not_found", "No icon for that app."));
    };

    if let Some(etag) = app.icon_etag.as_deref() {
        if icon::etag_matches(&headers, etag) {
            let mut response = StatusCode::NOT_MODIFIED.into_response();
            if let Ok(value) = HeaderValue::from_str(etag) {
                response.headers_mut().insert(header::ETAG, value);
            }
            return Ok(response);
        }
    }

    let mime = app
        .icon_mime
        .clone()
        .unwrap_or_else(|| "application/octet-stream".to_string());

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
    if let Some(etag) = app.icon_etag.as_deref() {
        if let Ok(value) = HeaderValue::from_str(etag) {
            response_headers.insert(header::ETAG, value);
        }
    }

    Ok(response)
}
