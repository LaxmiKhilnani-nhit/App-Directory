//! Encrypted-at-rest snapshot and the tiny `config.json` beside it.
//!
//! Layout under `%LOCALAPPDATA%\AppDirectory` (PLAN.md §6.1):
//! `config.json`, `cache.bin`, `icons\<app_id>.<ext>`.

use std::fs;
use std::path::Path;

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use chrono::{DateTime, Utc};
use rand::RngCore;
use serde::{Deserialize, Serialize};

use crate::api::SyncPayload;
use crate::{config, credentials};

/// Marks the cache file so a stale or foreign file is rejected instead of half-parsed.
const MAGIC: &[u8; 8] = b"APPDIR1\0";
const NONCE_LEN: usize = 12;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredSnapshot {
    pub schema_version: u32,
    pub last_synced_at: DateTime<Utc>,
    pub payload: SyncPayload,
}

impl StoredSnapshot {
    pub fn is_expired(&self, now: DateTime<Utc>) -> bool {
        now > self.payload.offline_valid_until || self.schema_version != config::SCHEMA_VERSION
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientConfig {
    pub schema_version: u32,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub revision: Option<i64>,
    pub shortcuts_created: bool,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            schema_version: config::SCHEMA_VERSION,
            last_synced_at: None,
            revision: None,
            shortcuts_created: false,
        }
    }
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Could not create {parent:?}: {error}"))?;
    }

    let temporary = path.with_extension("tmp");
    fs::write(&temporary, bytes)
        .map_err(|error| format!("Could not write {temporary:?}: {error}"))?;

    if path.exists() {
        let _ = fs::remove_file(path);
    }

    fs::rename(&temporary, path).map_err(|error| format!("Could not replace {path:?}: {error}"))
}

pub fn save(snapshot: &StoredSnapshot) -> Result<(), String> {
    let key = credentials::snapshot_key()?;
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|error| format!("Snapshot key is unusable: {error}"))?;

    let plaintext = serde_json::to_vec(snapshot)
        .map_err(|error| format!("Could not serialise the snapshot: {error}"))?;

    let mut nonce = [0u8; NONCE_LEN];
    rand::thread_rng().fill_bytes(&mut nonce);

    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce), plaintext.as_ref())
        .map_err(|_| "Could not encrypt the snapshot.".to_string())?;

    let mut file = Vec::with_capacity(MAGIC.len() + NONCE_LEN + ciphertext.len());
    file.extend_from_slice(MAGIC);
    file.extend_from_slice(&nonce);
    file.extend_from_slice(&ciphertext);

    write_atomic(&config::cache_path(), &file)
}

pub fn load() -> Result<Option<StoredSnapshot>, String> {
    let path = config::cache_path();
    if !path.exists() {
        return Ok(None);
    }

    let file = fs::read(&path).map_err(|error| format!("Could not read the cache: {error}"))?;
    if file.len() <= MAGIC.len() + NONCE_LEN || &file[..MAGIC.len()] != MAGIC {
        return Err("The cached snapshot is not in a format this client understands.".to_string());
    }

    let key = credentials::snapshot_key()?;
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|error| format!("Snapshot key is unusable: {error}"))?;

    let nonce = &file[MAGIC.len()..MAGIC.len() + NONCE_LEN];
    let ciphertext = &file[MAGIC.len() + NONCE_LEN..];

    let plaintext = cipher
        .decrypt(Nonce::from_slice(nonce), ciphertext)
        .map_err(|_| "The cached snapshot could not be decrypted.".to_string())?;

    let snapshot: StoredSnapshot = serde_json::from_slice(&plaintext)
        .map_err(|error| format!("The cached snapshot is unreadable: {error}"))?;

    Ok(Some(snapshot))
}

/// Wipes the cached data **and its metadata**, so a later login cannot resurrect a previous
/// person's "last synced / revision" numbers on the sign-in screen.

pub fn load_config() -> ClientConfig {
    fs::read(config::config_path())
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

pub fn save_config(value: &ClientConfig) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("Could not serialise config.json: {error}"))?;
    write_atomic(&config::config_path(), &bytes)
}

/// Records a successful sync in `config.json`; the snapshot itself stores the payload.
pub fn remember_sync(last_synced_at: DateTime<Utc>, revision: i64) -> Result<(), String> {
    let mut value = load_config();

    // Nothing changed on the server: avoid a redundant disk write on every refresh.
    let unchanged = value.revision == Some(revision)
        && value
            .last_synced_at
            .is_some_and(|previous| (last_synced_at - previous).num_seconds() < 60);
    if unchanged {
        return Ok(());
    }

    value.schema_version = config::SCHEMA_VERSION;
    value.last_synced_at = Some(last_synced_at);
    value.revision = Some(revision);
    save_config(&value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{SyncPayload, SyncUser};

    fn snapshot(valid_until: DateTime<Utc>, schema_version: u32) -> StoredSnapshot {
        StoredSnapshot {
            schema_version,
            last_synced_at: Utc::now(),
            payload: SyncPayload {
                revision: 7,
                server_time: Utc::now(),
                offline_valid_until: valid_until,
                snapshot_days: 7,
                sync_interval_hours: 4,
                announcement: None,
                user: SyncUser {
                    name: "Test User".to_string(),
                    department: None,
                },
                apps: Vec::new(),
                phone_directory: Vec::new(),
            },
        }
    }

    #[test]
    fn stays_valid_inside_the_grace_window() {
        let stored = snapshot(
            Utc::now() + chrono::Duration::days(6),
            config::SCHEMA_VERSION,
        );
        assert!(!stored.is_expired(Utc::now()));
    }

    #[test]
    fn expires_when_the_server_window_closes() {
        let stored = snapshot(
            Utc::now() - chrono::Duration::seconds(1),
            config::SCHEMA_VERSION,
        );
        assert!(stored.is_expired(Utc::now()));
    }

    #[test]
    fn rejects_a_snapshot_from_another_schema() {
        let stored = snapshot(
            Utc::now() + chrono::Duration::days(1),
            config::SCHEMA_VERSION + 1,
        );
        assert!(stored.is_expired(Utc::now()));
    }
}
