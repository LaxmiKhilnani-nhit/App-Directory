pub mod apps;
pub mod audit;
pub mod groups;
pub mod settings;
pub mod stats;
pub mod users;

use axum::{
    routing::{get, post, put},
    Json, Router,
};

use crate::domain::models::AdminMe;
use crate::http::identity::TrustedUser;
use crate::state::AppState;

/// All admin endpoints are behind the trusted-identity gate. There are no roles to enforce
/// any more: `/admin/me` always reports a super admin so the SPA's route guards pass.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/admin/me", get(me))
        .route("/api/v1/admin/stats", get(stats::get))
        .route("/api/v1/admin/apps", get(apps::list).post(apps::create))
        .route(
            "/api/v1/admin/apps/{id}",
            get(apps::get).patch(apps::update).delete(apps::delete),
        )
        .route("/api/v1/admin/apps/{id}/icon", post(apps::upload_icon))
        .route(
            "/api/v1/admin/apps/{id}/refetch-icon",
            post(apps::refetch_icon),
        )
        .route("/api/v1/admin/apps/{id}/groups", put(apps::set_groups))
        .route("/api/v1/admin/apps/{id}/users", put(apps::set_users))
        .route(
            "/api/v1/admin/groups",
            get(groups::list).post(groups::create),
        )
        .route(
            "/api/v1/admin/groups/{id}",
            get(groups::get)
                .patch(groups::update)
                .delete(groups::delete),
        )
        .route(
            "/api/v1/admin/groups/{id}/members",
            put(groups::set_members),
        )
        .route("/api/v1/admin/users", get(users::list).post(users::create))
        .route("/api/v1/admin/users/import", post(users::import))
        .route(
            "/api/v1/admin/users/{id}",
            get(users::get).patch(users::update).delete(users::delete),
        )
        .route("/api/v1/admin/users/{id}/apps", put(users::set_apps))
        .route(
            "/api/v1/admin/settings",
            get(settings::list).put(settings::update),
        )
        .route("/api/v1/admin/audit", get(audit::list))
}

async fn me(TrustedUser(user): TrustedUser) -> Json<AdminMe> {
    Json(AdminMe {
        is_super_admin: true,
        user,
    })
}
