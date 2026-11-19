//! Administrator credential store, deliberately separate from `users.csv` so a user import
//! can never create or promote an admin. Only `super_admin` exists; `app_admin` was removed.

use std::path::{Path, PathBuf};

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use chrono::Utc;
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::state::AppState;

#[derive(Debug, Clone, Deserialize)]
pub struct AdminRow {
    pub email: String,
    pub name: String,
    pub password_hash: String,
}

fn admins_path(state: &AppState) -> PathBuf {
    state.config.data_dir.join("admins.csv")
}

fn secret_path(state: &AppState) -> PathBuf {
    state.config.data_dir.join("secret.key")
}

/// Writes the admin file on first start. If `BOOTSTRAP_ADMIN_EMAIL` and
/// `BOOTSTRAP_ADMIN_PASSWORD` are set, the account is seeded with an Argon2 hash; otherwise
/// the file is left header-only and an administrator must add an account by hand.
pub fn ensure_file(dir: &Path) -> std::io::Result<()> {
    let path = dir.join("admins.csv");
    if path.exists() {
        return Ok(());
    }

    let mut content = String::from("email;name;password_hash\n");
    let email = std::env::var("BOOTSTRAP_ADMIN_EMAIL").unwrap_or_default();
    let password = std::env::var("BOOTSTRAP_ADMIN_PASSWORD").unwrap_or_default();
    if !email.trim().is_empty() && !password.is_empty() {
        content.push_str(&format!(
            "{};Administrator;{}\n",
            email.trim().to_lowercase(),
            hash_password(&password)
        ));
    }
    std::fs::write(&path, content)
}

pub fn all(state: &AppState) -> Vec<AdminRow> {
    let text = std::fs::read_to_string(admins_path(state)).unwrap_or_default();
    let mut rows = Vec::new();
    for line in text.lines().skip(1) {
        // Semicolon separated: Argon2 hashes themselves contain commas.
        let fields: Vec<&str> = line.split(';').collect();
        if fields.len() >= 3 && !fields[0].trim().is_empty() {
            rows.push(AdminRow {
                email: fields[0].trim().to_lowercase(),
                name: fields[1].trim().to_string(),
                password_hash: fields[2].trim().to_string(),
            });
        }
    }
    rows
}

pub fn find(state: &AppState, email: &str) -> Option<AdminRow> {
    let email = email.trim().to_lowercase();
    all(state).into_iter().find(|admin| admin.email == email)
}

pub fn hash_password(password: &str) -> String {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .expect("hash password")
}

pub fn verify_password(admin: &AdminRow, password: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(&admin.password_hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

fn secret(state: &AppState) -> Vec<u8> {
    let path = secret_path(state);
    if let Ok(bytes) = std::fs::read(&path) {
        if bytes.len() >= 16 {
            return bytes;
        }
    }

    let mut bytes = [0u8; 32];
    use rand::RngCore;
    rand::thread_rng().fill_bytes(&mut bytes);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&path, bytes);
    bytes.to_vec()
}

fn signature(secret: &[u8], payload: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(secret);
    hasher.update(b":");
    hasher.update(payload.as_bytes());
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Signed token: `email|expiry_unix|signature`. Not a password, but unforgeable without the
/// server secret and tied to an admin account that is checked on every request.
pub fn issue_token(state: &AppState, email: &str) -> String {
    let expires = Utc::now().timestamp() + 30 * 24 * 60 * 60;
    let payload = format!("{}|{}", email.trim().to_lowercase(), expires);
    let sig = signature(&secret(state), &payload);
    format!("{payload}|{sig}")
}

#[cfg(test)]
mod tests {
    use super::{hash_password, verify_password, AdminRow};

    #[test]
    fn argon2_hashes_verify() {
        let hash = hash_password("correct-horse");
        let admin = AdminRow {
            email: "a@example.com".to_string(),
            name: "A".to_string(),
            password_hash: hash,
        };
        assert!(verify_password(&admin, "correct-horse"));
        assert!(!verify_password(&admin, "wrong"));
    }
}

/// Returns the admin email when the token is valid and unexpired.
pub fn verify_token(state: &AppState, token: &str) -> Option<String> {
    let parts: Vec<&str> = token.split('|').collect();
    if parts.len() != 3 {
        return None;
    }
    let (email, expires, provided) = (parts[0], parts[1], parts[2]);
    let expiry: i64 = expires.parse().ok()?;
    if Utc::now().timestamp() > expiry {
        return None;
    }
    let payload = format!("{email}|{expires}");
    let expected = signature(&secret(state), &payload);
    if expected == provided && find(state, email).is_some() {
        Some(email.to_string())
    } else {
        None
    }
}
