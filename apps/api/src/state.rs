use std::sync::Arc;

use crate::config::Config;
use crate::store::Store;

/// Shared handler state. There is no database pool: the CSV `Store` is the single source
/// of truth and is shared behind an `Arc`.
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub store: Arc<Store>,
}

impl AppState {
    pub fn new(config: Config, store: Arc<Store>) -> Self {
        Self {
            config: Arc::new(config),
            store,
        }
    }

    /// Mail domain used to turn a bare Windows username into an email address. The
    /// `mail_domain` setting in settings.csv overrides the environment/config value.
    pub fn mail_domain(&self) -> String {
        let data = self.store.read();
        data.setting("mail_domain")
            .and_then(|value| value.as_str().map(str::to_string))
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| self.config.mail_domain.clone())
    }
}
