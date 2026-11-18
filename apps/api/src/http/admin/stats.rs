use axum::{extract::State, Json};
use chrono::Utc;

use crate::domain::models::Stats;
use crate::error::ApiResult;
use crate::http::identity::TrustedUser;
use crate::state::AppState;

pub async fn get(
    State(state): State<AppState>,
    TrustedUser(_actor): TrustedUser,
) -> ApiResult<Json<Stats>> {
    let data = state.store.read();
    let cutoff = Utc::now() - chrono::Duration::hours(24);

    Ok(Json(Stats {
        users: data.users.len() as i64,
        active_users: data.users.iter().filter(|user| user.is_active).count() as i64,
        apps: data.apps.iter().filter(|app| app.deleted_at.is_none()).count() as i64,
        enabled_apps: data
            .apps
            .iter()
            .filter(|app| app.deleted_at.is_none() && app.is_enabled)
            .count() as i64,
        groups: data.groups.len() as i64,
        // Reset requests are no longer generated; the queue is always empty.
        open_reset_requests: 0,
        logins_today: data
            .users
            .iter()
            .filter(|user| user.last_login_at.is_some_and(|stamp| stamp >= cutoff))
            .count() as i64,
        revision: data.revision,
    }))
}
