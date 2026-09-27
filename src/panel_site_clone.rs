//! Clone / staging: copy site files, databases, plugins, and linked Docker notes.

use crate::package_quota::require_site_create_allowed;
use crate::panel_site_staging::{StagingCloneOptions, apply_staging_extras};
use crate::sites::{
    SiteModify, SiteRecord, create_site_with_ssl, hosting_home_root, load_site, modify_site,
    normalize_domain, resolve_parent_domain, site_home_from_record,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn path_under_home(path: &Path) -> Result<(), String> {
    let home = hosting_home_root();
    let canon_home = fs::canonicalize(&home).unwrap_or(home.clone());
    let canon = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if !canon.starts_with(&canon_home) && !path.starts_with(&home) {
        return Err("Path escapes hosting home".into());
    }
    Ok(())
}

fn copy_dir_contents(src: &Path, dst: &Path) -> Result<u64, String> {
    path_under_home(src)?;
    path_under_home(dst)?;
    if !src.is_dir() {
        return Err(format!("Source missing: {}", src.display()));
    }
    fs::create_dir_all(dst).map_err(|e| format!("Could not create {}: {e}", dst.display()))?;

    #[cfg(unix)]
    {
        let status = Command::new("cp")
            .arg("-a")
            .arg(format!("{}/.", src.display()))
            .arg(dst)
            .status()
            .map_err(|e| format!("cp failed: {e}"))?;
        if !status.success() {
            return Err("cp -a failed while cloning site files".into());
        }
        Ok(approx_file_count(dst))
    }
    #[cfg(not(unix))]
    {
        copy_recursive(src, dst)?;
        Ok(approx_file_count(dst))
    }
}

#[cfg(not(unix))]
fn copy_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    for entry in fs::read_dir(src).map_err(|e| format!("read_dir: {e}"))? {
        let entry = entry.map_err(|e| format!("read_dir entry: {e}"))?;
        let ty = entry.file_type().map_err(|e| format!("file_type: {e}"))?;
        let to = dst.join(entry.file_name());
        if ty.is_dir() {
            fs::create_dir_all(&to).map_err(|e| format!("mkdir: {e}"))?;
            copy_recursive(&entry.path(), &to)?;
        } else if ty.is_file() {
            fs::copy(entry.path(), &to).map_err(|e| format!("copy: {e}"))?;
        }
    }
    Ok(())
}

fn approx_file_count(root: &Path) -> u64 {
    let mut n = 0u64;
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in rd.flatten() {
            let Ok(ft) = entry.file_type() else {
                continue;
            };
            if ft.is_dir() {
                stack.push(entry.path());
            } else {
                n += 1;
            }
        }
        if n > 50_000 {
            break;
        }
    }
    n
}

/// Build staging FQDN: `staging.<domain>` when valid, else `staging-<label>.<parent>`.
pub fn suggest_staging_domain(source: &SiteRecord) -> Result<String, String> {
    let candidate = format!("staging.{}", source.domain);
    match normalize_domain(&candidate) {
        Ok(d) => Ok(d),
        Err(_) => {
            let parent =
                resolve_parent_domain(&source.domain)?.unwrap_or_else(|| source.domain.clone());
            let label = source
                .domain
                .split('.')
                .next()
                .unwrap_or("site")
                .chars()
                .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
                .collect::<String>();
            normalize_domain(&format!("staging-{label}.{parent}"))
        }
    }
}

fn resolve_target_domain(
    source: &SiteRecord,
    target_raw: &str,
    use_staging: bool,
) -> Result<String, String> {
    let trimmed = target_raw.trim();
    if use_staging && trimmed.is_empty() {
        return suggest_staging_domain(source);
    }
    if trimmed.is_empty() {
        return Err("Target domain or subdomain label is required".into());
    }
    if !trimmed.contains('.') {
        let label = trimmed.to_ascii_lowercase();
        if !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
            || label.starts_with('-')
            || label.ends_with('-')
        {
            return Err("Subdomain label must be alphanumeric with optional hyphens".into());
        }
        let parent =
            resolve_parent_domain(&source.domain)?.unwrap_or_else(|| source.domain.clone());
        return normalize_domain(&format!("{label}.{parent}"));
    }
    normalize_domain(trimmed)
}

pub struct CloneResult {
    pub domain: String,
    pub docroot: String,
    pub files_copied: u64,
    pub note: String,
    pub production_domain: String,
}

#[derive(Debug, Clone, Copy)]
pub struct CloneOptions {
    pub clone_files: bool,
    pub staging: StagingCloneOptions,
}

impl Default for CloneOptions {
    fn default() -> Self {
        Self {
            clone_files: true,
            staging: StagingCloneOptions::default(),
        }
    }
}

pub fn clone_option_from_form(raw: &str) -> bool {
    !matches!(
        raw.trim().to_ascii_lowercase().as_str(),
        "0" | "false" | "off" | "no"
    )
}

/// Create staging (or custom clone), copy files, databases, plugins; register staging link.
pub fn clone_site_files(
    source: &SiteRecord,
    owner: &str,
    target_raw: &str,
    use_staging: bool,
) -> Result<CloneResult, String> {
    clone_site_with_options(
        source,
        owner,
        target_raw,
        use_staging,
        &CloneOptions::default(),
    )
}

pub fn clone_site_with_options(
    source: &SiteRecord,
    owner: &str,
    target_raw: &str,
    use_staging: bool,
    options: &CloneOptions,
) -> Result<CloneResult, String> {
    let target = resolve_target_domain(source, target_raw, use_staging)?;
    if target.eq_ignore_ascii_case(&source.domain) {
        return Err("Target domain must differ from the source".into());
    }
    if load_site(&target).is_ok() {
        return Err(format!("Site `{target}` already exists"));
    }
    require_site_create_allowed(owner, &target)?;

    let notes = format!(
        "Staging clone of {} (created from Manage > Clone/Staging)",
        source.domain
    );
    let (created, dns_report) = create_site_with_ssl(
        &target,
        owner,
        None,
        source.engine.as_deref(),
        Some(&notes),
        None,
    )?;

    let staging_link = if use_staging && target_raw.trim().is_empty() {
        Some(source.domain.clone())
    } else {
        None
    };
    let created = if staging_link.is_some() {
        modify_site(
            &created.domain,
            SiteModify {
                staging_of: Some(staging_link),
                ..SiteModify::default()
            },
        )?
    } else {
        created
    };

    let src_doc = PathBuf::from(&source.docroot);
    let dst_doc = PathBuf::from(&created.docroot);
    let placeholder = dst_doc.join("index.html");
    if placeholder.is_file() {
        let _ = fs::remove_file(&placeholder);
    }

    let mut files_copied = 0u64;
    if options.clone_files {
        files_copied = copy_dir_contents(&src_doc, &dst_doc)?;
        let src_home = site_home_from_record(source);
        let dst_home = site_home_from_record(&created);
        for name in [
            "wp-config.php",
            ".htaccess",
            "composer.json",
            "package.json",
        ] {
            let from = src_home.join(name);
            if from.is_file() {
                let to = dst_home.join(name);
                let _ = fs::copy(&from, &to);
            }
        }
    } else {
        let placeholder = dst_doc.join("index.html");
        if !placeholder.is_file() {
            let _ = fs::write(
                &placeholder,
                "<!-- Staging site: enable Files in clone options to copy public_html -->\n",
            );
        }
    }

    let extras = apply_staging_extras(source, &created, owner, &options.staging);

    let mut note_parts = vec![if options.clone_files {
        format!("{} file(s) copied.", files_copied)
    } else {
        "Files clone skipped (toggle was off).".into()
    }];
    if options.staging.plugins {
        note_parts.push(if extras.plugins_copied {
            "Site plugins/apps folder copied.".into()
        } else {
            "No site plugins folder to copy (or copy skipped).".into()
        });
    }
    if options.staging.cron {
        if extras.cron_jobs_copied > 0 || !extras.cron_note.is_empty() {
            note_parts.push(extras.cron_note.clone());
        } else {
            note_parts.push("Cron: none copied.".into());
        }
    }
    if options.staging.databases {
        if extras.databases_cloned.is_empty() {
            note_parts.push(
                "No linked MariaDB databases cloned (none registered for this domain or MariaDB unavailable)."
                    .into(),
            );
        } else {
            note_parts.push(format!(
                "Databases cloned: {}.",
                extras.databases_cloned.join("; ")
            ));
        }
    }
    if options.staging.docker && !extras.docker_notes.is_empty() {
        note_parts.push(format!("Docker: {}", extras.docker_notes.join(" ")));
    }
    if !extras.warnings.is_empty() {
        note_parts.push(format!("Warnings: {}", extras.warnings.join(" ")));
    }
    let dns_summary = dns_report.summary();
    if !dns_report.steps.is_empty() || !dns_report.warnings.is_empty() {
        note_parts.push(dns_summary);
    }
    note_parts.push("Promote/sync staging to production is not automated yet (phase 2).".into());

    Ok(CloneResult {
        domain: created.domain,
        docroot: created.docroot,
        files_copied,
        note: note_parts.join(" "),
        production_domain: source.domain.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::{now_unix, with_test_data_dir};
    use crate::sites::create_site;

    #[test]
    fn staging_suggestion_and_clone() {
        with_test_data_dir(|| {
            let home = std::env::temp_dir().join(format!(
                "cpn-clone-{}-{}",
                std::process::id(),
                now_unix()
            ));
            let _ = fs::remove_dir_all(&home);
            fs::create_dir_all(&home).unwrap();
            unsafe {
                std::env::set_var("CPN_SITES_HOME", &home);
            }
            let src = create_site("example.com", "Admin", None, None, None).unwrap();
            fs::write(Path::new(&src.docroot).join("hello.txt"), b"hi").unwrap();
            let sug = suggest_staging_domain(&src).unwrap();
            assert_eq!(sug, "staging.example.com");
            let out = clone_site_files(&src, "Admin", "", true).unwrap();
            assert_eq!(out.domain, "staging.example.com");
            assert!(Path::new(&out.docroot).join("hello.txt").is_file());
            let staged = load_site("staging.example.com").unwrap();
            assert_eq!(staged.staging_of.as_deref(), Some("example.com"));
            unsafe {
                std::env::remove_var("CPN_SITES_HOME");
            }
            let _ = fs::remove_dir_all(&home);
        });
    }
}
