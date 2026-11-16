use axum::{
    response::{Html, IntoResponse, Redirect},
    routing::get,
    Router,
};
use tower_http::services::{ServeDir, ServeFile};

use crate::config::Config;

const NOT_BUILT_PAGE: &str = r#"<!doctype html>
<html lang="en"><head><meta charset="utf-8"><title>App Directory — Admin</title>
<style>body{font-family:Segoe UI,system-ui,sans-serif;display:grid;place-items:center;height:100vh;margin:0;background:#f6f7f9;color:#131720}
code{background:#eef1f5;padding:2px 6px;border-radius:4px}</style></head>
<body><main style="text-align:center;max-width:32rem">
<h1>Admin portal not built</h1>
<p>The API is running, but no admin bundle was found at <code>admin-dist/index.html</code>.</p>
<p>Build it with <code>pnpm build:admin</code> (or <code>docker compose build</code>).</p>
</main></body></html>"#;

/// Serves the admin SPA under `/admin` with history fallback, or a friendly placeholder
/// when the bundle has not been built yet.
pub fn router(config: &Config) -> Router {
    let index = config.admin_dist.join("index.html");

    if !index.exists() {
        tracing::warn!(
            admin_dist = %config.admin_dist.display(),
            "admin bundle missing; serving placeholder at /admin"
        );
        return Router::new()
            .route("/admin", get(placeholder))
            .route("/admin/", get(placeholder))
            .route("/admin/{*path}", get(placeholder))
            .route("/", get(|| async { Redirect::temporary("/admin") }));
    }

    // `fallback` (not `not_found_service`) keeps the 200 status for deep links such as
    // /admin/users, which the SPA router resolves client-side.
    let serve = ServeDir::new(&config.admin_dist).fallback(ServeFile::new(index));

    Router::new()
        .nest_service("/admin", serve)
        .route("/", get(|| async { Redirect::temporary("/admin") }))
}

async fn placeholder() -> impl IntoResponse {
    Html(NOT_BUILT_PAGE)
}
