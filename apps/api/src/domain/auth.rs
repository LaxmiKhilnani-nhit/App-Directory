use std::sync::OnceLock;

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use rand::RngCore;
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::domain::password;
use crate::error::ApiError;

/// How long an admin-issued reset code stays valid.
pub const RESET_CODE_TTL_HOURS: i64 = 72;

/// `last_seen_at` is only rewritten once per minute; a write per request would be pure
/// noise on a 400-user install.
const LAST_SEEN_TOUCH_SECONDS: i64 = 60;

/// No `I`, `O`, `0` or `1`: codes get read out over the phone and typed by hand.
const RESET_ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionUser {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    pub department: Option<String>,
    pub role: String,
    pub must_change_password: bool,
}

#[derive(Debug, Clone)]
pub struct AuthSession {
    pub user: SessionUser,
    pub session_id: Uuid,
}

#[derive(Debug, Clone, Copy)]
pub struct LockoutPolicy {
    pub attempts: i64,
    pub minutes: i64,
}

#[derive(Debug)]
pub enum AuthError {
    InvalidCredentials,
    WrongCurrentPassword,
    Locked { until: DateTime<Utc> },
    Inactive,
    InvalidResetCode,
    PasswordTooShort { min: usize },
    PasswordUnchanged,
    Unauthorized,
    Internal(String),
}

impl From<sqlx::Error> for AuthError {
    fn from(error: sqlx::Error) -> Self {
        AuthError::Internal(format!("database error: {error}"))
    }
}

impl From<anyhow::Error> for AuthError {
    fn from(error: anyhow::Error) -> Self {
        AuthError::Internal(error.to_string())
    }
}

impl From<AuthError> for ApiError {
    fn from(error: AuthError) -> Self {
        match error {
            AuthError::InvalidCredentials => {
                ApiError::new(axum::http::StatusCode::UNAUTHORIZED, "invalid_credentials", "Email or password is incorrect.")
            }
            AuthError::WrongCurrentPassword => {
                ApiError::bad_request("invalid_password", "Current password is incorrect.")
            }
            AuthError::Locked { until } => {
                let minutes = (until - Utc::now()).num_minutes().max(1);
                ApiError::new(
                    axum::http::StatusCode::LOCKED,
                    "account_locked",
                    format!("Too many failed attempts. Try again in {minutes} minute(s)."),
                )
            }
            AuthError::Inactive => {
                ApiError::forbidden("account_inactive", "This account has been deactivated.")
            }
            AuthError::InvalidResetCode => {
                ApiError::bad_request("invalid_reset_code", "That reset code is not valid or has expired.")
            }
            AuthError::PasswordTooShort { min } => ApiError::bad_request(
                "password_too_short",
                format!("Password must be at least {min} characters."),
            ),
            AuthError::PasswordUnchanged => {
                ApiError::bad_request("password_unchanged", "Choose a password you have not used here before.")
            }
            AuthError::Unauthorized => ApiError::unauthorized("Sign in required."),
            AuthError::Internal(message) => {
                tracing::error!(error = %message, "auth internal error");
                ApiError::internal("Something went wrong. Please try again.")
            }
        }
    }
}

pub type AuthResult<T> = Result<T, AuthError>;

// ---------------------------------------------------------------- tokens

/// 32 random bytes, hex-encoded. Opaque to the client and never stored in plaintext.
pub fn generate_token() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    to_hex(&bytes)
}

/// Session tokens are looked up by hash, so a database leak does not hand out sessions.
pub fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    to_hex(&hasher.finalize())
}

fn to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(char::from_digit(u32::from(byte >> 4), 16).unwrap_or('0'));
        out.push(char::from_digit(u32::from(byte & 0x0f), 16).unwrap_or('0'));
    }
    out
}

/// Returns `(display, canonical)`. The dash is cosmetic — users read it aloud — while the
/// canonical form is what gets hashed and compared.
pub fn generate_reset_code() -> (String, String) {
    let mut bytes = [0u8; 8];
    rand::thread_rng().fill_bytes(&mut bytes);

    let canonical: String = bytes
        .iter()
        .map(|byte| char::from(RESET_ALPHABET[usize::from(*byte) % RESET_ALPHABET.len()]))
        .collect();

    let display = format!("{}-{}", &canonical[..4], &canonical[4..]);
    (display, canonical)
}

/// Accepts `abcd-efgh`, `ABCD EFGH` or `ABCDEFGH`.
fn canonical_code(raw: &str) -> String {
    raw.chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .map(|character| character.to_ascii_uppercase())
        .collect()
}

/// A real hash so a missing account costs roughly the same time as a wrong password.
fn dummy_hash() -> &'static str {
    static DUMMY: OnceLock<String> = OnceLock::new();
    DUMMY.get_or_init(|| {
        password::hash_password("timing-equaliser-not-a-real-password")
            .unwrap_or_else(|_| String::from("$argon2id$v=19$m=19456,t=2,p=1$c29tZXNhbHQ$ZHVtbXk"))
    })
}

// ---------------------------------------------------------------- sessions

#[derive(FromRow)]
struct LoginRow {
    id: Uuid,
    password_hash: Option<String>,
    is_active: bool,
    failed_attempts: i32,
    locked_until: Option<DateTime<Utc>>,
}

#[derive(FromRow)]
struct UserRow {
    id: Uuid,
    email: String,
    name: String,
    department: Option<String>,
    role: String,
    must_change_password: bool,
}

impl From<UserRow> for SessionUser {
    fn from(row: UserRow) -> Self {
        Self {
            id: row.id,
            email: row.email,
            name: row.name,
            department: row.department,
            role: row.role,
            must_change_password: row.must_change_password,
        }
    }
}

/// Verifies credentials, applies lockout, then opens a session.
pub async fn login(
    pool: &PgPool,
    email: &str,
    candidate: &str,
    ttl_days: i64,
    policy: LockoutPolicy,
    client_info: Option<&str>,
) -> AuthResult<(String, DateTime<Utc>, SessionUser)> {
    let row = sqlx::query_as::<_, LoginRow>(
        "select id, password_hash, is_active, failed_attempts, locked_until
           from users
          where lower(email) = lower($1)",
    )
    .bind(email.trim())
    .fetch_optional(pool)
    .await?;

    let Some(row) = row else {
        // Burn the same Argon2 work as a real check so response time does not reveal
        // whether the account exists.
        let _ = password::verify_password(dummy_hash(), candidate);
        return Err(AuthError::InvalidCredentials);
    };

    let now = Utc::now();
    if let Some(until) = row.locked_until {
        if until > now {
            return Err(AuthError::Locked { until });
        }
    }

    let Some(stored_hash) = row.password_hash.as_deref() else {
        // Account exists but was never given a password: only a reset code can unlock it.
        return Err(AuthError::InvalidCredentials);
    };

    if !password::verify_password(stored_hash, candidate) {
        let attempts = row.failed_attempts + 1;

        if policy.attempts > 0 && i64::from(attempts) >= policy.attempts {
            let until = now + ChronoDuration::minutes(policy.minutes.max(1));
            sqlx::query("update users set failed_attempts = 0, locked_until = $2 where id = $1")
                .bind(row.id)
                .bind(until)
                .execute(pool)
                .await?;
            return Err(AuthError::Locked { until });
        }

        sqlx::query("update users set failed_attempts = $2 where id = $1")
            .bind(row.id)
            .bind(attempts)
            .execute(pool)
            .await?;
        return Err(AuthError::InvalidCredentials);
    }

    if !row.is_active {
        return Err(AuthError::Inactive);
    }

    sqlx::query(
        "update users
            set failed_attempts = 0, locked_until = null, last_login_at = now()
          where id = $1",
    )
    .bind(row.id)
    .execute(pool)
    .await?;

    let user = sqlx::query_as::<_, UserRow>(
        "select id, email, name, department, role, must_change_password
           from users where id = $1",
    )
    .bind(row.id)
    .fetch_one(pool)
    .await?;

    let (token, expires_at, _) = create_session(pool, row.id, ttl_days, client_info).await?;

    Ok((token, expires_at, user.into()))
}

pub async fn create_session(
    pool: &PgPool,
    user_id: Uuid,
    ttl_days: i64,
    client_info: Option<&str>,
) -> AuthResult<(String, DateTime<Utc>, Uuid)> {
    let token = generate_token();
    let expires_at = Utc::now() + ChronoDuration::days(ttl_days.max(1));

    let session_id: Uuid = sqlx::query_scalar(
        "insert into sessions (user_id, token_hash, expires_at, client_info)
         values ($1, $2, $3, $4)
         returning id",
    )
    .bind(user_id)
    .bind(hash_token(&token))
    .bind(expires_at)
    .bind(client_info)
    .fetch_one(pool)
    .await?;

    Ok((token, expires_at, session_id))
}

#[derive(FromRow)]
struct SessionRow {
    session_id: Uuid,
    expires_at: DateTime<Utc>,
    revoked_at: Option<DateTime<Utc>>,
    last_seen_at: DateTime<Utc>,
    user_id: Uuid,
    email: String,
    name: String,
    department: Option<String>,
    role: String,
    must_change_password: bool,
    is_active: bool,
}

/// Resolves a bearer token to its session and user, or an error the extractor can map to
/// a 401/403.
pub async fn authenticate(pool: &PgPool, token: &str) -> AuthResult<AuthSession> {
    let token = token.trim();
    if token.is_empty() {
        return Err(AuthError::Unauthorized);
    }

    let row = sqlx::query_as::<_, SessionRow>(
        "select s.id           as session_id,
                s.expires_at   as expires_at,
                s.revoked_at   as revoked_at,
                s.last_seen_at as last_seen_at,
                u.id           as user_id,
                u.email        as email,
                u.name         as name,
                u.department   as department,
                u.role         as role,
                u.must_change_password as must_change_password,
                u.is_active    as is_active
           from sessions s
           join users u on u.id = s.user_id
          where s.token_hash = $1",
    )
    .bind(hash_token(token))
    .fetch_optional(pool)
    .await?;

    let Some(row) = row else {
        return Err(AuthError::Unauthorized);
    };

    let now = Utc::now();
    if row.revoked_at.is_some() || row.expires_at <= now {
        return Err(AuthError::Unauthorized);
    }

    if !row.is_active {
        return Err(AuthError::Inactive);
    }

    if now - row.last_seen_at > ChronoDuration::seconds(LAST_SEEN_TOUCH_SECONDS) {
        sqlx::query("update sessions set last_seen_at = now() where id = $1")
            .bind(row.session_id)
            .execute(pool)
            .await?;
    }

    Ok(AuthSession {
        session_id: row.session_id,
        user: SessionUser {
            id: row.user_id,
            email: row.email,
            name: row.name,
            department: row.department,
            role: row.role,
            must_change_password: row.must_change_password,
        },
    })
}

pub async fn revoke_session(pool: &PgPool, session_id: Uuid) -> AuthResult<()> {
    sqlx::query("update sessions set revoked_at = now() where id = $1 and revoked_at is null")
        .bind(session_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Revokes every live session for a user, optionally keeping the one making the request.
pub async fn revoke_all_sessions(pool: &PgPool, user_id: Uuid, keep: Option<Uuid>) -> AuthResult<()> {
    sqlx::query(
        "update sessions
            set revoked_at = now()
          where user_id = $1
            and revoked_at is null
            and ($2::uuid is null or id <> $2)",
    )
    .bind(user_id)
    .bind(keep)
    .execute(pool)
    .await?;

    Ok(())
}

// ---------------------------------------------------------------- passwords

pub async fn change_password(
    pool: &PgPool,
    user_id: Uuid,
    current: &str,
    new_password: &str,
    min_length: usize,
    keep_session: Option<Uuid>,
) -> AuthResult<()> {
    let stored: Option<String> =
        sqlx::query_scalar("select password_hash from users where id = $1")
            .bind(user_id)
            .fetch_optional(pool)
            .await?
            .flatten();

    let Some(stored) = stored else {
        return Err(AuthError::WrongCurrentPassword);
    };

    if !password::verify_password(&stored, current) {
        return Err(AuthError::WrongCurrentPassword);
    }

    if new_password.chars().count() < min_length {
        return Err(AuthError::PasswordTooShort { min: min_length });
    }

    if new_password == current {
        return Err(AuthError::PasswordUnchanged);
    }

    let hash = password::hash_password(new_password)?;

    sqlx::query(
        "update users
            set password_hash = $2,
                must_change_password = false,
                failed_attempts = 0,
                locked_until = null
          where id = $1",
    )
    .bind(user_id)
    .bind(hash)
    .execute(pool)
    .await?;

    // Changing a password signs every other device out, keeping the current session alive
    // so the portal (and the forced first-login change) does not bounce the user to login.
    revoke_all_sessions(pool, user_id, keep_session).await?;

    Ok(())
}

// ---------------------------------------------------------------- reset codes

/// Issues a code and returns it for display. Only the hash is stored.
pub async fn issue_reset_code(
    pool: &PgPool,
    user_id: Uuid,
    purpose: &str,
    created_by: Option<Uuid>,
    ttl_hours: i64,
) -> AuthResult<String> {
    let (display, canonical) = generate_reset_code();
    let code_hash = password::hash_password(&canonical)?;
    let expires_at = Utc::now() + ChronoDuration::hours(ttl_hours.max(1));

    sqlx::query(
        "insert into reset_codes (user_id, code_hash, purpose, expires_at, created_by)
         values ($1, $2, $3, $4, $5)",
    )
    .bind(user_id)
    .bind(code_hash)
    .bind(purpose)
    .bind(expires_at)
    .bind(created_by)
    .execute(pool)
    .await?;

    Ok(display)
}

/// Consumes a reset code and sets a new password. Returns the user it applied to.
pub async fn reset_password(
    pool: &PgPool,
    email: &str,
    code: &str,
    new_password: &str,
    min_length: usize,
) -> AuthResult<(Uuid, String)> {
    let user: Option<(Uuid, String)> = sqlx::query_as(
        "select id, email from users where lower(email) = lower($1)",
    )
    .bind(email.trim())
    .fetch_optional(pool)
    .await?;

    let Some((user_id, stored_email)) = user else {
        return Err(AuthError::InvalidResetCode);
    };

    #[derive(FromRow)]
    struct CodeRow {
        id: Uuid,
        code_hash: String,
    }

    let row = sqlx::query_as::<_, CodeRow>(
        "select id, code_hash
           from reset_codes
          where user_id = $1
            and used_at is null
            and expires_at > now()
          order by created_at desc
          limit 1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    let Some(row) = row else {
        return Err(AuthError::InvalidResetCode);
    };

    if !password::verify_password(&row.code_hash, &canonical_code(code)) {
        return Err(AuthError::InvalidResetCode);
    }

    if new_password.chars().count() < min_length {
        return Err(AuthError::PasswordTooShort { min: min_length });
    }

    let hash = password::hash_password(new_password)?;

    let mut transaction = pool.begin().await?;

    sqlx::query("update reset_codes set used_at = now() where id = $1")
        .bind(row.id)
        .execute(&mut *transaction)
        .await?;

    sqlx::query(
        "update users
            set password_hash = $2,
                must_change_password = false,
                failed_attempts = 0,
                locked_until = null
          where id = $1",
    )
    .bind(user_id)
    .bind(hash)
    .execute(&mut *transaction)
    .await?;

    sqlx::query(
        "update sessions set revoked_at = now() where user_id = $1 and revoked_at is null",
    )
    .bind(user_id)
    .execute(&mut *transaction)
    .await?;

    transaction.commit().await?;

    Ok((user_id, stored_email))
}

/// Queues a "forgot password" request for the admin portal to pick up. Returns `None`
/// for unknown or inactive accounts — callers must respond identically either way so the
/// endpoint cannot be used to enumerate users.
pub async fn queue_reset_request(pool: &PgPool, email: &str) -> AuthResult<Option<Uuid>> {
    let id: Option<Uuid> = sqlx::query_scalar(
        "insert into reset_requests (user_id)
         select u.id
           from users u
          where lower(u.email) = lower($1)
            and u.is_active
            and not exists (
                select 1 from reset_requests r
                 where r.user_id = u.id and r.status = 'open'
            )
         returning id",
    )
    .bind(email.trim())
    .fetch_optional(pool)
    .await?;

    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_unique_and_hashed() {
        let first = generate_token();
        let second = generate_token();
        assert_eq!(first.len(), 64);
        assert_ne!(first, second);
        assert_eq!(hash_token(&first).len(), 64);
        assert_ne!(hash_token(&first), first);
        assert_eq!(hash_token(&first), hash_token(&first));
    }

    #[test]
    fn reset_codes_are_readable_and_canonicalised() {
        let (display, canonical) = generate_reset_code();
        assert_eq!(display.len(), 9);
        assert_eq!(display.chars().nth(4), Some('-'));
        assert_eq!(canonical.len(), 8);
        assert_eq!(canonical_code(&display), canonical);
        assert_eq!(canonical_code(&canonical.to_lowercase()), canonical);
        assert_eq!(canonical_code("abcd efgh"), canonical_code("ABCDEFGH"));

        for character in canonical.chars() {
            assert!("ABCDEFGHJKLMNPQRSTUVWXYZ23456789".contains(character));
        }
    }
}
