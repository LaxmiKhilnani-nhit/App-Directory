//! Silent self-update for the portable exe.
//!
//! On launch the client checks the server's flag and, if the published version is newer,
//! downloads and verifies the new exe into `%LOCALAPPDATA%\AppDirectory\updates\`. The next
//! launch swaps it in before the UI opens. Nothing is installed and no click is required.

use std::fs;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

use sha2::{Digest, Sha256};

use crate::api::{ApiClient, CommandError, UpdateInfo};
use crate::config;

const STAGED_NAME: &str = "App Directory.exe";
const HASH_NAME: &str = "sha256.txt";

fn hash_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn updates_dir() -> PathBuf {
    config::data_dir().join("updates")
}

fn staged_path() -> PathBuf {
    updates_dir().join(STAGED_NAME)
}

fn hash_path() -> PathBuf {
    updates_dir().join(HASH_NAME)
}

/// Numeric dotted-version comparison, e.g. `0.2.0` > `0.1.9`. Non-numeric parts compare as 0.
pub fn is_newer(published: &str, current: &str) -> bool {
    let parts = |value: &str| -> Vec<u64> {
        value
            .split('.')
            .map(|part| part.parse::<u64>().unwrap_or(0))
            .collect()
    };

    let (published, current) = (parts(published), parts(current));
    for index in 0..published.len().max(current.len()) {
        let left = *published.get(index).unwrap_or(&0);
        let right = *current.get(index).unwrap_or(&0);
        if left != right {
            return left > right;
        }
    }
    false
}

/// Downloads and verifies an available update. Safe to call on every launch: a verified
/// staged file is left in place and downloaded again only if it is missing or corrupted.
pub async fn prepare(api: &ApiClient) -> Result<bool, CommandError> {
    let info = api.update_info().await?;
    if !info.available || !is_newer(&info.version, env!("CARGO_PKG_VERSION")) {
        return Ok(false);
    }

    if info.sha256.trim().is_empty() {
        return Err(CommandError::new(
            "invalid_update",
            "The server did not publish a checksum for this update.",
        ));
    }

    fs::create_dir_all(updates_dir()).map_err(|error| {
        CommandError::new(
            "update_failed",
            format!("Could not create the update folder: {error}"),
        )
    })?;

    let expected = info.sha256.trim().to_lowercase();
    if let (Ok(bytes), Ok(stored_hash)) = (fs::read(staged_path()), fs::read_to_string(hash_path()))
    {
        if stored_hash.trim().eq_ignore_ascii_case(&expected) && hash_hex(&bytes) == expected {
            return Ok(true);
        }
    }

    let bytes = api.download(&info.url).await?;
    if bytes.len() < 1024 {
        return Err(CommandError::new(
            "invalid_update",
            "The downloaded update is too small to be a valid executable.",
        ));
    }

    let actual = hash_hex(&bytes);
    if !actual.eq_ignore_ascii_case(&expected) {
        return Err(CommandError::new(
            "invalid_update",
            format!(
                "The update failed its checksum check (expected {}, got {}).",
                expected,
                &actual[..16.min(actual.len())]
            ),
        ));
    }

    fs::write(staged_path(), &bytes).map_err(|error| {
        CommandError::new(
            "update_failed",
            format!("Could not stage the update: {error}"),
        )
    })?;
    fs::write(hash_path(), &expected).map_err(|error| {
        CommandError::new(
            "update_failed",
            format!("Could not record the checksum: {error}"),
        )
    })?;

    Ok(true)
}

/// Called at startup, before the UI opens. Replaces the running exe with a verified staged
/// one and launches it. Returns `false` when there is nothing to apply.
pub fn apply_staged() -> Result<bool, CommandError> {
    // Windows may have kept a handle to the previous exe until it exited. Delete it here on
    // the new process's first launch, so no `.old.exe` is left on the user's machine.
    if let Ok(current) = std::env::current_exe() {
        let previous = current.with_file_name("App Directory.old.exe");
        if previous.exists() {
            let _ = fs::remove_file(&previous);
        }
    }

    let path = staged_path();
    if !path.is_file() {
        return Ok(false);
    }

    let bytes = fs::read(&path).map_err(|error| {
        CommandError::new(
            "update_failed",
            format!("Could not read the staged update: {error}"),
        )
    })?;
    let expected = fs::read_to_string(hash_path()).unwrap_or_default();
    if expected.trim().is_empty() || hash_hex(&bytes) != expected.trim().to_lowercase() {
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(hash_path());
        return Ok(false);
    }

    let current = std::env::current_exe().map_err(|error| {
        CommandError::new(
            "update_failed",
            format!("Could not locate the running exe: {error}"),
        )
    })?;
    let backup = current.with_file_name("App Directory.old.exe");

    if backup.exists() {
        let _ = fs::remove_file(&backup);
    }

    // Windows allows renaming a running exe but not overwriting it.
    fs::rename(&current, &backup).map_err(|error| {
        CommandError::new(
            "update_failed",
            format!("Could not move the old executable aside: {error}"),
        )
    })?;

    if let Err(error) = fs::copy(&path, &current) {
        let _ = fs::rename(&backup, &current);
        return Err(CommandError::new(
            "update_failed",
            format!("Could not install the new executable: {error}"),
        ));
    }

    let _ = fs::remove_file(&path);
    let _ = fs::remove_file(hash_path());

    match Command::new(&current)
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
    {
        Ok(_) => {
            // Usually the old process still holds this file; the new process cleans it up.
            let _ = fs::remove_file(&backup);
            std::process::exit(0);
        }
        Err(error) => {
            let _ = fs::rename(&backup, &current);
            Err(CommandError::new(
                "update_failed",
                format!("Could not restart after the update: {error}"),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::is_newer;

    #[test]
    fn compares_dotted_versions() {
        assert!(is_newer("0.2.0", "0.1.9"));
        assert!(is_newer("1.0", "0.9.9"));
        assert!(!is_newer("0.1.9", "0.2.0"));
        assert!(!is_newer("0.1.0", "0.1.0"));
    }
}
