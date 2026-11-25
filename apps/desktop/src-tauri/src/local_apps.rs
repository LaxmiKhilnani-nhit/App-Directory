//! Discovery of applications installed on this machine.
//!
//! We only read Start Menu shortcuts — no registry walking, no COM, no icon extraction — so a
//! full scan stays in the low milliseconds. Results are cached as JSON in the app's data folder
//! so the UI can render instantly on launch while a fresh scan runs in the background.
//! Nothing here is sent to the server: local apps belong only to the machine they were found on.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{api::CommandError, config};

const CACHE_NAME: &str = "local_apps.json";
const MAX_DEPTH: usize = 4;

/// Start Menu folders that hold Windows administration tools rather than installed apps.
const SYSTEM_FOLDERS: [&str; 6] = [
    "Windows Tools",
    "Administrative Tools",
    "Windows PowerShell",
    "System Tools",
    "Startup",
    "Windows Administration",
];

/// Conservative name hints for shortcuts that are clearly built-in Windows tools.
const SYSTEM_NAMES: [&str; 18] = [
    "registry editor",
    "regedit",
    "command prompt",
    "cmd",
    "powershell",
    "control panel",
    "device manager",
    "disk management",
    "event viewer",
    "task scheduler",
    "services",
    "system configuration",
    "msconfig",
    "windows defender",
    "disk cleanup",
    "performance monitor",
    "resource monitor",
    "snipping tool",
];

/// A single discovered application. `path` points at a Windows Start Menu shortcut (.lnk).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalApp {
    pub id: String,
    pub name: String,
    pub path: String,
    /// Present in results so administrators can opt in to showing them.
    #[serde(default)]
    pub is_system: bool,
}

/// Flags a discovered shortcut as a Windows system tool by its folder or its display name.
fn is_system_app(name: &str, path: &str) -> bool {
    let in_system_folder = Path::new(path).components().any(|component| {
        component.as_os_str().to_str().is_some_and(|part| {
            SYSTEM_FOLDERS
                .iter()
                .any(|folder| part.eq_ignore_ascii_case(folder))
        })
    });
    if in_system_folder {
        return true;
    }

    let lower = name.to_lowercase();
    SYSTEM_NAMES.iter().any(|needle| lower.contains(needle))
}

fn ignore_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    ["uninstall", "readme", "help", "documentation", "license"]
        .iter()
        .any(|needle| lower.contains(needle))
}

fn app_id(path: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(path.as_bytes());
    hasher
        .finalize()
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Walks a Start Menu root, collecting shortcut files. Bounded depth keeps the scan predictable.
fn collect(dir: &Path, depth: usize, into: &mut Vec<PathBuf>) {
    if depth > MAX_DEPTH {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, depth + 1, into);
        } else if path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("lnk"))
        {
            into.push(path);
        }
    }
}

fn start_menu_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Ok(program_data) = std::env::var("ProgramData") {
        roots.push(
            Path::new(&program_data)
                .join("Microsoft")
                .join("Windows")
                .join("Start Menu")
                .join("Programs"),
        );
    }
    if let Ok(app_data) = std::env::var("APPDATA") {
        roots.push(
            Path::new(&app_data)
                .join("Microsoft")
                .join("Windows")
                .join("Start Menu")
                .join("Programs"),
        );
    }
    roots
}

/// Scans both the all-users and current-user Start Menu folders, dedupes by display name,
/// and returns a sorted list.
pub fn scan() -> Vec<LocalApp> {
    let mut paths = Vec::new();
    for root in start_menu_roots() {
        collect(&root, 0, &mut paths);
    }

    let mut seen = HashSet::new();
    let mut apps = Vec::new();
    for path in paths {
        let name = path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .trim()
            .to_string();
        if name.is_empty() || ignore_name(&name) || !seen.insert(name.to_lowercase()) {
            continue;
        }
        let path_text = path.to_string_lossy().to_string();
        apps.push(LocalApp {
            id: app_id(&path_text),
            is_system: is_system_app(&name, &path_text),
            name,
            path: path_text,
        });
    }

    apps.sort_by(|left, right| {
        left.name
            .to_lowercase()
            .cmp(&right.name.to_lowercase())
            .then_with(|| left.path.cmp(&right.path))
    });
    apps
}

fn cache_path() -> PathBuf {
    config::data_dir().join(CACHE_NAME)
}

pub fn load_cache() -> Vec<LocalApp> {
    fs::read_to_string(cache_path())
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

pub fn save_cache(apps: &[LocalApp]) -> Result<(), CommandError> {
    let path = cache_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let text = serde_json::to_string(apps).map_err(|error| {
        CommandError::new("local_apps_failed", format!("Could not cache local apps: {error}"))
    })?;
    fs::write(path, text).map_err(|error| {
        CommandError::new("local_apps_failed", format!("Could not cache local apps: {error}"))
    })
}

/// Launches a discovered app. On Windows `open::that` uses ShellExecute, which correctly handles
/// `.lnk` shortcuts as well as executables without opening a console window.
pub fn open(path: &str) -> Result<(), CommandError> {
    let target = Path::new(path);
    if !target.is_file() {
        return Err(CommandError::new(
            "local_app_missing",
            "That application is no longer installed.",
        ));
    }
    let allowed = target
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("lnk") || extension.eq_ignore_ascii_case("exe")
        });
    if !allowed {
        return Err(CommandError::new(
            "local_app_invalid",
            "Only installed application shortcuts can be launched.",
        ));
    }
    open::that(target).map_err(|error| {
        CommandError::new(
            "local_app_failed",
            format!("Could not start that application: {error}"),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::{app_id, ignore_name, is_system_app};

    #[test]
    fn stable_ids_and_noise_filter() {
        assert_eq!(app_id("a"), app_id("a"));
        assert_ne!(app_id("a"), app_id("b"));
        assert!(ignore_name("Uninstall App"));
        assert!(!ignore_name("Google Chrome"));
    }

    #[test]
    fn detects_windows_tools_by_folder_and_name() {
        let tools = r"C:\ProgramData\Microsoft\Windows\Start Menu\Programs\Windows Tools\Services.lnk";
        assert!(is_system_app("Some tool", tools));
        assert!(is_system_app("Registry Editor", r"C:\Start Menu\Programs\Registry Editor.lnk"));
        assert!(is_system_app("Windows PowerShell", r"C:\Start Menu\Programs\PowerShell.lnk"));
    }

    #[test]
    fn leaves_ordinary_apps_untouched() {
        assert!(!is_system_app(
            "Google Chrome",
            r"C:\ProgramData\Microsoft\Windows\Start Menu\Programs\Google Chrome.lnk"
        ));
        assert!(!is_system_app("Payroll", r"C:\Start Menu\Programs\Payroll.lnk"));
    }
}
