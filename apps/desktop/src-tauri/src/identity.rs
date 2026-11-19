//! Identifies the user to the server. The email is entered once and cached on the device,
//! because a Windows account name is not always the same as the person's email local part.

use std::process::Command;

use crate::config;

/// `whoami` on Windows prints `DOMAIN\user`; on other platforms it may print just `user`.
pub fn windows_identity() -> String {
    let output = Command::new("whoami")
        .output()
        .ok()
        .filter(|result| result.status.success());

    let raw = output
        .map(|result| String::from_utf8_lossy(&result.stdout).trim().to_string())
        .unwrap_or_default();

    if !raw.is_empty() {
        return raw;
    }

    std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "unknown".to_string())
}

/// Human label for the UI, e.g. `laxmikhilnani.os` from `ad\laxmikhilnani.os`.
pub fn short_name(identity: &str) -> String {
    identity.rsplit('\\').next().unwrap_or(identity).to_string()
}

/// Prefills the email field on first run. The Windows name is only a guess — the user can
/// replace it before it is cached.
pub fn suggested_email() -> String {
    let raw = windows_identity();
    let local = short_name(&raw);

    if local.contains('@') {
        return local.to_lowercase();
    }

    format!("{}@{}", local, config::DEFAULT_MAIL_DOMAIN).to_lowercase()
}

/// The identity used on the wire: cached email first, Windows name as a fallback for the
/// very first launch before the user has confirmed an address.
pub fn current() -> String {
    config::cached_identity().unwrap_or_else(|| {
        let raw = windows_identity();
        if raw.contains('@') {
            raw
        } else {
            suggested_email()
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{short_name, suggested_email};

    #[test]
    fn strips_the_domain() {
        assert_eq!(short_name("ad\\laxmikhilnani.os"), "laxmikhilnani.os");
        assert_eq!(short_name("laxmikhilnani.os"), "laxmikhilnani.os");
    }

    #[test]
    fn suggestion_always_contains_an_at_sign() {
        let suggestion = suggested_email();
        assert!(suggestion.contains('@'), "{suggestion}");
    }
}
