pub mod apps;
pub mod audit_log;
pub mod groups;
pub mod reset;
pub mod settings;
pub mod stats;
pub mod users;

use axum::{
    extract::FromRequestParts,
    http::request::Parts,
    routing::{get, post, put},
    Json, Router,
};
use serde::Serialize;

use crate::domain::apps::AdminApp;
use crate::domain::auth::{AuthSession, SessionUser};
use crate::error::ApiError;
use crate::http::extract::AuthUser;
use crate::state::AppState;

/// Every admin endpoint sits behind this. It is the normal bearer-token check plus two
/// policy gates: the account must have finished its first-login password change, and it
/// must hold an admin role.
pub struct AdminUser(pub AuthSession);

impl FromRequestParts<AppState> for AdminUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let AuthUser(session) = AuthUser::from_request_parts(parts, state).await?;

        if session.user.must_change_password {
            return Err(ApiError::forbidden(
                "password_change_required",
                "Set a new password before using the admin portal.",
            ));
        }

        if !matches!(session.user.role.as_str(), "app_admin" | "super_admin") {
            return Err(ApiError::forbidden(
                "forbidden",
                "Administrator access is required.",
            ));
        }

        Ok(AdminUser(session))
    }
}

/// Stricter gate for account, group, settings and reset-queue management. An `app_admin`
/// may manage apps and grants in their scope, but not people or global configuration.
pub struct SuperAdmin(pub AuthSession);

impl FromRequestParts<AppState> for SuperAdmin {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let AdminUser(session) = AdminUser::from_request_parts(parts, state).await?;

        if session.user.role != "super_admin" {
            return Err(ApiError::forbidden(
                "forbidden",
                "Only a super admin can do that.",
            ));
        }

        Ok(SuperAdmin(session))
    }
}

/// Category scope for an `app_admin`. `admin_scope` is `{ "categories": [...] }`; a
/// super admin ignores it.
pub struct Scope<'a> {
    user: &'a SessionUser,
}

impl<'a> Scope<'a> {
    pub fn new(user: &'a SessionUser) -> Self {
        Self { user }
    }

    pub fn is_super(&self) -> bool {
        self.user.role == "super_admin"
    }

    fn categories(&self) -> Vec<String> {
        self.user
            .admin_scope
            .as_ref()
            .and_then(|scope| scope.get("categories"))
            .and_then(|categories| categories.as_array())
            .map(|categories| {
                categories
                    .iter()
                    .filter_map(|category| category.as_str())
                    .map(|category| category.trim().to_lowercase())
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn allows_category(&self, category: Option<&str>) -> bool {
        if self.is_super() {
            return true;
        }

        match category {
            Some(category) => self
                .categories()
                .iter()
                .any(|allowed| allowed == &category.trim().to_lowercase()),
            None => false,
        }
    }

    pub fn require_category(&self, category: Option<&str>) -> Result<(), ApiError> {
        if self.allows_category(category) {
            Ok(())
        } else {
            Err(ApiError::forbidden(
                "out_of_scope",
                "That app is outside your assigned categories.",
            ))
        }
    }

    pub fn require_app(&self, app: &AdminApp) -> Result<(), ApiError> {
        self.require_category(app.category.as_deref())
    }

    pub fn filter(&self, apps: Vec<AdminApp>) -> Vec<AdminApp> {
        if self.is_super() {
            return apps;
        }

        apps.into_iter()
            .filter(|app| self.allows_category(app.category.as_deref()))
            .collect()
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/admin/me", get(me))
        .route("/api/v1/admin/stats", get(stats::get))
        .route(
            "/api/v1/admin/apps",
            get(apps::list).post(apps::create),
        )
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
        .route(
            "/api/v1/admin/users",
            get(users::list).post(users::create),
        )
        .route("/api/v1/admin/users/import", post(users::import))
        .route(
            "/api/v1/admin/users/{id}",
            get(users::get).patch(users::update).delete(users::delete),
        )
        .route("/api/v1/admin/users/{id}/apps", put(users::set_apps))
        .route(
            "/api/v1/admin/users/{id}/reset-code",
            post(users::reset_code),
        )
        .route("/api/v1/admin/reset-requests", get(reset::list))
        .route(
            "/api/v1/admin/reset-requests/{id}/issue",
            post(reset::issue),
        )
        .route(
            "/api/v1/admin/reset-requests/{id}/dismiss",
            post(reset::dismiss),
        )
        .route(
            "/api/v1/admin/settings",
            get(settings::list).put(settings::update),
        )
        .route("/api/v1/admin/audit", get(audit_log::list))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Me {
    user: SessionUser,
    is_super_admin: bool,
}

async fn me(AdminUser(session): AdminUser) -> Json<Me> {
    Json(Me {
        is_super_admin: session.user.role == "super_admin",
        user: session.user,
    })
}

/// Shared "no database configured" mapping for the handlers below.
pub(crate) fn pool_of(state: &AppState) -> Result<&sqlx::PgPool, ApiError> {
    state
        .pool
        .as_ref()
        .ok_or_else(|| ApiError::internal("Database is not configured."))
}
