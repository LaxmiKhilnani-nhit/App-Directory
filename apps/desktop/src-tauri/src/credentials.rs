//! The only secret left is the AES key for the encrypted offline cache. There is no login
//! token: the client identifies itself with the current Windows username.

use base64::Engine;
use keyring::Entry;
use rand::RngCore;

const SERVICE: &str = "AppDirectory";
const SNAPSHOT_KEY_ACCOUNT: &str = "snapshot-key";

fn entry(account: &str) -> Result<Entry, String> {
    Entry::new(SERVICE, account).map_err(|error| format!("Credential store unavailable: {error}"))
}

/// 32 random bytes, generated once and reused for every cache write.
pub fn snapshot_key() -> Result<[u8; 32], String> {
    let entry = entry(SNAPSHOT_KEY_ACCOUNT)?;

    if let Ok(stored) = entry.get_password() {
        if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(stored) {
            if let Ok(key) = <[u8; 32]>::try_from(bytes.as_slice()) {
                return Ok(key);
            }
        }
    }

    let mut key = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut key);
    let encoded = base64::engine::general_purpose::STANDARD.encode(key);
    entry
        .set_password(&encoded)
        .map_err(|error| format!("Could not store the cache key: {error}"))?;
    Ok(key)
}
