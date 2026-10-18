//! Delete a foreign panel's programs and duplicate site trees after a verified copy.

use std::fs;
use std::path::{Path, PathBuf};

use super::apply::{VerifyReport, load_report};
use super::catalog::spec_by_id;
use super::scan::rel;

const PROTECTED_TOP: &[&str] = &[
    "home", "usr", "etc", "var", "opt", "www", "bin", "sbin", "root",
];

pub fn wipe_panel(root: &Path, panel_id: &str) -> Result<Vec<String>, String> {
    let spec = spec_by_id(panel_id).ok_or_else(|| format!("Unknown panel `{panel_id}`"))?;
    let report = load_report(panel_id)?;
    if report.panel_id != spec.id {
        return Err("Verify report does not match this panel".into());
    }
    if !report.verified {
        return Err(format!(
            "Apply for {} did not verify. Wipe is refused.",
            spec.name
        ));
    }
    if report.sites.is_empty() {
        return Err("Verify report has no sites. Wipe is refused.".into());
    }
    let mut removed = Vec::new();
    for raw in &report.delete_paths {
        let path = PathBuf::from(raw);
        if !wipe_allowed(root, &report, &path) {
            return Err(format!("Refusing to delete {}", path.display()));
        }
    }
    for raw in &report.delete_paths {
        let path = PathBuf::from(raw);
        if !path.exists() {
            continue;
        }
        remove_path(&path)?;
        removed.push(path.to_string_lossy().into_owned());
    }
    Ok(removed)
}

fn wipe_allowed(root: &Path, report: &VerifyReport, path: &Path) -> bool {
    if path.as_os_str().is_empty() {
        return false;
    }
    let Some(candidate) = anchored(root, path) else {
        return false;
    };
    let root_anchor = if candidate.starts_with(root) {
        root.to_path_buf()
    } else {
        root.canonicalize().unwrap_or_else(|_| root.to_path_buf())
    };
    if candidate == root_anchor {
        return false;
    }
    let data = crate::account::data_dir();
    if candidate.starts_with(&data) {
        return false;
    }
    for site in &report.sites {
        let dest = PathBuf::from(&site.dest);
        if candidate == dest || dest.starts_with(&candidate) {
            return false;
        }
        if site.adopt_in_place {
            let source = PathBuf::from(&site.source);
            if candidate == source || source.starts_with(&candidate) {
                return false;
            }
        }
    }
    for blocked in ["var/lib/mysql", "var/lib/mariadb", "var/lib/cpn"] {
        let blocked = rel(&root_anchor, blocked);
        if candidate.starts_with(&blocked) {
            return false;
        }
    }
    let Ok(rel_path) = candidate.strip_prefix(&root_anchor) else {
        return false;
    };
    let rel_parts: Vec<String> = rel_path
        .components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect();
    if rel_parts.len() < 2 {
        return false;
    }
    if rel_parts.len() == 2 && PROTECTED_TOP.contains(&rel_parts[0].as_str()) {
        return false;
    }
    true
}

fn anchored(root: &Path, path: &Path) -> Option<PathBuf> {
    if path.starts_with(root) {
        return Some(path.to_path_buf());
    }
    let root_c = root.canonicalize().ok()?;
    let path_c = path.canonicalize().ok()?;
    if path_c.starts_with(&root_c) {
        Some(path_c)
    } else {
        None
    }
}

fn remove_path(path: &Path) -> Result<(), String> {
    let meta = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if meta.file_type().is_symlink() {
        fs::remove_file(path).map_err(|error| format!("Could not unlink {}: {error}", path.display()))
    } else if meta.is_dir() {
        fs::remove_dir_all(path)
            .map_err(|error| format!("Could not remove {}: {error}", path.display()))
    } else {
        fs::remove_file(path).map_err(|error| format!("Could not remove {}: {error}", path.display()))
    }
}
