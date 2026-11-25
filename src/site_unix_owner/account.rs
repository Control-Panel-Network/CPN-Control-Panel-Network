//! Resolve and create CPN website Unix accounts.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::sites::{SiteRecord, hosting_home_root, site_home_from_record};

const RESERVED: &[&str] = &[
    "root",
    "bin",
    "daemon",
    "adm",
    "lp",
    "sync",
    "shutdown",
    "halt",
    "mail",
    "operator",
    "games",
    "ftp",
    "nobody",
    "systemd-network",
    "dbus",
    "polkitd",
    "sshd",
    "chrony",
    "tcpdump",
    "cpn",
    "cpn-panel",
    "cpn-webmail",
    "mysql",
    "mariadb",
    "nginx",
    "apache",
    "httpd",
    "postfix",
    "dovecot",
    "vmail",
    "docker",
    "lscpd",
    "lsadm",
];

/// Generate a candidate Unix username from a domain (prefix + digits).
pub fn propose_unix_username(domain: &str, salt: u32) -> String {
    let mut prefix: String = domain
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(5)
        .collect::<String>()
        .to_ascii_lowercase();
    if prefix.is_empty() {
        prefix = "site".into();
    }
    if prefix.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        prefix.insert(0, 'u');
    }
    if prefix.len() < 3 {
        prefix = format!("{prefix}cpn");
    }
    let digits = format!("{:04}", salt % 10_000);
    let mut name = format!("{prefix}{digits}");
    if name.len() > 32 {
        name.truncate(32);
    }
    name
}

pub(super) fn is_reserved(name: &str) -> bool {
    RESERVED.iter().any(|r| r.eq_ignore_ascii_case(name))
}

pub(super) fn user_exists(name: &str) -> bool {
    if name.is_empty()
        || name.contains(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
    {
        return false;
    }
    Command::new("getent")
        .args(["passwd", name])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub(super) fn passwd_home(user: &str) -> Option<String> {
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
    line.trim().split(':').nth(5).map(|s| s.to_string())
}

pub(super) fn passwd_ids(user: &str) -> Option<(u32, u32)> {
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
    let mut parts = line.trim().split(':');
    let _name = parts.next()?;
    let _x = parts.next()?;
    let uid: u32 = parts.next()?.parse().ok()?;
    let gid: u32 = parts.next()?.parse().ok()?;
    Some((uid, gid))
}

/// Find an existing system user whose home directory equals `home`.
pub fn find_unix_user_by_home(home: &Path) -> Option<String> {
    let want = home.to_string_lossy();
    let output = Command::new("getent")
        .args(["passwd"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        let mut parts = line.split(':');
        let Some(name) = parts.next() else { continue };
        if name == "root" || is_reserved(name) {
            continue;
        }
        let home_field = parts.nth(4);
        if home_field == Some(want.as_ref()) {
            return Some(name.to_string());
        }
    }
    None
}

pub(super) fn running_as_root() -> bool {
    #[cfg(unix)]
    {
        // SAFETY: geteuid has no preconditions.
        unsafe { libc::geteuid() == 0 }
    }
    #[cfg(not(unix))]
    {
        false
    }
}

fn unique_username(domain: &str) -> Result<String, String> {
    let seed = (crate::account::now_unix() % 10_000) as u32;
    for i in 0..200u32 {
        let candidate = propose_unix_username(domain, seed.wrapping_add(i * 97));
        if is_reserved(&candidate) {
            continue;
        }
        if !user_exists(&candidate) {
            return Ok(candidate);
        }
    }
    Err(format!(
        "Could not allocate a free Unix username for site `{domain}`"
    ))
}

fn create_unix_user(username: &str, home: &Path) -> Result<(), String> {
    if user_exists(username) {
        return Ok(());
    }
    let home_s = home.to_string_lossy().into_owned();
    let output = Command::new("useradd")
        .args([
            "-d",
            &home_s,
            "-M",
            "-U",
            "-s",
            "/usr/sbin/nologin",
            username,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("useradd failed: {e}"))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "useradd failed for site Unix user: {}",
            err.trim().chars().take(180).collect::<String>()
        ));
    }
    Ok(())
}

pub(super) fn ensure_passwd_home(username: &str, home: &Path) -> Result<(), String> {
    let current = passwd_home(username).unwrap_or_default();
    let want = home.to_string_lossy();
    if current == want {
        return Ok(());
    }
    let status = Command::new("usermod")
        .args(["-d", want.as_ref(), username])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .status()
        .map_err(|e| format!("usermod failed: {e}"))?;
    if !status.success() {
        return Err(format!(
            "Could not point Unix user `{username}` home at site directory"
        ));
    }
    Ok(())
}

/// Resolve or create the CPN website Unix account for a primary site home.
pub fn ensure_primary_unix_user(domain: &str, home: &Path) -> Result<String, String> {
    if let Some(existing) = find_unix_user_by_home(home) {
        return Ok(existing);
    }
    if !running_as_root() {
        return Err(
            "Site Unix ownership requires root to create or assign the website Unix account".into(),
        );
    }
    let username = unique_username(domain)?;
    create_unix_user(&username, home)?;
    ensure_passwd_home(&username, home)?;
    Ok(username)
}

/// Resolve the Unix account for a site (subdomains inherit the apex home user).
pub fn resolve_unix_user_for_site(site: &SiteRecord) -> Result<String, String> {
    if let Some(ref stored) = site.unix_user {
        let trimmed = stored.trim();
        if !trimmed.is_empty() && user_exists(trimmed) {
            return Ok(trimmed.to_string());
        }
    }
    let home = site_home_from_record(site);
    if let Some(by_home) = find_unix_user_by_home(&home) {
        return Ok(by_home);
    }
    let root = hosting_home_root();
    let mut cur = home.parent().map(|p| p.to_path_buf());
    while let Some(dir) = cur {
        if dir == root {
            break;
        }
        if let Some(found) = find_unix_user_by_home(&dir) {
            return Ok(found);
        }
        cur = dir.parent().map(|p| p.to_path_buf());
    }
    Err(format!(
        "No CPN website Unix account found for `{}` (home {})",
        site.domain,
        home.display()
    ))
}

#[cfg(unix)]
pub(super) fn inherit_unix_user_from_parents(home: &Path) -> Option<String> {
    let root = hosting_home_root();
    let mut cur = home.parent().map(|p| p.to_path_buf());
    while let Some(dir) = cur {
        if dir == root {
            break;
        }
        if let Some(found) = find_unix_user_by_home(&dir) {
            return Some(found);
        }
        cur = dir.parent().map(|p| p.to_path_buf());
    }
    None
}

#[cfg(unix)]
pub(super) fn is_apex_site_home(home: &Path) -> bool {
    home.parent().is_some_and(|p| p == hosting_home_root())
}

#[allow(dead_code)]
pub(super) fn apex_home_path(user: &str) -> Option<PathBuf> {
    passwd_home(user).map(PathBuf::from)
}
