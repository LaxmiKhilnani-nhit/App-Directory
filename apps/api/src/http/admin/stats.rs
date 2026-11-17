use axum::{extract::State, Json};
use sqlx::PgPool;

use crate::domain::stats::{self, Stats};
use crate::error::ApiResult;
use crate::http::admin::{pool_of, AdminUser};
use crate::state::AppState;

pub async fn get(State(state): State<AppState>, AdminUser(_session): AdminUser) -> ApiResult<Json<Stats>> {
    let pool: &PgPool = pool_of(&state)?;
    Ok(Json(stats::get(pool).await?))
}
