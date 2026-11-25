//! Apply and verify `user:group` ownership on site homes and docroots.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::account::{passwd_ids, running_as_root};

#[cfg(unix)]
use super::account::{
    ensure_passwd_home, ensure_primary_unix_user, find_unix_user_by_home,
    inherit_unix_user_from_parents, is_apex_site_home, user_exists,
};

/// Result of applying and verifying site home ownership.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnershipApplyReport {
    pub unix_user: String,
    pub home: PathBuf,
    pub docroot: PathBuf,
}

pub(super) fn path_owner_ids(path: &Path) -> Result<(u32, u32), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let meta = fs::metadata(path)
            .map_err(|e| format!("Could not stat {}: {e}", path.display()))?;
        Ok((meta.uid(), meta.gid()))
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Err("Ownership checks require Unix".into())
    }
}

fn path_mode(path: &Path) -> Result<u32, String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let meta = fs::metadata(path)
            .map_err(|e| format!("Could not stat {}: {e}", path.display()))?;
        Ok(meta.permissions().mode() & 0o7777)
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(0o755)
    }
}

/// True when mode is acceptable for a website home (not group/other writable).
pub fn mode_is_secure_home(mode: u32) -> bool {
    let perms = mode & 0o777;
    (perms & 0o022) == 0
}

fn chown_path(path: &Path, user: &str, recursive: bool) -> Result<(), String> {
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

/// Verify home and docroot are owned by `user:user` with a secure home mode.
pub fn verify_site_ownership(home: &Path, docroot: &Path, user: &str) -> Result<(), String> {
    let (want_uid, want_gid) = passwd_ids(user).ok_or_else(|| {
        format!("Website Unix account `{user}` is missing after ownership apply")
    })?;
    if want_uid == 0 {
        return Err("Refusing website ownership as root (uid 0)".into());
    }
    for (label, path) in [("home", home), ("docroot", docroot)] {
        if !path.exists() {
            return Err(format!(
                "Security check failed: site {label} missing at {}",
                path.display()
            ));
        }
        let (uid, gid) = path_owner_ids(path)?;
        if uid != want_uid || gid != want_gid {
            return Err(format!(
                "Security check failed: site {label} is uid:gid {uid}:{gid}, expected {want_uid}:{want_gid} (`{user}:{user}`)"
            ));
        }
    }
    let mode = path_mode(home)?;
    if !mode_is_secure_home(mode) {
        return Err(format!(
            "Security check failed: site home mode {:o} is group/other-writable",
            mode & 0o777
        ));
    }
    if path_owner_ids(home)?.0 == 0 {
        return Err("Security check failed: site home still owned by root".into());
    }
    Ok(())
}

/// Apply `user:user` ownership to the site home inode, docroot tree, and logs dir.
pub fn apply_site_ownership(
    home: &Path,
    docroot: &Path,
    user: &str,
) -> Result<OwnershipApplyReport, String> {
    if user.trim().is_empty() || user.eq_ignore_ascii_case("root") {
        return Err("Website Unix account must not be root or empty".into());
    }
    if !running_as_root() {
        return Err("Applying site ownership requires root".into());
    }
    fs::create_dir_all(home)
        .map_err(|e| format!("Could not create {}: {e}", home.display()))?;
    fs::create_dir_all(docroot)
        .map_err(|e| format!("Could not create {}: {e}", docroot.display()))?;
    set_mode(home, 0o755)?;
    set_mode(docroot, 0o755)?;
    // Home inode only (not -R): preserves mailbox / nested ownership quirks.
    chown_path(home, user, false)?;
    chown_path(docroot, user, true)?;
    let logs = home.join("logs");
    if logs.is_dir() {
        let _ = chown_path(&logs, user, false);
        set_mode(&logs, 0o755)?;
    }
    verify_site_ownership(home, docroot, user)?;
    Ok(OwnershipApplyReport {
        unix_user: user.to_string(),
        home: home.to_path_buf(),
        docroot: docroot.to_path_buf(),
    })
}

/// Ensure Unix user + ownership for a new site home/docroot. Hard-fails when root.
pub fn ensure_owned_site_tree(
    domain: &str,
    home: &Path,
    docroot: &Path,
    preferred_user: Option<&str>,
) -> Result<String, String> {
    #[cfg(not(unix))]
    {
        let _ = (domain, home, docroot, preferred_user);
        return Ok(String::new());
    }
    #[cfg(unix)]
    {
        if !running_as_root() {
            return Ok(preferred_user.unwrap_or("").to_string());
        }
        let inherited = inherit_unix_user_from_parents(home);
        let user = if let Some(u) = preferred_user.map(str::trim).filter(|s| !s.is_empty()) {
            if !user_exists(u) {
                return Err(format!("Preferred website Unix account `{u}` does not exist"));
            }
            if u.eq_ignore_ascii_case("root") {
                return Err("Preferred website Unix account cannot be root".into());
            }
            u.to_string()
        } else if let Some(existing) = find_unix_user_by_home(home) {
            existing
        } else if let Some(parent_user) = inherited {
            parent_user
        } else {
            ensure_primary_unix_user(domain, home)?
        };
        if is_apex_site_home(home) {
            ensure_passwd_home(&user, home)?;
        }
        apply_site_ownership(home, docroot, &user)?;
        Ok(user)
    }
}
