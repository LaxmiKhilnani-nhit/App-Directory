//! First-run desktop and Start-menu shortcuts (PLAN.md §6.2).
//!
//! Uses IShellLink directly so the client needs no script host or installer. Existing
//! shortcuts are never overwritten, and the whole step is recorded in `config.json`.

use std::path::{Path, PathBuf};

use windows::core::{Interface, HSTRING};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, IPersistFile, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
};
use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};

use crate::cache;

const DESCRIPTION: &str = "Launcher for in-house applications";

fn start_menu_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|base| base.join("Microsoft\\Windows\\Start Menu\\Programs"))
}

pub fn desktop_link() -> Option<PathBuf> {
    dirs::desktop_dir().map(|base| base.join("App Directory.lnk"))
}

pub fn start_menu_link() -> Option<PathBuf> {
    start_menu_dir().map(|base| base.join("App Directory.lnk"))
}

/// Writes one `.lnk`. Kept separate so it can be exercised without touching real folders.
pub fn create(target_exe: &Path, link: &Path) -> Result<(), String> {
    let working_directory = target_exe.parent().unwrap_or_else(|| Path::new("."));

    let target = HSTRING::from(target_exe);
    let working = HSTRING::from(working_directory);
    let icon = HSTRING::from(target_exe);
    let description = HSTRING::from(DESCRIPTION);
    let destination = HSTRING::from(link);

    unsafe {
        // RPC_E_CHANGED_MODE just means COM was already initialised for this thread.
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

        let shell_link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)
            .map_err(|error| format!("Could not create the shell link: {error}"))?;

        shell_link
            .SetPath(&target)
            .map_err(|error| format!("Could not set the shortcut target: {error}"))?;
        shell_link
            .SetWorkingDirectory(&working)
            .map_err(|error| format!("Could not set the working directory: {error}"))?;
        shell_link
            .SetDescription(&description)
            .map_err(|error| format!("Could not set the shortcut description: {error}"))?;
        shell_link
            .SetIconLocation(&icon, 0)
            .map_err(|error| format!("Could not set the shortcut icon: {error}"))?;

        let persist: IPersistFile = shell_link
            .cast()
            .map_err(|error| format!("Could not persist the shortcut: {error}"))?;

        // The last parameter is BOOL; windows-core accepts a bool here.
        persist
            .Save(&destination, true)
            .map_err(|error| format!("Could not write {link:?}: {error}"))?;
    }

    Ok(())
}

fn create_if_missing(target_exe: &Path, link: &Path) -> Result<bool, String> {
    if link.exists() {
        return Ok(false);
    }
    if let Some(parent) = link.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("Could not create {parent:?}: {error}"))?;
    }
    create(target_exe, link)?;
    Ok(true)
}

/// Creates both shortcuts once per profile. Safe to call on every launch.
pub fn ensure(target_exe: &Path) -> Result<(), String> {
    let mut settings = cache::load_config();
    if settings.shortcuts_created {
        return Ok(());
    }

    let mut made_any = false;
    if let Some(link) = desktop_link() {
        made_any |= create_if_missing(target_exe, &link)?;
    }
    if let Some(link) = start_menu_link() {
        made_any |= create_if_missing(target_exe, &link)?;
    }

    settings.shortcuts_created = true;
    cache::save_config(&settings)?;

    if made_any {
        tracing::info!("desktop and Start-menu shortcuts created");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_a_link_file() {
        let directory =
            std::env::temp_dir().join(format!("appdir-shortcut-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let link = directory.join("Test.lnk");
        let _ = std::fs::remove_file(&link);

        let exe = std::env::current_exe().unwrap();
        create(&exe, &link).expect("shortcut");
        assert!(link.exists(), "the .lnk should exist");
        assert!(std::fs::metadata(&link).unwrap().len() > 0);

        let _ = std::fs::remove_file(&link);
        let _ = std::fs::remove_dir(&directory);
    }
}
