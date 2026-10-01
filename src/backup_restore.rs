//! Restore / import archives into a CPN site (WordPress, cPanel, source control-panel, CPN).

use crate::backup_restore_apply::{
    RestoreApplyOpts, restore_cpanel, restore_cpn, restore_cyberpanel, restore_wordpress,
};
use crate::backup_restore_detect::{BackupFormat, DetectedBackup, detect_from_members};
pub use crate::backup_restore_entities::infer_domain_from_archive_name;
use crate::backup_restore_entities::{
    EntitySelection, EntityStatus, RestoreEntity, discover_entities,
};
use crate::backup_restore_extract::{
    ArchiveKind, archive_kind, extract_archive_safe, list_archive_members,
};
use crate::backup_restore_scan::find_restore_archive;
use crate::sites::{SiteRecord, create_site, ensure_site_directories, load_site};
use std::fs;
use std::path::Path;

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
    /// Explicit confirmation for optional email / docker / DNS / panel-config entities.
    pub confirm_optional_entities: bool,
    /// Selected entity ids from the plan page (`website`, `db:name`, `site:x`, …).
    pub entities: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct RestoreResult {
    pub format: BackupFormat,
    pub detected: DetectedBackup,
    pub message: String,
    pub warnings: Vec<String>,
    pub entity_statuses: Vec<EntityStatus>,
}

fn flag_true(raw: &str) -> bool {
    matches!(raw.trim(), "1" | "true" | "on" | "yes")
}

impl RestoreRequest {
    #[allow(clippy::too_many_arguments)]
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
        confirm_optional_entities: &str,
        entities: Vec<String>,
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
            confirm_optional_entities: flag_true(confirm_optional_entities),
            entities,
        }
    }
}

fn selection_from_request(req: &RestoreRequest) -> EntitySelection {
    EntitySelection::from_form_values(&req.entities)
}

fn opts_from_selection(sel: &EntitySelection) -> RestoreApplyOpts {
    if sel.is_empty() {
        return RestoreApplyOpts::default();
    }
    RestoreApplyOpts {
        restore_website: sel.wants_website(),
        restore_plugins: sel.wants_plugins() || sel.wants_website(),
        database_names: sel.database_names(),
        include_email: sel.wants_email(),
        include_docker: sel.wants_docker(),
        include_dns: sel.wants_dns(),
        include_panel_config: sel.wants_panel_config(),
    }
}

fn ensure_target_site(
    domain: &str,
    req: &RestoreRequest,
    need_files: bool,
) -> Result<SiteRecord, String> {
    match load_site(domain) {
        Ok(site) => {
            if need_files && !req.confirm_overwrite_files {
                return Err(
                    "Confirm overwrite of website files before restoring into an existing domain."
                        .into(),
                );
            }
            Ok(site)
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
            if need_files && !req.confirm_overwrite_files {
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
            create_site(domain, owner, None, None, Some("Created by backup restore"))
        }
    }
}

fn domain_tree_public_html(staging: &Path, domain: &str) -> Option<std::path::PathBuf> {
    let candidate = staging.join(domain).join("public_html");
    if candidate.is_dir() {
        return Some(candidate);
    }
    let candidate = staging.join(domain);
    if candidate.join("index.php").is_file() || candidate.join("index.html").is_file() {
        return Some(candidate);
    }
    None
}

fn build_entity_statuses(
    discovered: &[RestoreEntity],
    sel: &EntitySelection,
    warnings: &[String],
) -> Vec<EntityStatus> {
    let warn_blob = warnings.join(" ").to_ascii_lowercase();
    discovered
        .iter()
        .map(|ent| {
            let selected = sel.is_empty() || sel.contains(&ent.id);
            if !selected {
                return EntityStatus {
                    id: ent.id.clone(),
                    label: ent.label.clone(),
                    status: "skipped",
                    detail: "Not selected.".into(),
                };
            }
            let (status, detail) = if ent.needs_extra_confirm {
                (
                    "noted",
                    "Selected; see restore notes (optional payload not fully auto-applied)."
                        .to_string(),
                )
            } else if ent.id.starts_with("db:") || ent.id == "databases" {
                let name = ent.id.strip_prefix("db:").unwrap_or("databases");
                if warn_blob.contains(&format!("imported sql `{name}"))
                    || warn_blob.contains(&format!("imported sql `{name}.sql"))
                    || warn_blob.contains("imported sql `databases.sql")
                {
                    ("ok", "Imported.".into())
                } else if warn_blob.contains("skipped") && warn_blob.contains(name) {
                    ("skipped", "Filtered or skipped.".into())
                } else if warn_blob.contains("failed") && warn_blob.contains(name) {
                    ("failed", "Import reported a failure; see warnings.".into())
                } else {
                    ("ok", "Processed with format restore.".into())
                }
            } else if ent.id == "website"
                || ent.id.starts_with("site:")
                || ent.id.starts_with("subdomain:")
            {
                ("ok", "Applied when website restore ran.".into())
            } else {
                ("ok", "Processed.".into())
            };
            EntityStatus {
                id: ent.id.clone(),
                label: ent.label.clone(),
                status,
                detail,
            }
        })
        .collect()
}

/// List entities inside an archive without applying a restore.
pub fn plan_restore_entities(
    scope: &str,
    domain: &str,
    archive: &str,
) -> Result<(DetectedBackup, Vec<RestoreEntity>, String), String> {
    let hit = find_restore_archive(scope, domain, archive)?;
    if !hit.path.is_file() {
        return Err(format!(
            "Archive `{}` not found under {}.",
            hit.name,
            hit.dir.display()
        ));
    }
    if archive_kind(&hit.path) == ArchiveKind::Unsupported {
        return Err(
            "Unsupported archive type. Place a .tar.gz / .tgz / .zip under a documented upload location."
                .into(),
        );
    }
    let members = list_archive_members(&hit.path)?;
    let detected = detect_from_members(&hit.name, &members);
    let entities = discover_entities(&hit.name, &members);
    Ok((detected, entities, hit.name))
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
    let discovered = discover_entities(&hit.name, &members);
    let sel = selection_from_request(req);
    if !sel.is_empty()
        && discovered
            .iter()
            .any(|e| e.needs_extra_confirm && sel.contains(&e.id))
        && !req.confirm_optional_entities
    {
        return Err(
            "Optional entities (email / Docker / DNS / panel-config) are selected. Confirm optional entities before continuing."
                .into(),
        );
    }

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
    if domain.is_empty()
        && let Some(inferred) = infer_domain_from_archive_name(&hit.name)
    {
        domain = inferred;
    }
    // Prefer first selected site/subdomain when provided.
    let selected_domains = sel.selected_domains();
    if !selected_domains.is_empty()
        && (domain.is_empty() || !selected_domains.iter().any(|d| d == &domain))
    {
        domain = selected_domains[0].clone();
    }

    let opts = opts_from_selection(&sel);
    let need_files = opts.restore_website;
    let primary = ensure_target_site(&domain, req, need_files)?;

    let wants_sql = match &opts.database_names {
        None => detected.has_sql,
        Some(set) => !set.is_empty() || sel.contains("databases"),
    };
    if wants_sql && !req.confirm_import_databases {
        return Err(
            "Archive includes SQL dumps. Confirm database import before continuing (or deselect databases)."
                .into(),
        );
    }

    let stamp = crate::account::now_unix();
    let staging = hit.dir.join(format!(".restore-staging-{stamp}"));
    let _ = fs::remove_dir_all(&staging);
    fs::create_dir_all(&staging).map_err(|e| format!("Cannot create staging: {e}"))?;

    let outcome = (|| {
        extract_archive_safe(&archive_path, &staging)?;
        ensure_site_directories(&primary.docroot)?;
        let mut warnings = detected.notes.clone();
        warnings.push(format!(
            "Archive source: {} ({})",
            hit.provenance,
            hit.dir.display()
        ));
        if !sel.is_empty() {
            warnings.push(format!(
                "Selected entities: {}.",
                sel.ids.iter().cloned().collect::<Vec<_>>().join(", ")
            ));
        }

        // Create any additional selected domains (recreate-missing with confirm).
        let mut restored_domains = vec![primary.domain.clone()];
        for extra in &selected_domains {
            if extra == &primary.domain {
                continue;
            }
            let site = ensure_target_site(extra, req, need_files)?;
            ensure_site_directories(&site.docroot)?;
            if need_files {
                if let Some(tree) = domain_tree_public_html(&staging, extra) {
                    crate::backup_restore_apply::restore_docroot_from(&tree, &site)?;
                    warnings.push(format!(
                        "Restored domain-specific tree for `{extra}` from archive."
                    ));
                } else {
                    warnings.push(format!(
                        "Created/ensured `{extra}` but no domain-specific public_html tree was found; primary website files map to `{}`.",
                        primary.domain
                    ));
                }
            }
            restored_domains.push(site.domain);
        }

        match format {
            BackupFormat::Cpn => restore_cpn(&staging, &primary, &opts, &mut warnings)?,
            BackupFormat::WordPress => {
                restore_wordpress(&staging, &primary, req.db_name.trim(), &opts, &mut warnings)?
            }
            BackupFormat::Cpanel => restore_cpanel(&staging, &primary, &opts, &mut warnings)?,
            BackupFormat::CyberPanel => {
                restore_cyberpanel(&staging, &primary, &opts, &mut warnings)?
            }
            BackupFormat::Unknown => return Err("Unknown format".into()),
        }

        let entity_statuses = build_entity_statuses(&discovered, &sel, &warnings);
        let domains_msg = restored_domains.join(", ");
        Ok(RestoreResult {
            format,
            detected: detected.clone(),
            message: format!(
                "Restored {} (`{}`) into {} ({})",
                format.label(),
                hit.name,
                domains_msg,
                primary.docroot
            ),
            warnings,
            entity_statuses,
        })
    })();

    let _ = fs::remove_dir_all(&staging);
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infer_domain_reexport_works() {
        assert_eq!(
            infer_domain_from_archive_name("backup-newstargeted.com-10.01.2026_19-33-23.tar.gz")
                .as_deref(),
            Some("newstargeted.com")
        );
    }
}
