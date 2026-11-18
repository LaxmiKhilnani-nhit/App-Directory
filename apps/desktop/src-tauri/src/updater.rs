//! Portable, flag-driven self-update.
//!
//! When the server sets `update_enabled = 1` and publishes a version, the client shows an
//! "Update now" link. Clicking it downloads the new exe, verifies its SHA-256, swaps it into
//! place on the next launch, starts the new binary and exits. Nothing is installed.

use std::fs;
use std::process::Command;

use sha2::{Digest, Sha256};

use crate::api::{ApiClient, CommandError, UpdateInfo};
use crate::config;

fn hash_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Downloads and installs an update. Returns only if something failed; on success the new exe
/// is spawned and this process exits.
pub async fn install(api: &ApiClient, info: &UpdateInfo) -> Result<(), CommandError> {
    let bytes = api.download(&info.url).await?;
    if bytes.len() < 1024 {
        return Err(CommandError::new(
            "invalid_update",
            "The downloaded update is too small to be a valid executable.",
        ));
    }

    if !info.sha256.trim().is_empty() {
        let actual = hash_hex(&bytes);
        if !actual.eq_ignore_ascii_case(info.sha256.trim()) {
            return Err(CommandError::new(
                "invalid_update",
                format!(
                    "The update failed its checksum check (expected {}, got {}).",
                    info.sha256,
                    &actual[..16.min(actual.len())]
                ),
            ));
        }
    }

    let updates_dir = config::data_dir().join("updates");
    fs::create_dir_all(&updates_dir).map_err(|error| {
        CommandError::new(
            "update_failed",
            format!("Could not create the update folder: {error}"),
        )
    })?;

    let staged = updates_dir.join("App Directory.exe");
    fs::write(&staged, &bytes).map_err(|error| {
        CommandError::new(
            "update_failed",
            format!("Could not stage the update: {error}"),
        )
    })?;

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

    // Windows allows renaming a running executable but not overwriting it.
    fs::rename(&current, &backup).map_err(|error| {
        CommandError::new(
            "update_failed",
            format!("Could not move the old executable aside: {error}"),
        )
    })?;

    if let Err(error) = fs::copy(&staged, &current) {
        // Roll back so the client keeps working.
        let _ = fs::rename(&backup, &current);
        return Err(CommandError::new(
            "update_failed",
            format!("Could not install the new executable: {error}"),
        ));
    }

    let _ = fs::remove_file(&staged);

    match Command::new(&current).spawn() {
        Ok(_) => {
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
    use super::hash_hex;

    #[test]
    fn sha256_matches_a_known_value() {
        assert_eq!(
            hash_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
