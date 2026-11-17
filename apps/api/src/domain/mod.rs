pub mod apps;
pub mod audit;
pub mod auth;
pub mod de;
pub mod email;
pub mod entitlements;
pub mod groups;
pub mod icon_fetch;
pub mod models;
pub mod password;
pub mod reset;
pub mod revision;
pub mod seed;
pub mod settings;
pub mod stats;
pub mod users;

/// PostgreSQL unique-violation SQLSTATE. Mapped to a 409 instead of a 500 when an admin
/// reuses a name or email that already exists.
pub(crate) fn is_unique_violation(error: &sqlx::Error) -> bool {
    matches!(
        error,
        sqlx::Error::Database(database)
            if database.code().as_deref() == Some("23505")
    )
}
