//! Restore / import archives into a CPN site (WordPress, cPanel, CyberPanel source, CPN).

use crate::backup_restore_apply::{
    restore_cpanel, restore_cpn, restore_cyberpanel, restore_wordpress,
};
use crate::backup_restore_detect::{BackupFormat, DetectedBackup, detect_from_members};
use crate::backup_restore_extract::{
    archive_kind, extract_archive_safe, list_archive_members, ArchiveKind,
};
use crate::backups::{BackupScope, resolve_archive_dir};
use crate::sites::{ensure_site_directories, load_site};
use std::fs;

#[derive(Debug, Clone)]
pub struct RestoreRequest {
    pub scope: String,
    pub domain: String,
    pub archive: String,
    /// `auto` or a concrete format from [`BackupFormat::parse`].
    pub format: String,
    /// Optional MariaDB database name for WordPress / SQL import.
    pub db_name: String,
}

#[derive(Debug, Clone)]
pub struct RestoreResult {
    pub format: BackupFormat,
    pub detected: DetectedBackup,
    pub message: String,
    pub warnings: Vec<String>,
}

pub fn restore_backup(req: &RestoreRequest) -> Result<RestoreResult, String> {
    let domain = req.domain.trim();
    if domain.is_empty() {
        return Err("Target domain is required for restore / import.".into());
    }
    let site = load_site(domain)?;
    let scope = BackupScope::parse(&req.scope).unwrap_or(BackupScope::Site);
    let (dir, _) = resolve_archive_dir(scope, domain)?;
    let archive_name = sanitize_archive_name(&req.archive)?;
    let archive_path = dir.join(&archive_name);
    if !archive_path.is_file() {
        return Err(format!(
            "Archive `{}` not found under {}.",
            archive_name,
            dir.display()
        ));
    }

    if archive_kind(&archive_path) == ArchiveKind::Unsupported {
        return Err(
            "Unsupported archive type. Place a .tar.gz / .tgz / .zip under the site backups folder."
                .into(),
        );
    }

    let members = list_archive_members(&archive_path)?;
    let mut detected = detect_from_members(&archive_name, &members);
    let forced = BackupFormat::parse(&req.format)?;
    let format = if forced == BackupFormat::Unknown {
        detected.format
    } else {
        detected.format = forced;
        forced
    };
    if format == BackupFormat::Unknown {
        return Err(
            "Could not detect archive format. Choose WordPress, cPanel, CyberPanel, or CPN."
                .into(),
        );
    }
    if detected.has_wpress && format == BackupFormat::WordPress {
        return Err(
            "All-in-One WP Migration (.wpress) is detected but not imported directly. Export a zip with wp-content plus a .sql dump, then restore that archive."
                .into(),
        );
    }

    let stamp = crate::account::now_unix();
    let staging = dir.join(format!(".restore-staging-{stamp}"));
    let _ = fs::remove_dir_all(&staging);
    fs::create_dir_all(&staging).map_err(|e| format!("Cannot create staging: {e}"))?;

    let outcome = (|| {
        extract_archive_safe(&archive_path, &staging)?;
        ensure_site_directories(&site.docroot)?;
        let mut warnings = detected.notes.clone();
        match format {
            BackupFormat::Cpn => restore_cpn(&staging, &site, &mut warnings)?,
            BackupFormat::WordPress => {
                restore_wordpress(&staging, &site, req.db_name.trim(), &mut warnings)?
            }
            BackupFormat::Cpanel => restore_cpanel(&staging, &site, &mut warnings)?,
            BackupFormat::CyberPanel => restore_cyberpanel(&staging, &site, &mut warnings)?,
            BackupFormat::Unknown => return Err("Unknown format".into()),
        }
        Ok(RestoreResult {
            format,
            detected: detected.clone(),
            message: format!(
                "Restored {} (`{}`) into `{}` ({})",
                format.label(),
                archive_name,
                site.domain,
                site.docroot
            ),
            warnings,
        })
    })();

    let _ = fs::remove_dir_all(&staging);
    outcome
}

fn sanitize_archive_name(raw: &str) -> Result<String, String> {
    let name = raw.trim();
    if name.is_empty() {
        return Err("Archive filename is required.".into());
    }
    if name.contains('/') || name.contains('\\') || name.contains("..") {
        return Err("Archive name must be a single filename (no path).".into());
    }
    Ok(name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_rejects_paths() {
        assert!(sanitize_archive_name("../x.tar.gz").is_err());
        assert!(sanitize_archive_name("a/b.tar.gz").is_err());
        assert_eq!(sanitize_archive_name("ok.tar.gz").unwrap(), "ok.tar.gz");
    }
}
