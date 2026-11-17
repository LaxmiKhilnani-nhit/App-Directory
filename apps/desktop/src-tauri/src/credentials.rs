//! Windows Credential Manager access for the two secrets the client owns.
//!
//! Nothing sensitive is written to disk in plain text: the session token lives only in the
//! credential store, and `cache.bin` is encrypted with a key kept there too (PLAN.md §6.1).

use base64::Engine;
use keyring::Entry;
use rand::RngCore;

const SERVICE: &str = "AppDirectory";
const SESSION_ACCOUNT: &str = "session";
const SNAPSHOT_KEY_ACCOUNT: &str = "snapshot-key";

fn entry(account: &str) -> Result<Entry, String> {
    Entry::new(SERVICE, account).map_err(|error| format!("Credential store unavailable: {error}"))
}

fn read(account: &str) -> Result<Option<String>, String> {
    match entry(account)?.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(format!("Could not read the stored credential: {error}")),
    }
}

fn write(account: &str, value: &str) -> Result<(), String> {
    entry(account)?
        .set_password(value)
        .map_err(|error| format!("Could not store the credential: {error}"))
}

fn erase(account: &str) -> Result<(), String> {
    match entry(account)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(format!("Could not clear the stored credential: {error}")),
    }
}

pub fn token() -> Result<Option<String>, String> {
    Ok(read(SESSION_ACCOUNT)?.filter(|value| !value.is_empty()))
}

pub fn set_token(token: &str) -> Result<(), String> {
    write(SESSION_ACCOUNT, token)
}

pub fn clear_token() -> Result<(), String> {
    erase(SESSION_ACCOUNT)
}

/// 32 random bytes, generated once and reused for every cache write.
pub fn snapshot_key() -> Result<[u8; 32], String> {
    if let Some(stored) = read(SNAPSHOT_KEY_ACCOUNT)? {
        if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(stored) {
            if let Ok(key) = <[u8; 32]>::try_from(bytes.as_slice()) {
                return Ok(key);
            }
        }
        // Unreadable key material: the old cache is useless without it, so replace it.
    }

    let mut key = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut key);
    let encoded = base64::engine::general_purpose::STANDARD.encode(key);
    write(SNAPSHOT_KEY_ACCOUNT, &encoded)?;
    Ok(key)
}
