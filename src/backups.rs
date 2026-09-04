//! Selective backup archives under `/home/.../backups/`.

use crate::paths::panel_backups_dir;
use crate::service_detect::detect_database;
use crate::sites::{load_site, site_backups_dir, site_home_from_record};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupScope {
    Panel,
    Site,
    Subdomain,
}

impl BackupScope {
    pub fn parse(raw: &str) -> Result<Self, String> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "panel" | "panel_config" | "panel-config" => Ok(Self::Panel),
            "site" | "website" => Ok(Self::Site),
            "subdomain" | "sub" => Ok(Self::Subdomain),
            other => Err(format!(
                "Unknown backup scope `{other}`. Use: panel, site, subdomain"
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Panel => "panel",
            Self::Site => "site",
            Self::Subdomain => "subdomain",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct BackupContents {
    pub website_files: bool,
    pub backups_folder: bool,
    pub plugins_folder: bool,
    pub databases: bool,
    pub ftp: bool,
    pub panel_config: bool,
}

impl BackupContents {
    pub fn from_flags(
        panel_config: bool,
        website_files: bool,
        backups_folder: bool,
        plugins_folder: bool,
        databases: bool,
        ftp: bool,
    ) -> Self {
        Self {
            panel_config,
            website_files,
            backups_folder,
            plugins_folder,
            databases,
            ftp,
        }
    }

    pub fn any_selected(&self, scope: BackupScope) -> bool {
        match scope {
            BackupScope::Panel => self.panel_config || self.databases,
            BackupScope::Site | BackupScope::Subdomain => {
                self.website_files
                    || self.backups_folder
                    || self.plugins_folder
                    || self.databases
                    || self.ftp
            }
        }
    }
}

pub fn is_subdomain_site(domain: &str) -> bool {
    crate::sites::parent_domain_candidates(domain)
        .iter()
        .any(|candidate| crate::sites::load_site(candidate).is_ok())
}

pub fn resolve_archive_dir(scope: BackupScope, domain: &str) -> Result<(PathBuf, String), String> {
    match scope {
        BackupScope::Panel => {
            let dir = panel_backups_dir();
            Ok((dir.clone(), dir.display().to_string()))
        }
        BackupScope::Site | BackupScope::Subdomain => {
            if domain.trim().is_empty() {
                return Err("Select a domain for site or subdomain backups.".into());
            }
            let site = load_site(domain)?;
            if scope == BackupScope::Subdomain && !is_subdomain_site(&site.domain) {
                return Err(format!(
                    "`{}` is not registered as a subdomain site.",
                    site.domain
                ));
            }
            if scope == BackupScope::Site && is_subdomain_site(&site.domain) {
                return Err(format!(
                    "`{}` looks like a subdomain. Choose scope Subdomain instead.",
                    site.domain
                ));
            }
            let dir = site_backups_dir(&site);
            Ok((dir.clone(), dir.display().to_string()))
        }
    }
}

pub fn list_backup_files(dir: &Path) -> Vec<(String, u64)> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("")
            .to_string();
        if name.is_empty() {
            continue;
        }
        let size = fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0);
        files.push((name, size));
    }
    files.sort_by(|a, b| b.0.cmp(&a.0));
    files
}

fn flag_true(raw: &str) -> bool {
    matches!(raw.trim(), "1" | "true" | "on" | "yes")
}

#[derive(Debug, Clone)]
pub struct BackupRequest {
    pub scope: String,
    pub domain: String,
    pub panel_config: String,
    pub website_files: String,
    pub backups_folder: String,
    pub plugins_folder: String,
    pub databases: String,
    pub ftp: String,
}

pub fn create_selective_backup(req: &BackupRequest) -> Result<String, String> {
    let scope = BackupScope::parse(&req.scope)?;
    let contents = BackupContents::from_flags(
        flag_true(&req.panel_config),
        flag_true(&req.website_files),
        flag_true(&req.backups_folder),
        flag_true(&req.plugins_folder),
        flag_true(&req.databases),
        flag_true(&req.ftp),
    );
    if flag_true(&req.ftp) {
        return Err(
            "FTP content/users backup is not implemented yet. Leave that checkbox unchecked."
                .into(),
        );
    }
    if !contents.any_selected(scope) {
        return Err("Select at least one contents option.".into());
    }
    let (dir, _) = resolve_archive_dir(scope, &req.domain)?;
    fs::create_dir_all(&dir).map_err(|error| format!("Could not create backups dir: {error}"))?;
    let stamp = crate::account::now_unix();
    let prefix = match scope {
        BackupScope::Panel => "panel".to_string(),
        BackupScope::Site | BackupScope::Subdomain => {
            let safe = req
                .domain
                .trim()
                .chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                        c
                    } else {
                        '_'
                    }
                })
                .collect::<String>();
            format!("{}-{safe}", scope.as_str())
        }
    };
    let name = format!("{prefix}-{stamp}.tar.gz");
    let dest = dir.join(&name);
    let staging = dir.join(format!(".staging-{stamp}"));
    let _ = fs::remove_dir_all(&staging);
    fs::create_dir_all(&staging)
        .map_err(|error| format!("Could not create staging dir: {error}"))?;

    match scope {
        BackupScope::Panel => {
            if contents.panel_config {
                stage_panel_config(&staging)?;
            }
            if contents.databases {
                stage_database_dump(&staging)?;
            }
        }
        BackupScope::Site | BackupScope::Subdomain => {
            let site = load_site(&req.domain)?;
            let home = site_home_from_record(&site);
            if contents.website_files {
                stage_path_copy(&PathBuf::from(&site.docroot), &staging.join("public_html"))?;
            }
            if contents.backups_folder {
                let src = home.join("backups");
                if src.is_dir() {
                    stage_path_copy(&src, &staging.join("backups-copy"))?;
                }
            }
            if contents.plugins_folder {
                let src = home.join("plugins");
                if src.is_dir() {
                    stage_path_copy(&src, &staging.join("plugins"))?;
                }
            }
            if contents.databases {
                stage_database_dump(&staging)?;
            }
        }
    }

    let status = Command::new("tar")
        .arg("-czf")
        .arg(&dest)
        .arg("-C")
        .arg(&staging)
        .arg(".")
        .status()
        .map_err(|error| format!("Could not start tar: {error}"))?;
    let _ = fs::remove_dir_all(&staging);
    if !status.success() {
        let _ = fs::remove_file(&dest);
        return Err("tar backup failed".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&dest, fs::Permissions::from_mode(0o600));
    }
    Ok(name)
}

fn stage_panel_config(staging: &Path) -> Result<(), String> {
    let data = crate::account::data_dir();
    let dest = staging.join("panel-config");
    fs::create_dir_all(&dest).map_err(|e| format!("stage panel config: {e}"))?;
    for rel in [
        "panel-bootstrap.json",
        "accounts",
        "sites",
        "plugin-catalog-cache.json",
        "smtp.json",
        "panel-session.secret",
        "panel-hostname.json",
        "listen-port.json",
        "install-manifest.json",
        "panel-ui.json",
    ] {
        let src = data.join(rel);
        if !src.exists() {
            continue;
        }
        let target = dest.join(rel);
        if src.is_dir() {
            stage_path_copy(&src, &target)?;
        } else {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|e| format!("stage mkdir: {e}"))?;
            }
            fs::copy(&src, &target).map_err(|e| format!("stage copy {rel}: {e}"))?;
        }
    }
    Ok(())
}

fn stage_database_dump(staging: &Path) -> Result<(), String> {
    let db = detect_database();
    if !db.listening_3306 && db.service_label == "Not detected" {
        return Err("No local database detected for dump.".into());
    }
    let dump_path = staging.join("databases.sql");
    let file = fs::File::create(&dump_path).map_err(|e| format!("dump file: {e}"))?;
    let status = Command::new("mysqldump")
        .args(["--all-databases", "--single-transaction", "--routines"])
        .stdout(file)
        .status();
    match status {
        Ok(code) if code.success() => Ok(()),
        Ok(_) => Err("mysqldump failed (check local DB auth / socket access).".into()),
        Err(error) => Err(format!("mysqldump not available: {error}")),
    }
}

fn stage_path_copy(src: &Path, dest: &Path) -> Result<(), String> {
    if !src.exists() {
        return Ok(());
    }
    if src.is_file() {
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("copy mkdir: {e}"))?;
        }
        fs::copy(src, dest).map_err(|e| format!("copy file: {e}"))?;
        return Ok(());
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("copy mkdir: {e}"))?;
    }
    let status = Command::new("cp")
        .args(["-a", &src.to_string_lossy(), &dest.to_string_lossy()])
        .status()
        .map_err(|e| format!("cp failed: {e}"))?;
    if !status.success() {
        return Err(format!("cp -a {} failed", src.display()));
    }
    Ok(())
}

/// Backward-compatible full panel-data backup (panel config only).
pub fn create_panel_backup() -> Result<String, String> {
    create_selective_backup(&BackupRequest {
        scope: "panel".into(),
        domain: String::new(),
        panel_config: "1".into(),
        website_files: "0".into(),
        backups_folder: "0".into(),
        plugins_folder: "0".into(),
        databases: "0".into(),
        ftp: "0".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_scopes() {
        assert_eq!(BackupScope::parse("panel").unwrap(), BackupScope::Panel);
        assert_eq!(BackupScope::parse("website").unwrap(), BackupScope::Site);
        assert!(BackupScope::parse("full").is_err());
    }
}
