//! Scan preferred and fallback directories for restoreable backup archives.
//!
//! Preferred paths are site/panel `backups/` folders. Operators may also drop
//! archives under common roots (`/`, `/root`, `/home`, `/home/cpn/backups`,
//! `/var/lib/cpn/backups`). Listing reports provenance so restore can copy or
//! open the chosen file safely.

use crate::backups::{BackupScope, list_backup_files, resolve_archive_dir};
use crate::paths::{legacy_panel_backups_dir, panel_backups_dir};
use crate::sites::{list_sites, site_backups_dir};
use std::fs;
use std::path::{Path, PathBuf};

/// One archive discovered for restore listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreArchiveHit {
    /// Display filename (basename).
    pub name: String,
    pub size: u64,
    /// Absolute directory containing the file.
    pub dir: PathBuf,
    /// Absolute path to the archive.
    pub path: PathBuf,
    /// Short label for UI provenance (Preferred / Panel / Fallback / ...).
    pub provenance: String,
    /// Whether this is the preferred scope folder for the current request.
    pub preferred: bool,
}

fn is_archive_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".tar.gz")
        || lower.ends_with(".tgz")
        || lower.ends_with(".tar")
        || lower.ends_with(".zip")
        || lower.starts_with("backup-")
        || lower.starts_with("cpmove-")
}

fn push_dir_hits(
    dir: &Path,
    provenance: &str,
    preferred: bool,
    out: &mut Vec<RestoreArchiveHit>,
    seen: &mut std::collections::HashSet<PathBuf>,
) {
    if !dir.is_dir() {
        return;
    }
    for (name, size) in list_backup_files(dir) {
        if !is_archive_name(&name) {
            continue;
        }
        let path = dir.join(&name);
        let canon = fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
        if !seen.insert(canon.clone()) {
            continue;
        }
        out.push(RestoreArchiveHit {
            name,
            size,
            dir: dir.to_path_buf(),
            path,
            provenance: provenance.to_string(),
            preferred,
        });
    }
}

/// Shallow scan of a directory (non-recursive) for archive-like files.
fn scan_shallow(
    dir: &Path,
    provenance: &str,
    out: &mut Vec<RestoreArchiveHit>,
    seen: &mut std::collections::HashSet<PathBuf>,
) {
    push_dir_hits(dir, provenance, false, out, seen);
}

/// Optional one-level recursion under `/home` for `*/backups` and loose archives.
fn scan_home_tree(out: &mut Vec<RestoreArchiveHit>, seen: &mut std::collections::HashSet<PathBuf>) {
    let home = Path::new("/home");
    if !home.is_dir() {
        return;
    }
    let Ok(entries) = fs::read_dir(home) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        // Skip huge application trees; only look at backups dirs and top-level archives.
        let backups = path.join("backups");
        push_dir_hits(&backups, &format!("/home/{name}/backups"), false, out, seen);
        scan_shallow(&path, &format!("/home/{name}"), out, seen);
        // Subdomain layout: /home/<parent>/<sub.fqdn>/backups
        if let Ok(children) = fs::read_dir(&path) {
            for child in children.flatten() {
                let child_path = child.path();
                if !child_path.is_dir() {
                    continue;
                }
                let child_name = child.file_name().to_string_lossy().to_string();
                let sub_backups = child_path.join("backups");
                push_dir_hits(
                    &sub_backups,
                    &format!("/home/{name}/{child_name}/backups"),
                    false,
                    out,
                    seen,
                );
            }
        }
    }
}

/// Documented upload locations shown on the Restore UI (always, even if empty).
pub fn documented_upload_locations() -> Vec<(&'static str, &'static str)> {
    vec![
        ("Preferred (site)", "/home/<domain>/backups/"),
        (
            "Preferred (subdomain)",
            "/home/<parent>/<sub.fqdn>/backups/",
        ),
        (
            "Panel archives",
            "/home/cpn-panel/backups/ (legacy: /var/lib/cpn/backups/)",
        ),
        (
            "Operator drop (also scanned)",
            "/home/cpn/backups/, /root/, /home/, and / (top-level archives only)",
        ),
    ]
}

/// List archives for restore: preferred scope dir first, then fallbacks.
pub fn list_restore_archives_with_fallback(
    scope: &str,
    domain: &str,
) -> Result<(Option<String>, Vec<RestoreArchiveHit>), String> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut preferred_display: Option<String> = None;

    // Preferred path when scope/domain resolve.
    let parsed = BackupScope::parse(scope)?;
    match resolve_archive_dir(parsed, domain) {
        Ok((dir, display)) => {
            preferred_display = Some(display.clone());
            push_dir_hits(&dir, &display, true, &mut out, &mut seen);
        }
        Err(_) => {
            // Domain missing is OK for panel-wide / recreate flows; still scan fallbacks.
        }
    }

    // Known panel / legacy locations.
    let panel = panel_backups_dir();
    push_dir_hits(
        &panel,
        &format!("Panel ({})", panel.display()),
        preferred_display
            .as_ref()
            .map(|p| p == &panel.display().to_string())
            .unwrap_or(false),
        &mut out,
        &mut seen,
    );
    let legacy = legacy_panel_backups_dir();
    if legacy != panel {
        push_dir_hits(
            &legacy,
            &format!("Legacy panel ({})", legacy.display()),
            false,
            &mut out,
            &mut seen,
        );
    }

    // Registered site backups (even when another domain was selected).
    if let Ok(sites) = list_sites() {
        for site in sites {
            let dir = site_backups_dir(&site);
            push_dir_hits(
                &dir,
                &format!("Site {} ({})", site.domain, dir.display()),
                false,
                &mut out,
                &mut seen,
            );
        }
    }

    // Operator drop locations (Unix labs / production).
    if !cfg!(windows) {
        scan_shallow(
            Path::new("/home/cpn/backups"),
            "/home/cpn/backups",
            &mut out,
            &mut seen,
        );
        scan_shallow(Path::new("/root"), "/root", &mut out, &mut seen);
        scan_shallow(Path::new("/"), "/", &mut out, &mut seen);
        scan_home_tree(&mut out, &mut seen);
    }

    // Preferred first, then larger/newer names.
    out.sort_by(|a, b| match (b.preferred, a.preferred) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => b.name.cmp(&a.name),
    });

    Ok((preferred_display, out))
}

/// Resolve an archive by basename under preferred + fallback scan (path traversal safe).
pub fn find_restore_archive(
    scope: &str,
    domain: &str,
    archive_name: &str,
) -> Result<RestoreArchiveHit, String> {
    let name = archive_name.trim();
    if name.is_empty() || name.contains('/') || name.contains('\\') || name.contains("..") {
        return Err("Archive name must be a single filename (no path).".into());
    }
    let (_pref, hits) = list_restore_archives_with_fallback(scope, domain)?;
    hits.into_iter().find(|h| h.name == name).ok_or_else(|| {
        format!("Archive `{name}` was not found under preferred or fallback upload locations.")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;
    use std::io::Write;

    #[test]
    fn documents_preferred_and_fallback_paths() {
        let docs = documented_upload_locations();
        assert!(
            docs.iter()
                .any(|(_, p)| p.contains("/home/<domain>/backups"))
        );
        assert!(docs.iter().any(|(_, p)| p.contains("/var/lib/cpn/backups")));
        assert!(
            docs.iter()
                .any(|(label, _)| label.contains("Operator drop"))
        );
    }

    #[test]
    fn finds_archive_in_temp_panel_dir() {
        with_test_data_dir(|| {
            let dir = panel_backups_dir();
            fs::create_dir_all(&dir).unwrap();
            let file = dir.join("panel-demo-1.tar.gz");
            let mut f = fs::File::create(&file).unwrap();
            writeln!(f, "demo").unwrap();
            let hit = find_restore_archive("panel", "", "panel-demo-1.tar.gz").unwrap();
            assert!(hit.preferred || hit.path.ends_with("panel-demo-1.tar.gz"));
            assert_eq!(hit.name, "panel-demo-1.tar.gz");
        });
    }

    #[test]
    fn rejects_path_traversal_names() {
        assert!(find_restore_archive("panel", "", "../x.tar.gz").is_err());
        assert!(find_restore_archive("panel", "", "a/b.tar.gz").is_err());
    }
}
