use std::path::PathBuf;

/// Runtime configuration, all sourced from the environment so the same image works on
/// the VM, a staging box, and a developer machine.
#[derive(Clone, Debug)]
pub struct Config {
    pub bind_addr: String,
    pub port: u16,
    pub app_name: String,
    pub admin_dist: PathBuf,
    pub database_url: Option<String>,
    pub session_ttl_days: i64,
    pub bootstrap_admin_email: Option<String>,
    pub bootstrap_admin_password: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1".to_string(),
            port: 8090,
            app_name: "App Directory".to_string(),
            admin_dist: PathBuf::from("admin-dist"),
            database_url: None,
            session_ttl_days: 30,
            bootstrap_admin_email: None,
            bootstrap_admin_password: None,
        }
    }
}

impl Config {
    pub fn from_env() -> Self {
        let defaults = Self::default();

        Self {
            bind_addr: std::env::var("BIND_ADDR").unwrap_or(defaults.bind_addr),
            port: std::env::var("PORT")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(defaults.port),
            app_name: std::env::var("APP_NAME").unwrap_or(defaults.app_name),
            admin_dist: resolve_admin_dist(),
            database_url: non_empty_var("DATABASE_URL"),
            session_ttl_days: std::env::var("SESSION_TTL_DAYS")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(defaults.session_ttl_days),
            bootstrap_admin_email: non_empty_var("BOOTSTRAP_ADMIN_EMAIL"),
            bootstrap_admin_password: non_empty_var("BOOTSTRAP_ADMIN_PASSWORD"),
        }
    }
}

fn non_empty_var(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|value| !value.trim().is_empty())
}

/// The admin SPA bundle is copied into the image at `/app/admin-dist`; during local
/// development it lives at `apps/admin/dist` relative to the crate.
fn resolve_admin_dist() -> PathBuf {
    if let Ok(explicit) = std::env::var("ADMIN_DIST") {
        return PathBuf::from(explicit);
    }

    for candidate in ["admin-dist", "../admin/dist", "../../admin/dist"] {
        let path = PathBuf::from(candidate);
        if path.join("index.html").exists() {
            return path;
        }
    }

    PathBuf::from("admin-dist")
}
