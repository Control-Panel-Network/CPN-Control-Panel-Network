//! Apply owner/mode changes for one Fix permissions target.

use super::{DIR_MODE, FILE_MODE, MAX_FIX_ENTRIES, OwnershipFixPolicy};
use crate::panel_ops_path::is_protected_path;
use crate::site_unix_owner::mode_is_secure_home;
use crate::sites::hosting_home_root;
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

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

pub(super) fn path_owner_ids(path: &Path) -> Result<(u32, u32), String> {
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

pub(super) fn ensure_fixable(path: &Path, jail: &Path) -> Result<(), String> {
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

pub(super) fn fix_one_path(
    path: &Path,
    policy: &OwnershipFixPolicy,
) -> Result<(usize, bool), String> {
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
