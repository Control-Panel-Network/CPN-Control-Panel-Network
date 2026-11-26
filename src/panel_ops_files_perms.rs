//! Fix permissions for File Manager selections (owner:group + secure modes).
//!
//! # Policy
//!
//! - Website trees under the hosting home (`/home/<fqdn>/...`): resolve the CPN
//!   website Unix account and set `user:group` to that account (never `root`).
//! - Mailbox homes under `/home/<domain>/e-mails/<local>/...`: prefer the mailbox
//!   Unix account when one owns that mailbox home; otherwise the site account.
//! - Non-site paths (outside hosting homes): keep the existing non-root owner and
//!   only normalize modes. Root-owned non-site paths are refused (do not invent
//!   an owner and do not leave/force `root:root` for website content).
//! - Modes: directories `0755`, regular files `0644` (not group/other-writable).
//! - Folders: recursive owner + mode walk. Files: single path only.
//! - Always stays inside the File Manager jail; protected system paths refuse.

use crate::mail_home_layout::EMAILS_DIR;
use crate::panel_ops_path::{is_protected_path, join_child, resolve_under_jail};
use crate::site_unix_owner::{find_unix_user_by_home, mode_is_secure_home};
use crate::sites::{hosting_home_root, list_sites, site_home_from_record};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Soft cap so a runaway tree cannot hang the panel forever.
pub const MAX_FIX_ENTRIES: usize = 100_000;

const DIR_MODE: u32 = 0o755;
const FILE_MODE: u32 = 0o644;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OwnershipFixPolicy {
    /// Set owner and group to this Unix account (`user:user`).
    SetUser { user: String },
    /// Keep current owner/group; only normalize modes (non-site paths).
    ModesOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixPermissionsReport {
    pub fixed: usize,
    pub policy: OwnershipFixPolicy,
    pub recursive: bool,
}

fn running_as_root() -> bool {
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

fn path_owner_ids(path: &Path) -> Result<(u32, u32), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let meta =
            fs::metadata(path).map_err(|e| format!("Could not stat {}: {e}", path.display()))?;
        Ok((meta.uid(), meta.gid()))
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Err("Ownership checks require Unix".into())
    }
}

fn set_mode(path: &Path, mode: u32) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(mode))
            .map_err(|e| format!("chmod {}: {e}", path.display()))?;
    }
    let _ = (path, mode);
    Ok(())
}

fn chown_user_user(path: &Path, user: &str, recursive: bool) -> Result<(), String> {
    if user.trim().is_empty() || user.eq_ignore_ascii_case("root") {
        return Err("Refusing to set ownership to root or an empty account".into());
    }
    let spec = format!("{user}:{user}");
    let path_s = path
        .to_str()
        .ok_or_else(|| "Path is not valid UTF-8".to_string())?;
    let mut args = Vec::new();
    if recursive {
        args.push("-R");
    }
    args.push(spec.as_str());
    args.push(path_s);
    let output = Command::new("chown")
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("chown failed: {e}"))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "chown `{spec}` failed on {}: {}",
            path.display(),
            err.trim().chars().take(160).collect::<String>()
        ));
    }
    Ok(())
}

fn ensure_fixable(path: &Path, jail: &Path) -> Result<(), String> {
    if is_protected_path(path) {
        return Err(format!(
            "Refusing to fix permissions on protected path {}",
            path.display()
        ));
    }
    let jail_s = jail.to_string_lossy();
    let path_s = path.to_string_lossy();
    if path_s == jail_s && jail == Path::new("/") {
        return Err("Refusing to fix permissions on the filesystem root".into());
    }
    if path == hosting_home_root().as_path() {
        return Err(
            "Refusing to fix permissions on the hosting home root; select a site path".into(),
        );
    }
    Ok(())
}

fn path_is_under(path: &Path, root: &Path) -> bool {
    let path_s = path.to_string_lossy().replace('\\', "/");
    let root_s = root.to_string_lossy().replace('\\', "/");
    if path_s == root_s {
        return true;
    }
    let prefix = if root_s.ends_with('/') {
        root_s
    } else {
        format!("{root_s}/")
    };
    path_s.starts_with(&prefix)
}

/// Resolve the CPN ownership policy for a concrete absolute path.
pub fn resolve_ownership_policy(path: &Path) -> Result<OwnershipFixPolicy, String> {
    let home_root = hosting_home_root();
    if path_is_under(path, &home_root) {
        if let Some(user) = resolve_site_or_mailbox_user(path)? {
            if user.eq_ignore_ascii_case("root") {
                return Err("Resolved website Unix account cannot be root".into());
            }
            return Ok(OwnershipFixPolicy::SetUser { user });
        }
        return Err(format!(
            "No CPN website Unix account found for {}. Heal site ownership or select a path under a registered site home.",
            path.display()
        ));
    }

    // Non-site path: modes only, and only when the current owner is not root.
    let (uid, _gid) = path_owner_ids(path)?;
    if uid == 0 {
        return Err(format!(
            "Refusing modes-only fix on root-owned non-site path {}. Select a website path under {} so CPN can apply the site Unix user.",
            path.display(),
            home_root.display()
        ));
    }
    Ok(OwnershipFixPolicy::ModesOnly)
}

fn resolve_site_or_mailbox_user(path: &Path) -> Result<Option<String>, String> {
    // Prefer mailbox Unix account for `/home/<domain>/e-mails/<local>/...`.
    if let Some(mailbox_home) = mailbox_home_for_path(path)
        && let Some(user) = find_unix_user_by_home(&mailbox_home)
    {
        return Ok(Some(user));
    }

    // Registered site whose home contains this path (longest home wins).
    if let Ok(sites) = list_sites() {
        let mut best: Option<(usize, String)> = None;
        for site in sites {
            let home = site_home_from_record(&site);
            if !path_is_under(path, &home) {
                continue;
            }
            let len = home.to_string_lossy().len();
            let user = site
                .unix_user
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .or_else(|| find_unix_user_by_home(&home));
            if let Some(user) = user
                && best.as_ref().is_none_or(|(best_len, _)| len > *best_len)
            {
                best = Some((len, user));
            }
        }
        if let Some((_, user)) = best {
            return Ok(Some(user));
        }
    }

    // Walk up looking for a passwd home match.
    let home_root = hosting_home_root();
    let mut cur = Some(path.to_path_buf());
    while let Some(dir) = cur {
        if dir == home_root {
            break;
        }
        if let Some(user) = find_unix_user_by_home(&dir) {
            return Ok(Some(user));
        }
        cur = dir.parent().map(|p| p.to_path_buf());
    }
    Ok(None)
}

fn mailbox_home_for_path(path: &Path) -> Option<PathBuf> {
    let s = path.to_string_lossy().replace('\\', "/");
    let parts: Vec<&str> = s.split('/').filter(|p| !p.is_empty()).collect();
    // home / <domain> / e-mails / <local> [/ ...]
    if parts.len() >= 4 && parts[0] == "home" && parts[2] == EMAILS_DIR {
        return Some(PathBuf::from(format!(
            "/home/{}/{}/{}",
            parts[1], EMAILS_DIR, parts[3]
        )));
    }
    None
}

fn apply_modes_walk(path: &Path, counter: &mut usize) -> Result<(), String> {
    if *counter >= MAX_FIX_ENTRIES {
        return Err(format!(
            "Permission fix stopped after {MAX_FIX_ENTRIES} entries; narrow the selection"
        ));
    }
    *counter += 1;
    let meta = fs::symlink_metadata(path)
        .map_err(|e| format!("Could not stat {}: {e}", path.display()))?;
    if meta.file_type().is_symlink() {
        // Do not follow or rewrite symlink modes/targets.
        return Ok(());
    }
    if meta.is_dir() {
        set_mode(path, DIR_MODE)?;
        for ent in fs::read_dir(path).map_err(|e| format!("read {}: {e}", path.display()))? {
            let ent = ent.map_err(|e| format!("read entry: {e}"))?;
            apply_modes_walk(&ent.path(), counter)?;
        }
    } else {
        set_mode(path, FILE_MODE)?;
    }
    Ok(())
}

fn fix_one_path(path: &Path, policy: &OwnershipFixPolicy) -> Result<(usize, bool), String> {
    let meta = fs::symlink_metadata(path)
        .map_err(|e| format!("Could not stat {}: {e}", path.display()))?;
    if meta.file_type().is_symlink() {
        return Err(format!(
            "Refusing to fix permissions on symlink {}",
            path.display()
        ));
    }
    let recursive = meta.is_dir();
    match policy {
        OwnershipFixPolicy::SetUser { user } => {
            chown_user_user(path, user, recursive)?;
        }
        OwnershipFixPolicy::ModesOnly => {}
    }
    let mut count = 0usize;
    apply_modes_walk(path, &mut count)?;
    // Spot-check: directory mode must stay secure.
    if recursive {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(path)
                .map_err(|e| format!("Could not re-stat {}: {e}", path.display()))?
                .permissions()
                .mode()
                & 0o777;
            if !mode_is_secure_home(mode) {
                return Err(format!(
                    "Security check failed: directory mode {mode:o} is group/other-writable"
                ));
            }
        }
        if matches!(policy, OwnershipFixPolicy::SetUser { .. }) {
            let (uid, _) = path_owner_ids(path)?;
            if uid == 0 {
                return Err("Security check failed: path still owned by root".into());
            }
        }
    }
    Ok((count, recursive))
}

/// Fix permissions for selected names under `parent` inside `jail`.
pub fn fix_permissions(parent: &str, names: &[String], jail: &Path) -> Result<String, String> {
    if names.is_empty() {
        return Err("Nothing selected".into());
    }
    if !running_as_root() {
        return Err("Fix permissions requires root on the panel host".into());
    }
    let parent = resolve_under_jail(parent, jail)?;
    let mut total = 0usize;
    let mut any_recursive = false;
    let mut set_users: Vec<String> = Vec::new();
    let mut modes_only = 0usize;

    for name in names {
        let target = join_child(&parent, name, jail)?;
        ensure_fixable(&target, jail)?;
        let policy = resolve_ownership_policy(&target)?;
        match &policy {
            OwnershipFixPolicy::SetUser { user } => {
                if !set_users.iter().any(|u| u == user) {
                    set_users.push(user.clone());
                }
            }
            OwnershipFixPolicy::ModesOnly => modes_only += 1,
        }
        let (count, recursive) = fix_one_path(&target, &policy)?;
        total += count;
        any_recursive |= recursive;
    }

    let policy_label = if !set_users.is_empty() && modes_only == 0 {
        format!(
            "owner `{}`",
            set_users
                .iter()
                .map(|u| format!("{u}:{u}"))
                .collect::<Vec<_>>()
                .join(", ")
        )
    } else if set_users.is_empty() && modes_only > 0 {
        "modes only (existing non-root owner kept)".into()
    } else {
        format!(
            "mixed policy (site users {}; {} modes-only)",
            set_users
                .iter()
                .map(|u| format!("{u}:{u}"))
                .collect::<Vec<_>>()
                .join(", "),
            modes_only
        )
    };
    let scope = if any_recursive {
        "recursive for folders"
    } else {
        "selected files"
    };
    Ok(format!(
        "Fixed permissions on {total} path(s) ({scope}; dirs {DIR_MODE:o}, files {FILE_MODE:o}; {policy_label})"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn mailbox_home_parser() {
        assert_eq!(
            mailbox_home_for_path(Path::new("/home/example.com/e-mails/info/Maildir/cur")),
            Some(PathBuf::from("/home/example.com/e-mails/info"))
        );
        assert_eq!(
            mailbox_home_for_path(Path::new("/home/example.com/public_html")),
            None
        );
    }

    #[test]
    fn path_under_helper() {
        assert!(path_is_under(
            Path::new("/home/example.com/public_html"),
            Path::new("/home/example.com")
        ));
        assert!(path_is_under(
            Path::new("/home/example.com"),
            Path::new("/home/example.com")
        ));
        assert!(!path_is_under(
            Path::new("/home/other.com"),
            Path::new("/home/example.com")
        ));
    }

    #[test]
    fn refuse_protected_and_home_root() {
        let jail = Path::new("/");
        assert!(ensure_fixable(Path::new("/etc"), jail).is_err());
        assert!(ensure_fixable(Path::new("/"), jail).is_err());
        let _guard = ENV_LOCK.lock().unwrap();
        // SAFETY: test-only; locked so concurrent tests do not race on this env var.
        unsafe {
            std::env::set_var("CPN_SITES_HOME", "/home");
        }
        assert!(ensure_fixable(Path::new("/home"), jail).is_err());
        unsafe {
            std::env::remove_var("CPN_SITES_HOME");
        }
    }

    #[test]
    fn empty_selection_errors() {
        let err = fix_permissions("/tmp", &[], Path::new("/")).unwrap_err();
        assert!(err.to_ascii_lowercase().contains("nothing"));
    }

    #[test]
    fn join_stays_in_jail() {
        let jail = Path::new("/home/example.com");
        let parent = resolve_under_jail("/home/example.com/public_html", jail).unwrap();
        assert!(join_child(&parent, "..", jail).is_err());
        assert!(join_child(&parent, "ok.txt", jail).is_ok());
    }

    #[test]
    fn secure_modes_match_site_policy() {
        assert!(mode_is_secure_home(DIR_MODE));
        assert!(mode_is_secure_home(FILE_MODE));
        assert!(!mode_is_secure_home(0o775));
        assert!(!mode_is_secure_home(0o666));
    }

    #[test]
    fn policy_enum_labels() {
        let set = OwnershipFixPolicy::SetUser {
            user: "site0001".into(),
        };
        assert!(matches!(set, OwnershipFixPolicy::SetUser { .. }));
        assert_eq!(OwnershipFixPolicy::ModesOnly, OwnershipFixPolicy::ModesOnly);
    }
}
