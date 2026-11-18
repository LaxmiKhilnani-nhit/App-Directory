use std::path::PathBuf;

/// Runtime configuration, all sourced from the environment so the same image works on the
/// VM, a staging box, and a developer machine.
///
/// There is deliberately no database configuration: all state lives in CSV files under
/// `data_dir`, and identity is trusted from a Windows username header.
#[derive(Clone, Debug)]
pub struct Config {
    pub bind_addr: String,
    pub port: u16,
    pub app_name: String,
    pub admin_dist: PathBuf,
    /// Directory holding users.csv, groups.csv, apps.csv, settings.csv, audit.csv,
    /// revision.csv and the icons/ sub-directory.
    pub data_dir: PathBuf,
    /// Where the portable Windows client is mounted (see download.rs / CLIENT_EXE).
    pub client_exe: Option<PathBuf>,
    /// Domain appended to a bare Windows username to form an email address. The
    /// `mail_domain` setting in settings.csv wins over this value at runtime.
    pub mail_domain: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1".to_string(),
            port: 8090,
            app_name: "App Directory".to_string(),
            admin_dist: PathBuf::from("admin-dist"),
            data_dir: PathBuf::from("data"),
            client_exe: None,
            mail_domain: "nhit.co.in".to_string(),
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
            data_dir: std::env::var("DATA_DIR")
                .map(PathBuf::from)
                .unwrap_or(defaults.data_dir),
            client_exe: non_empty_var("CLIENT_EXE").map(PathBuf::from),
            mail_domain: std::env::var("MAIL_DOMAIN")
                .ok()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or(defaults.mail_domain),
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
