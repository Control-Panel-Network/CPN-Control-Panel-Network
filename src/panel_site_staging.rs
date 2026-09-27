//! Staging helpers: databases, plugins, cron, Docker stacks, wp-config remap.

use crate::account::{data_dir, now_unix};
use crate::package_quota::require_quota;
use crate::packages::QuotaResource;
use crate::panel_ops_db::{
    create_database_with_user, local_mariadb_ready, mariadb_client_bin, mariadb_dump_bin,
};
use crate::panel_ops_docker_compose::CPN_MANAGED_LABEL;
use crate::panel_ops_site_cron::clone_cron_jobs_to_target;
use crate::panel_site_staging_docker::{clone_stack_for_staging, stacks_linked_to_site};
use crate::plugin_activation::{activate_host_plugin_for_domain, list_activations_for_domain};
use crate::resource_accounts::{create_database, list_databases};
use crate::sites::{SiteRecord, site_home_from_record, site_plugins_dir};
use rand::Rng;
use rand::distr::Alphanumeric;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Copy)]
pub struct StagingCloneOptions {
    pub databases: bool,
    pub plugins: bool,
    pub cron: bool,
    pub docker: bool,
}

impl Default for StagingCloneOptions {
    fn default() -> Self {
        Self {
            databases: true,
            plugins: true,
            cron: true,
            docker: true,
        }
    }
}

pub struct StagingExtras {
    pub databases_cloned: Vec<String>,
    pub plugins_copied: bool,
    pub cron_jobs_copied: usize,
    pub cron_note: String,
    pub docker_notes: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StagingDbMapEntry {
    source_db: String,
    staging_db: String,
    db_user: String,
    #[serde(default)]
    updated_at_unix: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct StagingDbMapFile {
    staging_of: String,
    staging_domain: String,
    entries: Vec<StagingDbMapEntry>,
}

pub fn collect_linked_database_names(source: &SiteRecord) -> Vec<String> {
    let domain = source.domain.to_ascii_lowercase();
    let mut names: Vec<String> = list_databases()
        .into_iter()
        .filter(|d| d.domain.eq_ignore_ascii_case(&domain))
        .map(|d| d.name)
        .collect();
    let docroot = PathBuf::from(&source.docroot);
    if let Some(wp) = guess_db_name_from_wp_config(&docroot.join("wp-config.php")) {
        if !names.iter().any(|n| n.eq_ignore_ascii_case(&wp)) {
            names.push(wp);
        }
    }
    let home_wp = site_home_from_record(source).join("wp-config.php");
    if let Some(wp) = guess_db_name_from_wp_config(&home_wp) {
        if !names.iter().any(|n| n.eq_ignore_ascii_case(&wp)) {
            names.push(wp);
        }
    }
    names.sort();
    names.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    names
}

pub fn apply_staging_extras(
    source: &SiteRecord,
    target: &SiteRecord,
    owner: &str,
    opts: &StagingCloneOptions,
) -> StagingExtras {
    let mut out = StagingExtras {
        databases_cloned: Vec::new(),
        plugins_copied: false,
        cron_jobs_copied: 0,
        cron_note: String::new(),
        docker_notes: Vec::new(),
        warnings: Vec::new(),
    };
    let mut db_map_entries: Vec<StagingDbMapEntry> = Vec::new();

    if opts.plugins {
        let src_plugins = site_plugins_dir(source);
        let dst_plugins = site_plugins_dir(target);
        if src_plugins.is_dir() {
            match copy_tree_best_effort(&src_plugins, &dst_plugins) {
                Ok(()) => out.plugins_copied = true,
                Err(err) => out.warnings.push(format!("Plugins copy skipped: {err}")),
            }
        }
        for act in list_activations_for_domain(&source.domain) {
            if let Err(err) = activate_host_plugin_for_domain(&target.domain, &act.plugin_id) {
                out.warnings.push(format!(
                    "Host plugin `{}` activation on staging skipped: {err}",
                    act.plugin_id
                ));
            }
        }
    }

    if opts.databases {
        for src_db in collect_linked_database_names(source) {
            if let Err(err) = require_quota(owner, QuotaResource::Databases) {
                out.warnings
                    .push(format!("Database `{src_db}` not cloned: {err}"));
                break;
            }
            match clone_one_database(&src_db, owner, &target.domain) {
                Ok(result) => {
                    out.databases_cloned
                        .push(format!("{} -> {}", src_db, result.staging_name));
                    db_map_entries.push(StagingDbMapEntry {
                        source_db: src_db.clone(),
                        staging_db: result.staging_name.clone(),
                        db_user: result.db_user.clone(),
                        updated_at_unix: now_unix(),
                    });
                    let wp_paths = [
                        PathBuf::from(&target.docroot).join("wp-config.php"),
                        site_home_from_record(target).join("wp-config.php"),
                    ];
                    for wp in wp_paths {
                        if wp.is_file()
                            && src_db.eq_ignore_ascii_case(
                                &guess_db_name_from_wp_config(&wp).unwrap_or_default(),
                            )
                        {
                            let _ = remap_wp_config_credentials(
                                &wp,
                                &result.staging_name,
                                &result.db_user,
                                &result.db_password,
                            );
                        }
                    }
                }
                Err(err) => out
                    .warnings
                    .push(format!("Database `{src_db}` clone failed: {err}")),
            }
        }
        if !db_map_entries.is_empty() {
            if let Err(err) = write_db_map_file(source, target, &db_map_entries) {
                out.warnings
                    .push(format!("Could not write staging DB map file: {err}"));
            }
        }
    }

    if opts.cron {
        match clone_cron_jobs_to_target(&source.domain, target) {
            Ok((count, detail)) => {
                out.cron_jobs_copied = count;
                out.cron_note = detail;
            }
            Err(err) => out.warnings.push(format!("Cron clone skipped: {err}")),
        }
    }

    if opts.docker {
        let linked = stacks_linked_to_site(source);
        if linked.is_empty() {
            out.docker_notes.push(
                "No site-linked Docker compose stacks detected (stack id or compose file references this domain or site home)."
                    .into(),
            );
        } else {
            for stack in linked {
                let compose_text = fs::read_to_string(&stack.compose_file).unwrap_or_default();
                if compose_text.contains(CPN_MANAGED_LABEL) {
                    out.docker_notes.push(format!(
                        "Compose stack `{}` uses {} and was left on production (not cloned).",
                        stack.id, CPN_MANAGED_LABEL
                    ));
                    continue;
                }
                match clone_stack_for_staging(&stack, source, target, owner) {
                    Ok(msg) => out.docker_notes.push(msg),
                    Err(err) => out
                        .warnings
                        .push(format!("Docker stack `{}` not cloned: {err}", stack.id)),
                }
            }
        }
    }

    out
}

struct CloneDbResult {
    staging_name: String,
    db_user: String,
    db_password: String,
}

fn write_db_map_file(
    source: &SiteRecord,
    target: &SiteRecord,
    entries: &[StagingDbMapEntry],
) -> Result<(), String> {
    let path = site_home_from_record(target).join(".cpn-staging-db-map.json");
    let file = StagingDbMapFile {
        staging_of: source.domain.clone(),
        staging_domain: target.domain.clone(),
        entries: entries.to_vec(),
    };
    let raw = serde_json::to_string_pretty(&file).map_err(|e| format!("serialize db map: {e}"))?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir: {e}"))?;
    }
    fs::write(&path, raw).map_err(|e| format!("write db map: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn clone_one_database(
    source_db: &str,
    owner: &str,
    registry_domain: &str,
) -> Result<CloneDbResult, String> {
    if !local_mariadb_ready() {
        return Err("Local MariaDB is not available. Install MariaDB from Host packages.".into());
    }
    let staging_name = unique_staging_db_name(source_db, registry_domain)?;
    let db_user = staging_name.clone();
    let db_password = random_db_password();
    create_database_with_user(&staging_name, &db_user, &db_password)?;
    let dump_path = data_dir()
        .join("tmp")
        .join(format!("staging-dump-{source_db}-{}.sql", now_unix()));
    if let Some(parent) = dump_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("tmp dir: {e}"))?;
    }
    dump_single_database(source_db, &dump_path)?;
    import_single_database(&staging_name, &dump_path)?;
    let _ = fs::remove_file(&dump_path);
    let _ = create_database(owner, &staging_name, registry_domain);
    Ok(CloneDbResult {
        staging_name,
        db_user,
        db_password,
    })
}

fn unique_staging_db_name(source_db: &str, target_domain: &str) -> Result<String, String> {
    let base = staging_db_ident(source_db, target_domain);
    let existing: Vec<String> = list_databases().into_iter().map(|d| d.name).collect();
    if !existing.iter().any(|n| n.eq_ignore_ascii_case(&base)) {
        return Ok(base);
    }
    for i in 2..100 {
        let candidate = truncate_db_ident(&format!("{base}_{i}"));
        if !existing.iter().any(|n| n.eq_ignore_ascii_case(&candidate)) {
            return Ok(candidate);
        }
    }
    Err(format!(
        "Could not allocate staging database name for `{source_db}`"
    ))
}

fn staging_db_ident(source_db: &str, target_domain: &str) -> String {
    let from_source = format!("stg_{source_db}");
    if from_source.len() <= 64 && source_db.len() <= 48 {
        return truncate_db_ident(&from_source);
    }
    truncate_db_ident(&domain_db_slug(target_domain))
}

fn domain_db_slug(domain: &str) -> String {
    let mut base: String = domain
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    while base.contains("__") {
        base = base.replace("__", "_");
    }
    base = base.trim_matches('_').to_string();
    if base.is_empty() {
        base = "site".into();
    }
    truncate_db_ident(&format!("stg_{base}"))
}

fn truncate_db_ident(raw: &str) -> String {
    if raw.len() <= 64 {
        raw.to_string()
    } else {
        raw.chars().take(64).collect()
    }
}

fn dump_single_database(db_name: &str, dest: &Path) -> Result<(), String> {
    let dump_bin = mariadb_dump_bin().ok_or_else(|| {
        "mariadb-dump/mysqldump not found. Install MariaDB client tools.".to_string()
    })?;
    let file = fs::File::create(dest).map_err(|e| format!("dump file: {e}"))?;
    let status = Command::new(dump_bin)
        .args(["--single-transaction", "--routines", "--events", db_name])
        .stdout(file)
        .status()
        .map_err(|e| format!("run {dump_bin}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("Dump of `{db_name}` failed (check MariaDB auth)."))
    }
}

fn import_single_database(db_name: &str, sql_path: &Path) -> Result<(), String> {
    let bin = mariadb_client_bin()
        .ok_or_else(|| "MariaDB client not found (`mariadb` or `mysql`).".to_string())?;
    let file = fs::File::open(sql_path).map_err(|e| format!("open dump: {e}"))?;
    let mut child = Command::new(bin)
        .arg(db_name)
        .stdin(Stdio::from(file))
        .spawn()
        .map_err(|e| format!("import spawn: {e}"))?;
    let status = child.wait().map_err(|e| format!("import wait: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("Import into `{db_name}` failed."))
    }
}

fn copy_tree_best_effort(src: &Path, dst: &Path) -> Result<(), String> {
    if !src.is_dir() {
        return Ok(());
    }
    fs::create_dir_all(dst).map_err(|e| format!("mkdir: {e}"))?;
    #[cfg(unix)]
    {
        let status = Command::new("cp")
            .arg("-a")
            .arg(format!("{}/.", src.display()))
            .arg(dst)
            .status()
            .map_err(|e| format!("cp: {e}"))?;
        if status.success() {
            return Ok(());
        }
        return Err("cp -a failed".into());
    }
    #[cfg(not(unix))]
    {
        copy_recursive_win(src, dst)
    }
}

#[cfg(not(unix))]
fn copy_recursive_win(src: &Path, dst: &Path) -> Result<(), String> {
    for entry in fs::read_dir(src).map_err(|e| format!("read_dir: {e}"))? {
        let entry = entry.map_err(|e| format!("entry: {e}"))?;
        let ty = entry.file_type().map_err(|e| format!("type: {e}"))?;
        let to = dst.join(entry.file_name());
        if ty.is_dir() {
            fs::create_dir_all(&to).map_err(|e| format!("mkdir: {e}"))?;
            copy_recursive_win(&entry.path(), &to)?;
        } else if ty.is_file() {
            fs::copy(entry.path(), &to).map_err(|e| format!("copy: {e}"))?;
        }
    }
    Ok(())
}

pub fn guess_db_name_from_wp_config(path: &Path) -> Option<String> {
    let raw = fs::read_to_string(path).ok()?;
    for line in raw.lines() {
        let t = line.trim();
        if t.starts_with("define")
            && t.contains("DB_NAME")
            && let Some(start) = t.find(',')
        {
            let rest = &t[start + 1..];
            let rest = rest.trim().trim_start_matches(['\'', '"']);
            let end = rest.find(['\'', '"', ')']).unwrap_or(rest.len());
            let name = rest[..end].trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

pub fn remap_wp_config_credentials(
    path: &Path,
    db_name: &str,
    db_user: &str,
    db_pass: &str,
) -> Result<(), String> {
    if !path.is_file() {
        return Ok(());
    }
    let raw = fs::read_to_string(path).map_err(|e| format!("read wp-config: {e}"))?;
    let mut out = String::with_capacity(raw.len() + 128);
    for line in raw.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("define") && trimmed.contains("DB_NAME") {
            out.push_str(&format!(
                "define( 'DB_NAME', '{}' );\n",
                db_name.replace('\'', "")
            ));
        } else if trimmed.starts_with("define") && trimmed.contains("DB_USER") {
            out.push_str(&format!(
                "define( 'DB_USER', '{}' );\n",
                db_user.replace('\'', "")
            ));
        } else if trimmed.starts_with("define") && trimmed.contains("DB_PASSWORD") {
            out.push_str(&format!(
                "define( 'DB_PASSWORD', '{}' );\n",
                db_pass.replace('\'', "")
            ));
        } else if trimmed.starts_with("define") && trimmed.contains("DB_HOST") {
            out.push_str("define( 'DB_HOST', 'localhost' );\n");
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    fs::write(path, out).map_err(|e| format!("write wp-config: {e}"))?;
    Ok(())
}

fn random_db_password() -> String {
    rand::rng()
        .sample_iter(&Alphanumeric)
        .take(24)
        .map(char::from)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn staging_db_ident_respects_length() {
        let long = "a".repeat(80);
        let id = staging_db_ident(&long, "staging.example.com");
        assert!(id.len() <= 64);
        assert!(id.starts_with("stg_"));
    }
}
