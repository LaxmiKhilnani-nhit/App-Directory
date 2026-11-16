use std::sync::Arc;

use sqlx::PgPool;

use crate::config::Config;

/// Shared handler state. The pool is optional so the API can run without a database
/// (Phase 0 health/static work, and local frontend development).
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub pool: Option<PgPool>,
}

impl AppState {
    pub fn new(config: Config, pool: Option<PgPool>) -> Self {
        Self {
            config: Arc::new(config),
            pool,
        }
    }

    pub fn pool(&self) -> Result<&PgPool, AppStateError> {
        self.pool.as_ref().ok_or(AppStateError::NoDatabase)
    }
}

#[derive(Debug)]
pub enum AppStateError {
    NoDatabase,
}

impl std::fmt::Display for AppStateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppStateError::NoDatabase => write!(formatter, "database is not configured"),
        }
    }
}

impl std::error::Error for AppStateError {}
