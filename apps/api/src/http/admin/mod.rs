pub mod apps;
pub mod audit;
pub mod departments;
pub mod groups;
pub mod phone_directory;
pub mod settings;
pub mod stats;
pub mod users;

use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    routing::{get, post, put},
    Json, Router,
};
use serde_json::json;

use crate::domain::models::AdminMe;
use crate::http::identity::{self, TrustedUser};
use crate::state::AppState;

/// The admin portal is for administrators only. A plain `user` account may sign in to the
/// desktop client but gets a 403 from every /admin endpoint.
async fn require_admin(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let (parts, body) = request.into_parts();
    let query_identity = identity::query_value(&parts, "identity");
    let allowed = identity::resolve(&parts, query_identity.as_deref(), &state)
        .as_deref()
        .and_then(|raw| {
            let email = identity::normalize(raw, &state.mail_domain());
            crate::domain::admins::find(&state, &email)
        })
        .is_some();

    if !allowed {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({
                "error": { "code": "admin_only", "message": "Administrator access is required." }
            })),
        )
            .into_response();
    }

    next.run(Request::from_parts(parts, body)).await
}

/// All admin endpoints are behind the trusted-identity gate. There are no roles to enforce
/// any more: `/admin/me` always reports a super admin so the SPA's route guards pass.
pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/api/v1/admin/me", get(me))
        .route("/api/v1/admin/stats", get(stats::get))
        .route(
            "/api/v1/admin/departments",
            get(departments::list).post(departments::create),
        )
        .route(
            "/api/v1/admin/departments/{id}",
            axum::routing::patch(departments::update).delete(departments::delete),
        )
        .route(
            "/api/v1/admin/departments/{id}/apps",
            get(departments::get_apps).put(departments::set_apps),
        )
        .route("/api/v1/admin/apps", get(apps::list).post(apps::create))
        .route(
            "/api/v1/admin/apps/{id}",
            get(apps::get).patch(apps::update).delete(apps::delete),
        )
        .route(
            "/api/v1/admin/apps/{id}/departments",
            put(apps::set_departments),
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
        .route(
            "/api/v1/admin/phone-directory",
            get(phone_directory::list).post(phone_directory::create),
        )
        .route(
            "/api/v1/admin/phone-directory/import",
            post(phone_directory::import),
        )
        .route(
            "/api/v1/admin/phone-directory/{id}",
            axum::routing::patch(phone_directory::update).delete(phone_directory::delete),
        )
        .route("/api/v1/admin/users", get(users::list).post(users::create))
        .route("/api/v1/admin/users/import", post(users::import))
        .route(
            "/api/v1/admin/users/{id}",
            get(users::get).patch(users::update).delete(users::delete),
        )
        .route("/api/v1/admin/users/{id}/apps", put(users::set_apps))
        .route(
            "/api/v1/admin/users/{id}/purge",
            axum::routing::delete(users::purge),
        )
        .route(
            "/api/v1/admin/settings",
            get(settings::list).put(settings::update),
        )
        .route("/api/v1/admin/audit", get(audit::list))
        .layer(axum::middleware::from_fn_with_state(state, require_admin))
}

async fn me(TrustedUser(user): TrustedUser) -> Json<AdminMe> {
    Json(AdminMe {
        is_super_admin: user.role == "super_admin",
        user,
    })
}
