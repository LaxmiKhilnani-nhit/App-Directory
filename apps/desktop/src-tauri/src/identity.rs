//! The machine's current Windows identity. There is no sign-in: this value is sent to the
//! server as `X-Windows-Identity`, and the server decides which apps the user may see.

use std::process::Command;

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

#[cfg(test)]
mod tests {
    use super::short_name;

    #[test]
    fn strips_the_domain() {
        assert_eq!(short_name("ad\\laxmikhilnani.os"), "laxmikhilnani.os");
        assert_eq!(short_name("laxmikhilnani.os"), "laxmikhilnani.os");
    }
}
