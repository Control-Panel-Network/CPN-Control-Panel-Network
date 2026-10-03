//! Site-app backups under `/home/<domain>/backups/apps/<app>/`.

use crate::sites::{SiteRecord, site_home_from_record};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug, Clone)]
pub struct AppBackup {
    pub id: String,
    pub path: PathBuf,
    pub bytes: u64,
}

fn safe_component(raw: &str) -> Result<String, String> {
    let value = raw.trim();
    if value.is_empty()
        || value.len() > 80
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
    {
        return Err("Backup name contains unsupported characters".into());
    }
    Ok(value.to_string())
}

fn backup_root(site: &SiteRecord, app: &str) -> Result<PathBuf, String> {
    let home = site_home_from_record(site);
    let root = home.join("backups").join("apps").join(safe_component(app)?);
    let mut current = home;
    for part in ["backups", "apps", app] {
        current.push(part);
        if let Ok(meta) = fs::symlink_metadata(&current)
            && meta.file_type().is_symlink()
        {
            return Err(format!(
                "Refusing app backup through symlink {}",
                current.display()
            ));
        }
    }
    Ok(root)
}

fn host_backup_root(app: &str) -> Result<PathBuf, String> {
    Ok(crate::account::data_dir()
        .join("backups")
        .join("apps")
        .join(safe_component(app)?))
}

fn timestamp() -> String {
    Command::new("date")
        .args(["-u", "+%Y%m%d-%H%M%S"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()
        .filter(|out| out.status.success())
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| crate::account::now_unix().to_string())
}

pub fn list_app_backups(site: &SiteRecord, app: &str) -> Result<Vec<AppBackup>, String> {
    let root = backup_root(site, app)?;
    let Ok(entries) = fs::read_dir(&root) else {
        return Ok(Vec::new());
    };
    let mut backups = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if entry
            .file_type()
            .map(|kind| !kind.is_file() || kind.is_symlink())
            .unwrap_or(true)
        {
            continue;
        }
        let Some(name) = path.file_name().and_then(|v| v.to_str()) else {
            continue;
        };
        if !name.ends_with(".tar.gz") || safe_component(name).is_err() {
            continue;
        }
        let bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
        backups.push(AppBackup {
            id: name.to_string(),
            path,
            bytes,
        });
    }
    backups.sort_by(|a, b| b.id.cmp(&a.id));
    Ok(backups)
}

pub fn create_app_backup(
    site: &SiteRecord,
    app: &str,
    version: &str,
    source: &Path,
) -> Result<AppBackup, String> {
    if !source.is_dir() {
        return Err(format!("App path {} does not exist", source.display()));
    }
    let root = backup_root(site, app)?;
    fs::create_dir_all(&root).map_err(|e| format!("Could not create app backup directory: {e}"))?;
    let version = safe_component(if version.trim().is_empty() {
        "unknown"
    } else {
        version
    })?;
    let id = format!("{}-{version}.tar.gz", timestamp());
    let path = root.join(&id);
    let tmp = root.join(format!(".{id}.tmp"));
    let status = Command::new("tar")
        .args(["-czf"])
        .arg(&tmp)
        .args(["-C"])
        .arg(source)
        .arg(".")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .status()
        .map_err(|e| format!("Could not start tar: {e}"))?;
    if !status.success() {
        let _ = fs::remove_file(&tmp);
        return Err("Could not create app backup archive".into());
    }
    fs::rename(&tmp, &path).map_err(|e| format!("Could not finalize app backup: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    let bytes = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    Ok(AppBackup { id, path, bytes })
}

pub fn create_host_app_backup(
    app: &str,
    version: &str,
    paths: &[&str],
) -> Result<AppBackup, String> {
    let root = host_backup_root(app)?;
    fs::create_dir_all(&root)
        .map_err(|e| format!("Could not create host app backup directory: {e}"))?;
    let version = safe_component(if version.trim().is_empty() {
        "unknown"
    } else {
        version
    })?;
    let id = format!("{}-{version}.tar.gz", timestamp());
    let path = root.join(&id);
    let tmp = root.join(format!(".{id}.tmp"));
    let existing = paths
        .iter()
        .map(|value| value.trim_start_matches('/'))
        .filter(|value| !value.is_empty() && Path::new("/").join(value).exists())
        .collect::<Vec<_>>();
    if existing.is_empty() {
        return Err(format!("No {app} files were found to back up"));
    }
    let status = Command::new("tar")
        .args(["-czf"])
        .arg(&tmp)
        .args(["-C", "/"])
        .args(&existing)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .status()
        .map_err(|e| format!("Could not start tar: {e}"))?;
    if !status.success() {
        let _ = fs::remove_file(&tmp);
        return Err("Could not create host app backup archive".into());
    }
    fs::rename(&tmp, &path).map_err(|e| format!("Could not finalize host app backup: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    let bytes = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    Ok(AppBackup { id, path, bytes })
}

pub fn list_host_app_backups(app: &str) -> Result<Vec<AppBackup>, String> {
    let root = host_backup_root(app)?;
    let Ok(entries) = fs::read_dir(&root) else {
        return Ok(Vec::new());
    };
    let mut backups = entries
        .flatten()
        .filter_map(|entry| {
            if entry.file_type().ok()?.is_symlink() || !entry.file_type().ok()?.is_file() {
                return None;
            }
            let path = entry.path();
            let id = path.file_name()?.to_str()?.to_string();
            if !id.ends_with(".tar.gz") || safe_component(&id).is_err() {
                return None;
            }
            Some(AppBackup {
                id,
                bytes: entry.metadata().map(|m| m.len()).unwrap_or(0),
                path,
            })
        })
        .collect::<Vec<_>>();
    backups.sort_by(|a, b| b.id.cmp(&a.id));
    Ok(backups)
}

fn validate_archive_members(path: &Path) -> Result<(), String> {
    let output = Command::new("tar")
        .args(["-tzf"])
        .arg(path)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map_err(|e| format!("Could not inspect app backup: {e}"))?;
    if !output.status.success() {
        return Err("App backup archive is unreadable".into());
    }
    let listing = String::from_utf8_lossy(&output.stdout);
    for member in listing.lines() {
        let member = member.trim().replace('\\', "/");
        if member.starts_with('/')
            || member.split('/').any(|part| part == "..")
            || member.contains('\0')
        {
            return Err("App backup contains an unsafe path and was not restored".into());
        }
    }
    Ok(())
}

pub fn restore_host_app_backup(app: &str, id: Option<&str>) -> Result<AppBackup, String> {
    let backups = list_host_app_backups(app)?;
    let selected = match id
        .map(str::trim)
        .filter(|v| !v.is_empty() && *v != "latest")
    {
        None => backups.first().cloned(),
        Some(wanted) => {
            safe_component(wanted)?;
            backups.into_iter().find(|item| item.id == wanted)
        }
    }
    .ok_or_else(|| format!("No selected backup exists for {app}"))?;
    validate_archive_members(&selected.path)?;
    let status = Command::new("tar")
        .args(["--no-same-owner", "--no-same-permissions", "-xzf"])
        .arg(&selected.path)
        .args(["-C", "/"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .status()
        .map_err(|e| format!("Could not start host app restore: {e}"))?;
    if !status.success() {
        return Err("Host app backup could not be restored".into());
    }
    Ok(selected)
}

fn selected_backup(site: &SiteRecord, app: &str, id: Option<&str>) -> Result<AppBackup, String> {
    let backups = list_app_backups(site, app)?;
    if backups.is_empty() {
        return Err(format!("No backups exist for {app}"));
    }
    match id
        .map(str::trim)
        .filter(|v| !v.is_empty() && *v != "latest")
    {
        None => Ok(backups[0].clone()),
        Some(wanted) => {
            safe_component(wanted)?;
            backups
                .into_iter()
                .find(|item| item.id == wanted)
                .ok_or_else(|| "Selected app backup was not found".to_string())
        }
    }
}

pub fn restore_app_backup(
    site: &SiteRecord,
    app: &str,
    id: Option<&str>,
    target: &Path,
) -> Result<AppBackup, String> {
    let backup = selected_backup(site, app, id)?;
    validate_archive_members(&backup.path)?;
    let parent = target
        .parent()
        .ok_or_else(|| "App target has no parent directory".to_string())?;
    fs::create_dir_all(parent).map_err(|e| format!("Could not create app parent: {e}"))?;
    let staging = parent.join(format!(".cpn-app-restore-{}", crate::account::now_unix()));
    let rollback = parent.join(format!(".cpn-app-rollback-{}", crate::account::now_unix()));
    fs::create_dir_all(&staging).map_err(|e| format!("Could not create restore staging: {e}"))?;
    let extracted = Command::new("tar")
        .args(["--no-same-owner", "--no-same-permissions", "-xzf"])
        .arg(&backup.path)
        .args(["-C"])
        .arg(&staging)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .status()
        .map_err(|e| format!("Could not start tar restore: {e}"))?;
    if !extracted.success() {
        let _ = fs::remove_dir_all(&staging);
        return Err("Backup archive could not be extracted; current files were not changed".into());
    }
    if target.exists() {
        fs::rename(target, &rollback)
            .map_err(|e| format!("Could not stage current app files for rollback: {e}"))?;
    }
    if let Err(error) = fs::rename(&staging, target) {
        if rollback.exists() {
            let _ = fs::rename(&rollback, target);
        }
        return Err(format!(
            "Could not activate restored files; original files were put back: {error}"
        ));
    }
    if rollback.exists() {
        let _ = fs::remove_dir_all(&rollback);
    }
    Ok(backup)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backup_components_reject_traversal() {
        assert!(safe_component("cmsms").is_ok());
        assert!(safe_component("20261004-010203-2.2.23.tar.gz").is_ok());
        assert!(safe_component("../cmsms").is_err());
        assert!(safe_component("cmsms/backup").is_err());
    }
}
