//! Remove superseded Tachyon version directories after a verified cutover.
//!
//! The webmail self-updater and the CPN release upgrade both install a new
//! `tachyon/v/<version>` directory next to the old one. Upstream never removes the old one,
//! so every update leaves another full copy of the application behind. CPN owns the install
//! layout, so once the newest version is verified healthy this module deletes every older
//! numeric version directory (plus stale `.<version>.cpn-new` staging leftovers).
//!
//! Policy: keep only the active (highest) version. Opt out by creating
//! `/var/lib/cpn/tachyon-keep-old-versions`. Only `tachyon/v/<digits.dots>` entries are ever
//! removed; the data folder (`/var/lib/cpn-webmail`), `include.php`, `index.php` and the
//! admin bridge config are never touched. Symlinks and non-directories are skipped.

use std::path::{Path, PathBuf};

const DOCROOT: &str = "/opt/cpn-webmail/tachyon";
/// Operator opt-out: while this file exists, old version directories are kept.
pub const KEEP_OLD_MARKER: &str = "/var/lib/cpn/tachyon-keep-old-versions";

fn strict_version(name: &str) -> bool {
    !name.is_empty()
        && name.split('.').count() >= 2
        && name
            .split('.')
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

/// Staging directory left behind by an interrupted install (`.<version>.cpn-new`).
fn stale_staging(name: &str) -> bool {
    name.strip_prefix('.')
        .and_then(|rest| rest.strip_suffix(".cpn-new"))
        .map(strict_version)
        .unwrap_or(false)
}

/// Pure planner: which entries of `tachyon/v/` should be removed for `active`.
///
/// Returns nothing unless `active` is a strict version present in `names`. Every other strict
/// version directory (older or not) and every stale staging directory is returned; unknown
/// names are never removed.
pub fn plan_prune(names: &[String], active: &str) -> Vec<String> {
    if !strict_version(active) || !names.iter().any(|n| n == active) {
        return Vec::new();
    }
    let mut out: Vec<String> = names
        .iter()
        .filter(|n| n.as_str() != active && (strict_version(n) || stale_staging(n)))
        .cloned()
        .collect();
    out.sort();
    out
}

fn version_root() -> PathBuf {
    Path::new(DOCROOT).join("tachyon/v")
}

fn list_names(root: &Path) -> Vec<String> {
    let Ok(rd) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    rd.flatten()
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect()
}

fn dir_size(path: &Path) -> u64 {
    let mut total = 0;
    let Ok(rd) = std::fs::read_dir(path) else {
        return 0;
    };
    for ent in rd.flatten() {
        let Ok(ft) = ent.file_type() else { continue };
        if ft.is_dir() {
            total += dir_size(&ent.path());
        } else if ft.is_file() {
            total += ent.metadata().map(|m| m.len()).unwrap_or(0);
        }
    }
    total
}

/// The active version must be complete and serving before anything older is removed.
fn active_version_verified(active: &str) -> Result<(), String> {
    let dir = version_root().join(active);
    if !dir.join("include.php").is_file() {
        return Err(format!("active Tachyon {active} is missing include.php"));
    }
    if !Path::new(DOCROOT).join("index.php").is_file() {
        return Err("Tachyon index.php is missing".into());
    }
    // Admin SSO hooks must already be present in the new tree (re-applied after cutover).
    if crate::tachyon_admin_bridge_install::bridge_needs_heal() {
        return Err("admin SSO bridge is not current yet; keeping old versions".into());
    }
    // Only probe HTTP when Tachyon is the client behind the webmail loopback.
    let active_client = crate::active_webmail::load_active_pref()
        .unwrap_or_else(crate::active_webmail::default_webmail_client);
    if matches!(active_client, crate::model::MailSystem::Tachyon) {
        crate::install_webmail_health::verify_webmail_http_surface("Tachyon")?;
    }
    Ok(())
}

/// Remove superseded Tachyon version directories once the active version is verified.
///
/// Returns a short status line; `Ok` with `skipped:` when nothing was done and why.
pub fn prune_old_versions() -> Result<String, String> {
    if !cfg!(unix) {
        return Ok("skipped: unsupported platform".into());
    }
    if Path::new(KEEP_OLD_MARKER).is_file() {
        return Ok("skipped: old versions kept by operator marker".into());
    }
    let root = version_root();
    if !root.is_dir() {
        return Ok("skipped: Tachyon not installed".into());
    }
    let Some(active) = crate::install_snappymail_repo::detect_app_version(DOCROOT, "tachyon")
    else {
        return Ok("skipped: no Tachyon version directory".into());
    };
    let doomed = plan_prune(&list_names(&root), &active);
    if doomed.is_empty() {
        return Ok(format!("Tachyon {active} is the only version"));
    }
    active_version_verified(&active)?;

    let mut removed: Vec<String> = Vec::new();
    let mut freed: u64 = 0;
    let mut errors: Vec<String> = Vec::new();
    for name in &doomed {
        let path = root.join(name);
        // Re-check at removal time: never follow a symlink or leave the version root.
        match std::fs::symlink_metadata(&path) {
            Ok(m) if m.is_dir() && !m.file_type().is_symlink() => {}
            _ => continue,
        }
        let size = dir_size(&path);
        match std::fs::remove_dir_all(&path) {
            Ok(()) => {
                freed += size;
                removed.push(name.clone());
            }
            Err(e) => errors.push(format!("{name}: {e}")),
        }
    }
    if !removed.is_empty() {
        // Drop opcode cache entries that referenced the removed trees.
        let _ = std::process::Command::new("systemctl")
            .args(["reload", "php-fpm"])
            .status();
    }
    if !errors.is_empty() {
        return Err(format!(
            "Tachyon cleanup incomplete (removed {}): {}",
            removed.join(", "),
            errors.join("; ")
        ));
    }
    Ok(format!(
        "Tachyon cleanup: kept {active}, removed {} ({} KiB freed)",
        removed.join(", "),
        freed / 1024
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn keeps_only_active_version() {
        let plan = plan_prune(&v(&["4.2.3", "4.8.0", "4.7.1"]), "4.8.0");
        assert_eq!(plan, v(&["4.2.3", "4.7.1"]));
    }

    #[test]
    fn nothing_to_do_for_single_version() {
        assert!(plan_prune(&v(&["4.8.0"]), "4.8.0").is_empty());
    }

    #[test]
    fn refuses_when_active_missing_or_invalid() {
        assert!(plan_prune(&v(&["4.2.3"]), "4.8.0").is_empty());
        assert!(plan_prune(&v(&["4.2.3", "x.y"]), "x.y").is_empty());
        assert!(plan_prune(&v(&["4.2.3", "4.8.0"]), "").is_empty());
    }

    #[test]
    fn never_removes_unknown_names() {
        let plan = plan_prune(
            &v(&["4.8.0", "custom", "4.8.0-beta", "data", "4.2.3"]),
            "4.8.0",
        );
        assert_eq!(plan, v(&["4.2.3"]));
    }

    #[test]
    fn removes_stale_staging_dirs() {
        let plan = plan_prune(&v(&["4.8.0", ".4.9.0.cpn-new", ".junk.cpn-new"]), "4.8.0");
        assert_eq!(plan, v(&[".4.9.0.cpn-new"]));
    }

    #[test]
    fn marker_path_is_stable() {
        assert_eq!(KEEP_OLD_MARKER, "/var/lib/cpn/tachyon-keep-old-versions");
    }
}
