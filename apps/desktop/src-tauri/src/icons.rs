//! Icons live on disk so an offline start still shows real artwork.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use base64::Engine;

use crate::config;

pub fn path_for(app_id: &str) -> PathBuf {
    config::icons_dir().join(format!("{app_id}.bin"))
}

/// Sniffs the served bytes; the admin portal accepts several formats, not just PNG.
pub fn mime_of(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        return "image/png";
    }
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return "image/jpeg";
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return "image/gif";
    }
    if bytes.len() > 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        return "image/webp";
    }
    if bytes.starts_with(&[0x00, 0x00, 0x01, 0x00]) {
        return "image/x-icon";
    }

    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(256)]).to_lowercase();
    if head.contains("<svg") {
        return "image/svg+xml";
    }

    "application/octet-stream"
}

pub fn save(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Could not create {parent:?}: {error}"))?;
    }
    fs::write(path, bytes).map_err(|error| format!("Could not save the icon: {error}"))
}

/// Reads a cached icon back as a data URL, which works regardless of asset-protocol scope.
pub fn data_url(app_id: &str) -> Option<String> {
    let bytes = fs::read(path_for(app_id)).ok()?;
    if bytes.is_empty() {
        return None;
    }

    let mime = mime_of(&bytes);
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    Some(format!("data:{mime};base64,{encoded}"))
}

/// Deletes icons for apps that are no longer in the snapshot.
pub fn prune(keep: &HashSet<String>) {
    let directory = config::icons_dir();
    let Ok(entries) = fs::read_dir(&directory) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
            continue;
        };
        if !keep.contains(stem) {
            let _ = fs::remove_file(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::mime_of;

    #[test]
    fn sniffs_the_formats_the_admin_portal_accepts() {
        assert_eq!(
            mime_of(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 1]),
            "image/png"
        );
        assert_eq!(mime_of(&[0xFF, 0xD8, 0xFF, 0xE0]), "image/jpeg");
        assert_eq!(
            mime_of(br#"<svg xmlns="http://www.w3.org/2000/svg"></svg>"#),
            "image/svg+xml"
        );
        assert_eq!(mime_of(b"not an image"), "application/octet-stream");
    }
}
