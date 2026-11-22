//! Mailbox home layout under the site tree: `/home/<domain>/e-mails/<user>/`.
//!
//! New local (PAM) mailboxes use this path so mailbox Maildirs are not created as
//! top-level `/home/<localpart>` next to website homes. Classic `/home/vmail/...`
//! trees remain readable for import/heal on hosts that already use them.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Directory name under the site home that holds mailbox user homes.
pub const EMAILS_DIR: &str = "e-mails";

/// Classic virtual-mail tree still used by some hosts (read/heal only).
pub const CLASSIC_VMAIL_ROOT: &str = "/home/vmail";

/// Preferred mailbox home: `/home/<domain>/e-mails/<local>`.
pub fn preferred_mailbox_home(domain: &str, local: &str) -> PathBuf {
    PathBuf::from(format!(
        "/home/{}/{EMAILS_DIR}/{}",
        domain.trim().to_ascii_lowercase(),
        local.trim().to_ascii_lowercase()
    ))
}

/// Preferred Maildir path under the domain e-mails tree.
pub fn preferred_maildir(domain: &str, local: &str) -> PathBuf {
    preferred_mailbox_home(domain, local).join("Maildir")
}

/// Parse `local@domain` into lowercase parts.
pub fn split_address(address: &str) -> Result<(String, String), String> {
    let address = address.trim().to_ascii_lowercase();
    let Some((local, domain)) = address.split_once('@') else {
        return Err("Mailbox address must include @domain".into());
    };
    if local.is_empty() || domain.is_empty() {
        return Err("Mailbox address is incomplete".into());
    }
    if local.contains('/') || domain.contains('/') || local.contains("..") || domain.contains("..")
    {
        return Err("Mailbox address path components are invalid".into());
    }
    Ok((local.to_string(), domain.to_string()))
}

/// Resolve an existing mailbox home for `address` (preferred, legacy PAM, classic vmail).
pub fn resolve_mailbox_home(address: &str) -> Option<PathBuf> {
    let (local, domain) = split_address(address).ok()?;
    let candidates = [
        preferred_mailbox_home(&domain, &local),
        PathBuf::from(format!("/home/{local}")),
        PathBuf::from(format!("{CLASSIC_VMAIL_ROOT}/{domain}/{local}")),
    ];
    for home in candidates {
        if home.is_dir() && home.join("Maildir").is_dir() {
            return Some(home);
        }
        // Home may exist before Maildir is seeded.
        if home == preferred_mailbox_home(&domain, &local) && home.is_dir() {
            return Some(home);
        }
    }
    None
}

/// Resolve Maildir for `address`, preferring the domain e-mails layout.
pub fn resolve_maildir(address: &str) -> Option<PathBuf> {
    resolve_mailbox_home(address)
        .map(|h| h.join("Maildir"))
        .filter(|p| p.is_dir())
}

/// True when `home` is under `/home/<domain>/e-mails/`.
pub fn is_domain_emails_home(home: &Path) -> bool {
    let s = home.to_string_lossy().replace('\\', "/");
    let parts: Vec<&str> = s.split('/').filter(|p| !p.is_empty()).collect();
    // home / <domain> / e-mails / <user>
    parts.len() >= 4 && parts[0] == "home" && parts[parts.len() - 2] == EMAILS_DIR
}

/// True when `home` looks like a legacy top-level PAM mailbox (`/home/<local>`).
pub fn is_legacy_top_level_home(home: &Path, local: &str) -> bool {
    home == Path::new(&format!("/home/{local}"))
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MigrateHomesReport {
    pub scanned: usize,
    pub migrated: usize,
    pub skipped: usize,
    pub dry_run: bool,
    pub notes: Vec<String>,
}

/// Move legacy `/home/<local>` mailbox homes into `/home/<domain>/e-mails/<local>`.
///
/// Does not rotate passwords. Skips classic `/home/vmail` trees and already-migrated homes.
/// When `dry_run` is true, only logs planned moves.
pub fn migrate_legacy_top_level_homes(dry_run: bool) -> Result<MigrateHomesReport, String> {
    let mut report = MigrateHomesReport {
        dry_run,
        ..MigrateHomesReport::default()
    };
    let accounts = crate::mail_accounts::list_accounts();
    for account in accounts {
        if account.smtp_mode != crate::mail_accounts::MailSmtpMode::PostfixLocal {
            continue;
        }
        report.scanned += 1;
        match migrate_one_address(&account.address, dry_run) {
            Ok(MigrateOne::Migrated(note)) => {
                report.migrated += 1;
                report.notes.push(note);
            }
            Ok(MigrateOne::Skipped(note)) => {
                report.skipped += 1;
                report.notes.push(note);
            }
            Err(err) => {
                report.skipped += 1;
                report.notes.push(format!("{}: {err}", account.address));
            }
        }
    }
    Ok(report)
}

#[derive(Debug)]
enum MigrateOne {
    Migrated(String),
    Skipped(String),
}

fn migrate_one_address(address: &str, dry_run: bool) -> Result<MigrateOne, String> {
    let (local, domain) = split_address(address)?;
    let dest = preferred_mailbox_home(&domain, &local);
    let legacy = PathBuf::from(format!("/home/{local}"));

    if dest.join("Maildir").is_dir() {
        if is_legacy_top_level_home(&legacy, &local) && legacy.join("Maildir").is_dir() {
            return Ok(MigrateOne::Skipped(format!(
                "{address}: dest already has Maildir; legacy `/home/{local}` still present (manual cleanup)"
            )));
        }
        // Ensure passwd home points at dest when the user exists.
        if !dry_run {
            let _ = ensure_unix_home_dir(&local, &dest);
        }
        return Ok(MigrateOne::Skipped(format!(
            "{address}: already under domain e-mails layout"
        )));
    }

    if !legacy.is_dir() || !legacy.join("Maildir").is_dir() {
        return Ok(MigrateOne::Skipped(format!(
            "{address}: no legacy `/home/{local}/Maildir` to move"
        )));
    }

    // Do not migrate website homes that happen to share a name (no Maildir-only check above).
    if looks_like_website_home(&legacy) {
        return Ok(MigrateOne::Skipped(format!(
            "{address}: `/home/{local}` looks like a website home; skipped"
        )));
    }

    let note = format!(
        "{address}: {} `/home/{local}` -> {}",
        if dry_run { "would move" } else { "move" },
        dest.display()
    );
    if dry_run {
        return Ok(MigrateOne::Migrated(note));
    }

    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Cannot create {}: {e}", parent.display()))?;
    }
    if dest.exists() {
        return Err(format!(
            "Destination `{}` already exists; refusing to overwrite",
            dest.display()
        ));
    }
    fs::rename(&legacy, &dest).map_err(|e| {
        format!(
            "Cannot move `{}` to `{}`: {e}",
            legacy.display(),
            dest.display()
        )
    })?;
    ensure_unix_home_dir(&local, &dest)?;
    let _ = Command::new("chown")
        .args(["-R", &format!("{local}:{local}"), &dest.to_string_lossy()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    Ok(MigrateOne::Migrated(note))
}

fn looks_like_website_home(home: &Path) -> bool {
    home.join("public_html").is_dir()
        || home.join("public_html").is_file()
        || home.join("html").is_dir()
}

/// Ensure Maildir skeleton exists under `home` and is owned by `user`.
pub fn ensure_maildir_skeleton(home: &Path, user: &str) -> Result<(), String> {
    let maildir = home.join("Maildir");
    for sub in ["new", "cur", "tmp"] {
        let p = maildir.join(sub);
        fs::create_dir_all(&p).map_err(|e| format!("Cannot create Maildir: {e}"))?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(home, fs::Permissions::from_mode(0o700));
        let _ = fs::set_permissions(&maildir, fs::Permissions::from_mode(0o700));
    }
    let _ = Command::new("chown")
        .args(["-R", &format!("{user}:{user}"), &home.to_string_lossy()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    Ok(())
}

/// Create or update the Unix account home directory pointer (no password changes).
pub fn ensure_unix_home_dir(user: &str, home: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        let _ = (user, home);
        return Ok(());
    }
    #[cfg(not(windows))]
    {
        if let Some(parent) = home.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Cannot create {}: {e}", parent.display()))?;
        }
        if !home.is_dir() {
            fs::create_dir_all(home)
                .map_err(|e| format!("Cannot create {}: {e}", home.display()))?;
        }
        if !crate::panel_ops_mailbox_provision::user_exists(user) {
            return Ok(());
        }
        let current = passwd_home(user).unwrap_or_default();
        if current == home.to_string_lossy() {
            return Ok(());
        }
        let status = Command::new("usermod")
            .args(["-d", &home.to_string_lossy(), user])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .status()
            .map_err(|e| format!("usermod failed: {e}"))?;
        if !status.success() {
            return Err(format!("usermod -d failed for `{user}`"));
        }
        Ok(())
    }
}

#[cfg(unix)]
fn passwd_home(user: &str) -> Option<String> {
    let output = Command::new("getent")
        .args(["passwd", user])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let line = String::from_utf8_lossy(&output.stdout);
    // name:x:uid:gid:gecos:home:shell
    line.trim().split(':').nth(5).map(|s| s.to_string())
}

#[cfg(not(unix))]
fn passwd_home(_user: &str) -> Option<String> {
    None
}

/// Move mailbox home and rename the Unix account when the local part changes.
///
/// Best-effort: never logs passwords. Returns a short non-secret note.
pub fn rename_mailbox_home(old_address: &str, new_address: &str) -> Result<String, String> {
    let (old_local, old_domain) = split_address(old_address)?;
    let (new_local, new_domain) = split_address(new_address)?;
    if old_domain != new_domain {
        return Err("Mailbox rename must keep the same domain".into());
    }
    if old_local == new_local {
        return Ok("local part unchanged".into());
    }
    let src = resolve_mailbox_home(old_address)
        .unwrap_or_else(|| preferred_mailbox_home(&old_domain, &old_local));
    let dest = preferred_mailbox_home(&new_domain, &new_local);
    if src.is_dir() && src != dest {
        if dest.exists() {
            return Err(format!(
                "Destination `{}` already exists; refusing to overwrite",
                dest.display()
            ));
        }
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Cannot create {}: {e}", parent.display()))?;
        }
        fs::rename(&src, &dest).map_err(|e| {
            format!(
                "Cannot move `{}` to `{}`: {e}",
                src.display(),
                dest.display()
            )
        })?;
    } else if !dest.is_dir() {
        fs::create_dir_all(&dest).map_err(|e| format!("Cannot create {}: {e}", dest.display()))?;
    }
    #[cfg(unix)]
    {
        if crate::panel_ops_mailbox_provision::user_exists(&old_local)
            && old_local != new_local
            && !crate::panel_ops_mailbox_provision::user_exists(&new_local)
        {
            let status = Command::new("usermod")
                .args(["-l", &new_local, "-d", &dest.to_string_lossy(), &old_local])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .status()
                .map_err(|e| format!("usermod rename failed: {e}"))?;
            if !status.success() {
                return Err(format!(
                    "usermod rename failed for `{old_local}` -> `{new_local}`"
                ));
            }
        } else {
            ensure_unix_home_dir(&new_local, &dest)?;
        }
        let owner = if crate::panel_ops_mailbox_provision::user_exists(&new_local) {
            new_local.as_str()
        } else {
            old_local.as_str()
        };
        let _ = Command::new("chown")
            .args(["-R", &format!("{owner}:{owner}"), &dest.to_string_lossy()])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    Ok(format!("mailbox home now {}", dest.display()))
}

/// Create a nologin system user whose home is the domain e-mails path (no `/home/<user>`).
pub fn ensure_mailbox_unix_user(user: &str, home: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        let _ = (user, home);
        return Ok(());
    }
    #[cfg(not(windows))]
    {
        if let Some(parent) = home.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Cannot create {}: {e}", parent.display()))?;
        }
        fs::create_dir_all(home).map_err(|e| format!("Cannot create {}: {e}", home.display()))?;
        if crate::panel_ops_mailbox_provision::user_exists(user) {
            return ensure_unix_home_dir(user, home);
        }
        let status = Command::new("useradd")
            .args([
                "-M",
                "-d",
                &home.to_string_lossy(),
                "-s",
                "/sbin/nologin",
                user,
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .status()
            .map_err(|e| format!("useradd failed: {e}"))?;
        if !status.success() {
            return Err(format!("useradd failed for `{user}`"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preferred_path_layout() {
        assert_eq!(
            preferred_mailbox_home("ThreeBrickStudio.com", "Info"),
            PathBuf::from("/home/threebrickstudio.com/e-mails/info")
        );
        assert_eq!(
            preferred_maildir("example.com", "alice"),
            PathBuf::from("/home/example.com/e-mails/alice/Maildir")
        );
    }

    #[test]
    fn detects_domain_emails_vs_legacy() {
        assert!(is_domain_emails_home(Path::new(
            "/home/threebrickstudio.com/e-mails/info"
        )));
        assert!(!is_domain_emails_home(Path::new("/home/info")));
        assert!(is_legacy_top_level_home(Path::new("/home/info"), "info"));
        assert!(!is_legacy_top_level_home(
            Path::new("/home/threebrickstudio.com/e-mails/info"),
            "info"
        ));
    }

    #[test]
    fn split_address_rejects_path_injection() {
        assert!(split_address("a@b.com").is_ok());
        assert!(split_address("a/../x@b.com").is_err());
        assert!(split_address("a@b/../c.com").is_err());
    }
}
