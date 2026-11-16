use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::domain::reset::{self, ResetRequestRow};
use crate::domain::users::IssuedCode;
use crate::error::{ApiError, ApiResult};
use crate::http::admin::{pool_of, SuperAdmin};
use crate::http::extract::ClientIp;
use crate::state::AppState;

#[derive(Debug, Default, Deserialize)]
pub struct ResetQuery {
    pub status: Option<String>,
}

pub async fn list(
    State(state): State<AppState>,
    SuperAdmin(_session): SuperAdmin,
    Query(params): Query<ResetQuery>,
) -> ApiResult<Json<Vec<ResetRequestRow>>> {
    let pool = pool_of(&state)?;

    if let Some(status) = params.status.as_deref() {
        if !matches!(status, "open" | "issued" | "dismissed") {
            return Err(ApiError::bad_request(
                "invalid_status",
                "Status must be open, issued or dismissed.",
            ));
        }
    }

    Ok(Json(reset::list(pool, params.status.as_deref()).await?))
}

pub async fn issue(
    State(state): State<AppState>,
    SuperAdmin(session): SuperAdmin,
    ClientIp(ip): ClientIp,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<IssuedCode>> {
    let pool = pool_of(&state)?;

    let code = reset::issue(
        pool,
        session.user.id,
        id,
        ip.map(|a| a.to_string()).as_deref(),
    )
    .await?;

    Ok(Json(code))
}

pub async fn dismiss(
    State(state): State<AppState>,
    SuperAdmin(session): SuperAdmin,
    ClientIp(ip): ClientIp,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    let pool = pool_of(&state)?;

    reset::dismiss(
        pool,
        session.user.id,
        id,
        ip.map(|a| a.to_string()).as_deref(),
    )
    .await?;

    Ok(StatusCode::NO_CONTENT)
}
