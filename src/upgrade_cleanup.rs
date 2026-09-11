//! Conservative post-upgrade cleanup of stale CPN packaging/staging only.
//!
//! Never removes websites, docroots, app instances, Docker stacks/volumes,
//! plugins, SSL material, MFA keys, or feature/config prefs. Prefer keep.

use std::fs;
use std::path::{Path, PathBuf};

/// Paths that must never be deleted by upgrade cleanup (logged when matched).
pub fn preserve_path_prefixes() -> Vec<&'static str> {
    vec![
        "/etc/cpn",
        "/var/lib/cpn",
        "/var/lib/cpn-webmail",
        "/home/",
        "/opt/cpn-webmail/snappymail",
        "/opt/cpn-webmail/roundcube",
        "/opt/cpn-webmail/current",
        "/etc/nginx/conf.d/cpn-",
        "/etc/php-fpm.d/cpn-",
        "/usr/lib/systemd/system/cpn-",
        "/etc/systemd/system/cpn-",
    ]
}

/// Allowlisted exact files that may be removed when present.
fn removable_exact_files() -> Vec<&'static str> {
    vec![
        "/tmp/cpn-installer-status.json",
        "/usr/bin/cpn-installer.bak",
        "/usr/bin/cpn-installer.old",
        "/usr/bin/cpn-installer~",
        "/usr/bin/cpn.bak",
        "/usr/bin/cpn.old",
        "/usr/bin/cpn~",
    ]
}

/// Allowlisted directory name prefixes under `/var/tmp` (CPN staging only).
fn removable_var_tmp_prefixes() -> Vec<&'static str> {
    vec!["cpn-upgrade-", "cpn-gpg-", "cpn-install-", "cpn-release-"]
}

#[derive(Debug, Clone, Default)]
pub struct CleanupReport {
    pub removed: Vec<String>,
    pub skipped_preserved: Vec<String>,
    pub notes: Vec<String>,
}

fn is_preserved(path: &Path) -> bool {
    let text = path.to_string_lossy();
    preserve_path_prefixes().iter().any(|prefix| {
        text == *prefix
            || text.starts_with(&format!("{prefix}/"))
            || text.starts_with(prefix)
    })
}

fn remove_path(path: &Path, report: &mut CleanupReport) {
    if is_preserved(path) {
        report
            .skipped_preserved
            .push(format!("kept (preserve rule): {}", path.display()));
        return;
    }
    if !path.exists() {
        return;
    }
    let result = if path.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    };
    match result {
        Ok(()) => report.removed.push(path.display().to_string()),
        Err(error) => report.notes.push(format!(
            "could not remove {}: {error}",
            path.display()
        )),
    }
}

fn clean_var_tmp_staging(report: &mut CleanupReport) {
    let root = Path::new("/var/tmp");
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !removable_var_tmp_prefixes()
            .iter()
            .any(|prefix| name.starts_with(prefix))
        {
            continue;
        }
        remove_path(&entry.path(), report);
    }
}

fn clean_var_cache_cpn_staging(report: &mut CleanupReport) {
    // Only remove known staging subdirs; never wipe the whole cache tree blindly.
    let staging = [
        "/var/cache/cpn/staging",
        "/var/cache/cpn/downloads",
        "/var/cache/cpn/tmp",
    ];
    for path in staging {
        let p = PathBuf::from(path);
        if p.is_dir() {
            remove_path(&p, report);
        }
    }
}

/// Remove obsolete versioned extract dirs under `/opt/cpn-webmail` that are not
/// the live `snappymail`, `roundcube`, or `current` trees. Config/data under
/// those live trees and `/var/lib/cpn-webmail` stay untouched.
fn clean_obsolete_webmail_code_trees(report: &mut CleanupReport) {
    let root = Path::new("/opt/cpn-webmail");
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    let keep = ["snappymail", "roundcube", "current"];
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if keep.iter().any(|k| *k == name) {
            report
                .skipped_preserved
                .push(format!("kept live webmail tree: {}", entry.path().display()));
            continue;
        }
        // Only remove clearly superseded extract/staging names.
        let obsolete = name.starts_with("snappymail-")
            || name.starts_with("roundcube-")
            || name.starts_with("extract-")
            || name.starts_with("staging-")
            || name.ends_with(".bak")
            || name.ends_with(".old");
        if obsolete {
            remove_path(&entry.path(), report);
        } else {
            report.skipped_preserved.push(format!(
                "kept unknown webmail path (when in doubt keep): {}",
                entry.path().display()
            ));
        }
    }
}

/// Run allowlisted cleanup after a successful package maintenance step.
pub fn cleanup_stale_packaging() -> CleanupReport {
    let mut report = CleanupReport::default();
    if cfg!(windows) {
        report
            .notes
            .push("cleanup skipped on Windows (packaging paths are Linux)".into());
        return report;
    }

    report.notes.push(
        "preserving websites, apps, docker stacks/volumes, plugins, SSL, MFA, and /etc/cpn + /var/lib/cpn configs"
            .into(),
    );

    for path in removable_exact_files() {
        remove_path(Path::new(path), &mut report);
    }
    clean_var_tmp_staging(&mut report);
    clean_var_cache_cpn_staging(&mut report);
    clean_obsolete_webmail_code_trees(&mut report);

    if report.removed.is_empty() {
        report
            .notes
            .push("no stale CPN packaging/staging paths found to remove".into());
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserve_rules_cover_data_and_homes() {
        assert!(is_preserved(Path::new("/var/lib/cpn/sites/example.json")));
        assert!(is_preserved(Path::new("/etc/cpn/listen_port")));
        assert!(is_preserved(Path::new("/home/example.com/public_html")));
        assert!(is_preserved(Path::new("/var/lib/cpn-webmail/snappymail")));
        assert!(!is_preserved(Path::new("/var/tmp/cpn-upgrade-abc")));
        assert!(!is_preserved(Path::new("/tmp/cpn-installer-status.json")));
    }

    #[test]
    fn no_em_or_en_dash_in_notes_template() {
        let report = cleanup_stale_packaging();
        for note in report.notes.iter().chain(report.skipped_preserved.iter()) {
            assert!(!note.contains('\u{2014}'));
            assert!(!note.contains('\u{2013}'));
        }
    }
}
