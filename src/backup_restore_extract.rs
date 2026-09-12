//! Safe archive listing and extraction (reject zip-slip / path traversal).

use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveKind {
    TarGz,
    Zip,
    Unsupported,
}

pub fn archive_kind(path: &Path) -> ArchiveKind {
    let name = path
        .file_name()
        .and_then(|v| v.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
        ArchiveKind::TarGz
    } else if name.ends_with(".zip") {
        ArchiveKind::Zip
    } else if name.ends_with(".wpress") {
        ArchiveKind::Unsupported
    } else if name.ends_with(".tar") {
        ArchiveKind::TarGz
    } else {
        ArchiveKind::Unsupported
    }
}

/// Reject absolute paths, Windows drive prefixes, and `..` components.
pub fn is_safe_archive_member(raw: &str) -> bool {
    let trimmed = raw.trim().trim_start_matches("./");
    if trimmed.is_empty() {
        return true;
    }
    if trimmed.contains('\0') {
        return false;
    }
    let path = Path::new(trimmed);
    if path.is_absolute() || path.has_root() {
        return false;
    }
    for comp in path.components() {
        match comp {
            Component::ParentDir => return false,
            Component::Prefix(_) => return false,
            Component::RootDir => return false,
            Component::CurDir | Component::Normal(_) => {}
        }
    }
    // Also reject members that normalize outside via repeated separators tricks.
    let mut depth = 0i32;
    for part in trimmed.replace('\\', "/").split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            return false;
        }
        depth += 1;
        if depth > 512 {
            return false;
        }
    }
    true
}

pub fn validate_member_list(members: &[String]) -> Result<(), String> {
    let mut bad = Vec::new();
    for m in members {
        if !is_safe_archive_member(m) {
            bad.push(m.clone());
            if bad.len() >= 5 {
                break;
            }
        }
    }
    if bad.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "Archive contains unsafe paths (zip-slip blocked): {}",
            bad.join(", ")
        ))
    }
}

pub fn list_archive_members(archive: &Path) -> Result<Vec<String>, String> {
    match archive_kind(archive) {
        ArchiveKind::TarGz => list_tar_members(archive),
        ArchiveKind::Zip => list_zip_members(archive),
        ArchiveKind::Unsupported => Err(format!(
            "Unsupported archive type for `{}`. Use .tar.gz / .tgz / .zip.",
            archive.display()
        )),
    }
}

fn list_tar_members(archive: &Path) -> Result<Vec<String>, String> {
    let name = archive
        .file_name()
        .and_then(|v| v.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let mut cmd = Command::new("tar");
    if name.ends_with(".tar") && !name.ends_with(".tar.gz") {
        cmd.arg("-tf");
    } else {
        cmd.arg("-tzf");
    }
    let out = cmd
        .arg(archive)
        .output()
        .map_err(|e| format!("Could not list tar archive: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "tar list failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect())
}

fn list_zip_members(archive: &Path) -> Result<Vec<String>, String> {
    let out = Command::new("unzip")
        .args(["-Z1"])
        .arg(archive)
        .output()
        .map_err(|e| format!("Could not list zip archive (is unzip installed?): {e}"))?;
    if !out.status.success() {
        // Fallback: unzip -l parse is brittle; prefer -Z1.
        return Err(format!(
            "unzip list failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect())
}

/// Extract into `dest` after validating every member path. Dest must exist.
pub fn extract_archive_safe(archive: &Path, dest: &Path) -> Result<(), String> {
    fs::create_dir_all(dest).map_err(|e| format!("Cannot create extract dir: {e}"))?;
    let members = list_archive_members(archive)?;
    validate_member_list(&members)?;
    match archive_kind(archive) {
        ArchiveKind::TarGz => extract_tar(archive, dest),
        ArchiveKind::Zip => extract_zip(archive, dest),
        ArchiveKind::Unsupported => Err("Unsupported archive type".into()),
    }?;
    reject_escaping_symlinks(dest)?;
    Ok(())
}

fn extract_tar(archive: &Path, dest: &Path) -> Result<(), String> {
    let name = archive
        .file_name()
        .and_then(|v| v.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let mut cmd = Command::new("tar");
    // Avoid absolute-name / overwrite tricks on GNU tar when available.
    cmd.arg("--no-same-owner");
    if name.ends_with(".tar") && !name.ends_with(".tar.gz") {
        cmd.args(["-xf"]);
    } else {
        cmd.args(["-xzf"]);
    }
    let status = cmd
        .arg(archive)
        .arg("-C")
        .arg(dest)
        .status()
        .map_err(|e| format!("Could not start tar extract: {e}"))?;
    if !status.success() {
        return Err("tar extract failed".into());
    }
    Ok(())
}

fn extract_zip(archive: &Path, dest: &Path) -> Result<(), String> {
    // -j would junk paths; we need structure. -n never overwrite outside; still validate first.
    let status = Command::new("unzip")
        .args(["-q", "-o"])
        .arg(archive)
        .arg("-d")
        .arg(dest)
        .status()
        .map_err(|e| format!("Could not start unzip: {e}"))?;
    if !status.success() {
        return Err("unzip extract failed".into());
    }
    Ok(())
}

fn reject_escaping_symlinks(root: &Path) -> Result<(), String> {
    let root_canon = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    walk_check(&root_canon, &root_canon)
}

fn walk_check(root: &Path, current: &Path) -> Result<(), String> {
    let rd = match fs::read_dir(current) {
        Ok(v) => v,
        Err(_) => return Ok(()),
    };
    for ent in rd.flatten() {
        let path = ent.path();
        let ft = ent.file_type().map_err(|e| format!("stat failed: {e}"))?;
        if ft.is_symlink() {
            let target = fs::read_link(&path).map_err(|e| format!("readlink failed: {e}"))?;
            let resolved = if target.is_absolute() {
                target.clone()
            } else {
                path.parent().unwrap_or(root).join(&target)
            };
            let resolved_norm = normalize_logical(&resolved);
            if !resolved_norm.starts_with(root) {
                return Err(format!(
                    "Refusing symlink escape: {} -> {}",
                    path.display(),
                    target.display()
                ));
            }
        } else if ft.is_dir() {
            walk_check(root, &path)?;
        }
    }
    Ok(())
}

fn normalize_logical(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::Prefix(p) => out.push(p.as_os_str()),
            Component::RootDir => out.push(comp.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                let _ = out.pop();
            }
            Component::Normal(seg) => out.push(seg),
        }
    }
    out
}

/// Copy directory tree with `cp -a` when available; fallback recursive copy.
pub fn copy_tree(src: &Path, dest: &Path) -> Result<(), String> {
    if !src.exists() {
        return Ok(());
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir: {e}"))?;
    }
    if src.is_file() {
        fs::copy(src, dest).map_err(|e| format!("copy file: {e}"))?;
        return Ok(());
    }
    #[cfg(unix)]
    {
        let status = Command::new("cp")
            .args(["-a", &src.to_string_lossy(), &dest.to_string_lossy()])
            .status()
            .map_err(|e| format!("cp failed: {e}"))?;
        if status.success() {
            return Ok(());
        }
    }
    copy_tree_fallback(src, dest)
}

fn copy_tree_fallback(src: &Path, dest: &Path) -> Result<(), String> {
    fs::create_dir_all(dest).map_err(|e| format!("mkdir dest: {e}"))?;
    for ent in fs::read_dir(src).map_err(|e| format!("read_dir: {e}"))? {
        let ent = ent.map_err(|e| format!("read_dir entry: {e}"))?;
        let from = ent.path();
        let to = dest.join(ent.file_name());
        let ft = ent.file_type().map_err(|e| e.to_string())?;
        if ft.is_dir() {
            copy_tree_fallback(&from, &to)?;
        } else if ft.is_file() {
            fs::copy(&from, &to).map_err(|e| format!("copy: {e}"))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_zip_slip_members() {
        assert!(!is_safe_archive_member("../etc/passwd"));
        assert!(!is_safe_archive_member("/etc/passwd"));
        assert!(!is_safe_archive_member("foo/../../etc/passwd"));
        assert!(is_safe_archive_member("./public_html/index.html"));
        assert!(is_safe_archive_member("wp-content/uploads/a.jpg"));
    }

    #[test]
    fn validate_list_surfaces_bad_paths() {
        let members = vec!["ok/file.txt".into(), "../evil".into()];
        assert!(validate_member_list(&members).is_err());
    }
}
