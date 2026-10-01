//! Restore / import archives into a CPN site (WordPress, cPanel, source control-panel, CPN).

use crate::backup_restore_apply::{
    restore_cpanel, restore_cpn, restore_cyberpanel, restore_wordpress,
};
use crate::backup_restore_detect::{BackupFormat, DetectedBackup, detect_from_members};
use crate::backup_restore_extract::{
    ArchiveKind, archive_kind, extract_archive_safe, list_archive_members,
};
use crate::backup_restore_scan::find_restore_archive;
use crate::sites::{create_site, ensure_site_directories, load_site};
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
    /// Panel username used as site owner when creating a missing domain.
    pub owner: String,
    /// When set, create the target domain if it does not exist (requires confirm).
    pub create_domain_if_missing: bool,
    /// Explicit confirmation to create a new site/domain from archive metadata.
    pub confirm_create_domain: bool,
    /// Explicit confirmation before overwriting website files in the docroot.
    pub confirm_overwrite_files: bool,
    /// Explicit confirmation before importing SQL / creating databases.
    pub confirm_import_databases: bool,
}

#[derive(Debug, Clone)]
pub struct RestoreResult {
    pub format: BackupFormat,
    pub detected: DetectedBackup,
    pub message: String,
    pub warnings: Vec<String>,
}

fn flag_true(raw: &str) -> bool {
    matches!(raw.trim(), "1" | "true" | "on" | "yes")
}

impl RestoreRequest {
    pub fn from_form_flags(
        scope: String,
        domain: String,
        archive: String,
        format: String,
        db_name: String,
        create_domain_if_missing: &str,
        confirm_create_domain: &str,
        confirm_overwrite_files: &str,
        confirm_import_databases: &str,
    ) -> Self {
        Self {
            scope,
            domain,
            archive,
            format,
            db_name,
            owner: String::new(),
            create_domain_if_missing: flag_true(create_domain_if_missing),
            confirm_create_domain: flag_true(confirm_create_domain),
            confirm_overwrite_files: flag_true(confirm_overwrite_files),
            confirm_import_databases: flag_true(confirm_import_databases),
        }
    }
}

pub fn restore_backup(req: &RestoreRequest) -> Result<RestoreResult, String> {
    let mut domain = req.domain.trim().to_string();
    let hit = find_restore_archive(&req.scope, &domain, &req.archive)?;
    let archive_path = hit.path.clone();
    if !archive_path.is_file() {
        return Err(format!(
            "Archive `{}` not found under {}.",
            hit.name,
            hit.dir.display()
        ));
    }

    if archive_kind(&archive_path) == ArchiveKind::Unsupported {
        return Err(
            "Unsupported archive type. Place a .tar.gz / .tgz / .zip under a documented upload location."
                .into(),
        );
    }

    let members = list_archive_members(&archive_path)?;
    let mut detected = detect_from_members(&hit.name, &members);
    let forced = BackupFormat::parse(&req.format)?;
    let format = if forced == BackupFormat::Unknown {
        detected.format
    } else {
        detected.format = forced;
        forced
    };
    if format == BackupFormat::Unknown {
        return Err(
            "Could not detect archive format. Choose WordPress, cPanel, source control-panel, or CPN."
                .into(),
        );
    }
    if detected.has_wpress && format == BackupFormat::WordPress {
        return Err(
            "All-in-One WP Migration (.wpress) is detected but not imported directly. Export a zip with wp-content plus a .sql dump, then restore that archive."
                .into(),
        );
    }

    // Infer domain from classic backup-* filename when operator left domain empty.
    if domain.is_empty() {
        if let Some(inferred) = infer_domain_from_archive_name(&hit.name) {
            domain = inferred;
        }
    }

    let site = match load_site(&domain) {
        Ok(site) => {
            if !req.confirm_overwrite_files {
                return Err(
                    "Confirm overwrite of website files before restoring into an existing domain."
                        .into(),
                );
            }
            site
        }
        Err(_) => {
            if domain.is_empty() {
                return Err(
                    "Target domain is required (or use an archive named backup-<domain>-...)."
                        .into(),
                );
            }
            if !(req.create_domain_if_missing && req.confirm_create_domain) {
                return Err(format!(
                    "Site `{domain}` does not exist. Enable Create domain if missing and confirm creation, or create the site first."
                ));
            }
            if !req.confirm_overwrite_files {
                return Err(
                    "Confirm website file apply before creating and restoring into a new domain."
                        .into(),
                );
            }
            let owner = if req.owner.trim().is_empty() {
                "cpnowner"
            } else {
                req.owner.trim()
            };
            create_site(
                &domain,
                owner,
                None,
                None,
                Some("Created by backup restore"),
            )?
        }
    };

    if detected.has_sql && !req.confirm_import_databases {
        return Err(
            "Archive includes SQL dumps. Confirm database import before continuing (or strip SQL from the archive)."
                .into(),
        );
    }

    let stamp = crate::account::now_unix();
    let staging = hit.dir.join(format!(".restore-staging-{stamp}"));
    let _ = fs::remove_dir_all(&staging);
    fs::create_dir_all(&staging).map_err(|e| format!("Cannot create staging: {e}"))?;

    let outcome = (|| {
        extract_archive_safe(&archive_path, &staging)?;
        ensure_site_directories(&site.docroot)?;
        let mut warnings = detected.notes.clone();
        warnings.push(format!(
            "Archive source: {} ({})",
            hit.provenance,
            hit.dir.display()
        ));
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
                hit.name,
                site.domain,
                site.docroot
            ),
            warnings,
        })
    })();

    let _ = fs::remove_dir_all(&staging);
    outcome
}

/// `backup-example.com-10.01.2026_19-33-23` -> `example.com`
pub fn infer_domain_from_archive_name(name: &str) -> Option<String> {
    let base = name
        .trim()
        .trim_end_matches(".tar.gz")
        .trim_end_matches(".tgz")
        .trim_end_matches(".tar")
        .trim_end_matches(".zip");
    let rest = base.strip_prefix("backup-")?;
    // Split on date-like suffix `_DD.MM.YYYY` or `-DD.MM.YYYY` / trailing `_HH-MM-SS`.
    // Prefer last segment that looks like a domain (contains a dot).
    let candidates: Vec<&str> = rest
        .split('_')
        .flat_map(|p| p.split('-'))
        .filter(|p| p.contains('.') && p.contains(char::is_alphabetic))
        .collect();
    // Reconstruct domain: backup names are `backup-<domain>-DD.MM.YYYY_HH-MM-SS`
    // so take everything before the first `-DD.` date marker.
    if let Some(idx) = rest.find("-") {
        // Find `-DD.MM.` pattern
        let bytes = rest.as_bytes();
        let mut i = 0;
        while i + 4 < bytes.len() {
            if bytes[i] == b'-'
                && bytes[i + 1].is_ascii_digit()
                && bytes[i + 2].is_ascii_digit()
                && bytes[i + 3] == b'.'
            {
                let domain = rest[..i].trim().to_ascii_lowercase();
                if domain.contains('.') {
                    return Some(domain);
                }
            }
            i += 1;
        }
        let _ = idx;
    }
    candidates
        .into_iter()
        .rev()
        .find(|c| c.matches('.').count() >= 1)
        .map(|c| c.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infer_domain_from_classic_backup_name() {
        assert_eq!(
            infer_domain_from_archive_name(
                "backup-newstargeted.com-10.01.2026_19-33-23.tar.gz"
            )
            .as_deref(),
            Some("newstargeted.com")
        );
        assert_eq!(
            infer_domain_from_archive_name("backup-example.co.uk-01.02.2026_12-00-00").as_deref(),
            Some("example.co.uk")
        );
        assert!(infer_domain_from_archive_name("site-files.zip").is_none());
    }
}
