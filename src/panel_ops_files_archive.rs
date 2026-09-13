//! Compress and extract helpers for File Manager (root or site jail).

use crate::backup_restore_extract::extract_archive_safe;
use crate::panel_ops_path::{
    is_protected_path, join_child, resolve_under_jail, validate_entry_name,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn ensure_writable(path: &Path, jail: &Path) -> Result<(), String> {
    if is_protected_path(path) {
        return Err(format!(
            "Refusing to modify protected path {}",
            path.display()
        ));
    }
    if path.to_string_lossy() == jail.to_string_lossy() {
        return Err("Refusing to modify the File Manager jail root".into());
    }
    Ok(())
}

/// Compress selected names in `parent` into `archive_name` (.zip or .tar.gz).
pub fn compress_entries(
    parent: &str,
    names: &[String],
    archive_name: &str,
    jail: &Path,
) -> Result<String, String> {
    if names.is_empty() {
        return Err("Nothing selected to compress".into());
    }
    let parent = resolve_under_jail(parent, jail)?;
    let archive_name = validate_entry_name(archive_name)?;
    let lower = archive_name.to_ascii_lowercase();
    if !(lower.ends_with(".zip") || lower.ends_with(".tar.gz") || lower.ends_with(".tgz")) {
        return Err("Archive name must end with .zip, .tar.gz, or .tgz".into());
    }
    let dest = join_child(&parent, archive_name, jail)?;
    ensure_writable(&dest, jail)?;
    if dest.exists() {
        return Err("Archive already exists".into());
    }
    for name in names {
        let _ = join_child(&parent, name, jail)?;
    }
    let status = if lower.ends_with(".zip") {
        let mut cmd = Command::new("zip");
        cmd.arg("-r").arg(&dest);
        for name in names {
            cmd.arg(name);
        }
        cmd.current_dir(&parent)
            .status()
            .map_err(|e| format!("zip failed to start (is zip installed?): {e}"))?
    } else {
        let mut cmd = Command::new("tar");
        cmd.arg("-czf").arg(&dest);
        for name in names {
            cmd.arg(name);
        }
        cmd.current_dir(&parent)
            .status()
            .map_err(|e| format!("tar failed to start: {e}"))?
    };
    if !status.success() {
        let _ = fs::remove_file(&dest);
        return Err("Compress command failed".into());
    }
    Ok(format!("Created {}", dest.display()))
}

/// Extract an archive file that lives under `parent` into the same directory.
pub fn extract_entry(parent: &str, archive_name: &str, jail: &Path) -> Result<String, String> {
    let parent = resolve_under_jail(parent, jail)?;
    let archive = join_child(&parent, archive_name, jail)?;
    if !archive.is_file() {
        return Err("Archive file not found".into());
    }
    ensure_writable(&parent, jail)?;
    extract_archive_safe(&archive, &parent)?;
    Ok(format!(
        "Extracted {} into {}",
        archive.display(),
        parent.display()
    ))
}

/// Resolve a path for callers that only need jail validation.
pub fn resolve_path(path: &str, jail: &Path) -> Result<PathBuf, String> {
    resolve_under_jail(path, jail)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_bad_archive_name() {
        let err =
            compress_entries("/tmp", &["a".into()], "out.exe", Path::new("/")).unwrap_err();
        assert!(err.contains("Archive name"));
    }
}
