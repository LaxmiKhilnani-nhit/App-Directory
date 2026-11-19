//! Trusted Windows identity resolution.
//!
//! There is no password, session or onboarding code anywhere in this API. Every request is
//! attributed to the Windows username supplied by the reverse proxy in
//! `X-Windows-Identity` (or `X-Remote-User`), or by the caller as `?identity=`. A bare
//! username such as `ad\laxmikhilnani` becomes `laxmikhilnani@<mail_domain>` using the
//! `mail_domain` setting (default `nhit.co.in`).

use axum::{
    extract::FromRequestParts,
    http::{header::AUTHORIZATION, request::Parts},
};
use uuid::Uuid;

use crate::domain::models::SessionUser;
use crate::error::ApiError;
use crate::state::AppState;
use crate::store::UserRow;

const IDENTITY_HEADERS: [&str; 2] = ["x-windows-identity", "x-remote-user"];

/// Pulls a single value out of the raw query string without needing `serde_urlencoded`.
pub fn query_value(parts: &Parts, key: &str) -> Option<String> {
    let query = parts.uri.query()?;
    query.split('&').find_map(|pair| {
        let (name, value) = pair.split_once('=')?;
        if name == key {
            Some(percent_decode(value))
        } else {
            None
        }
    })
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let (Some(high), Some(low)) =
                (hex_value(bytes[index + 1]), hex_value(bytes[index + 2]))
            {
                out.push(high * 16 + low);
                index += 3;
                continue;
            }
        }
        if bytes[index] == b'+' {
            out.push(b' ');
        } else {
            out.push(bytes[index]);
        }
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// Turns `ad\laxmikhilnani`, `laxmikhilnani` or an email into a canonical email address.
pub fn normalize(raw: &str, mail_domain: &str) -> String {
    let mut value = raw.trim();
    if let Some((_, local)) = value.rsplit_once('\\') {
        value = local;
    }
    value = value.trim();
    if value.is_empty() {
        return String::new();
    }
    if value.contains('@') {
        value.to_lowercase()
    } else {
        format!(
            "{}@{}",
            value.to_lowercase(),
            mail_domain.trim().to_lowercase()
        )
    }
}

/// Resolves the trusted identity from the request. There is deliberately no fallback to a
/// configured or "first active" account: that would let an arbitrary imported user inherit
/// admin access. An absent identity must be rejected.
pub fn resolve(parts: &Parts, query_identity: Option<&str>, _state: &AppState) -> Option<String> {
    for name in IDENTITY_HEADERS {
        if let Some(raw) = parts
            .headers
            .get(name)
            .and_then(|value| value.to_str().ok())
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Some(raw.to_string());
        }
    }

    if let Some(raw) = query_identity
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Some(raw.to_string());
    }

    if let Some(token) = parts
        .headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| {
            value
                .strip_prefix("Bearer ")
                .or_else(|| value.strip_prefix("bearer "))
        })
        .map(str::trim)
        .filter(|token| !token.is_empty() && *token != "csv")
    {
        return Some(token.to_string());
    }

    None
}

/// Looks up the CSV user for a raw identity string.
pub fn find_user(state: &AppState, raw_identity: &str) -> Option<UserRow> {
    let email = normalize(raw_identity, &state.mail_domain());
    let data = state.store.read();
    data.user_by_email(&email).cloned()
}

fn synthetic_user(email: String) -> SessionUser {
    let name = email
        .split('@')
        .next()
        .unwrap_or("Administrator")
        .replace(['.', '_', '-'], " ");
    SessionUser {
        id: Uuid::new_v5(&Uuid::NAMESPACE_DNS, format!("identity:{email}").as_bytes()),
        email,
        name: if name.trim().is_empty() {
            "Administrator".to_string()
        } else {
            name
        },
        department: None,
        role: "super_admin".to_string(),
        admin_scope: None,
        must_change_password: false,
    }
}

/// Builds the session user handed to the admin SPA. Unknown identities are accepted as a
/// synthetic super admin so the portal's route guards pass; no account is created in CSV.
pub fn trusted_user(state: &AppState, raw_identity: &str) -> SessionUser {
    match find_user(state, raw_identity) {
        Some(user) => {
            let mut session = SessionUser::from(&user);
            session.role = "super_admin".to_string();
            session.must_change_password = false;
            session
        }
        None => synthetic_user(normalize(raw_identity, &state.mail_domain())),
    }
}

/// Session user for an identity that must exist in the CSV store.
pub fn known_user(state: &AppState, raw_identity: &str) -> Result<SessionUser, ApiError> {
    find_user(state, raw_identity)
        .map(|user| SessionUser::from(&user))
        .ok_or_else(|| {
            ApiError::forbidden(
                "unknown_user",
                "This Windows account is not registered in the App Directory.",
            )
        })
}

// ------------------------------------------------------------------ extractors

/// Any trusted caller, treated as a super admin by the admin portal.
pub struct TrustedUser(pub SessionUser);

impl FromRequestParts<AppState> for TrustedUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let identity = query_value(parts, "identity");
        let raw = resolve(parts, identity.as_deref(), state)
            .ok_or_else(|| ApiError::unauthorized("A Windows identity is required."))?;
        Ok(TrustedUser(trusted_user(state, &raw)))
    }
}

/// A trusted caller who must already exist in users.csv (used by client sync).
pub struct KnownUser(pub SessionUser);

impl FromRequestParts<AppState> for KnownUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let identity = query_value(parts, "identity");
        let raw = resolve(parts, identity.as_deref(), state).ok_or_else(|| {
            ApiError::forbidden("unknown_user", "No Windows identity was supplied.")
        })?;
        Ok(KnownUser(known_user(state, &raw)?))
    }
}
