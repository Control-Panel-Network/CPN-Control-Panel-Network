//! Copy sites, databases, mail, and DNS into CPN, then write a verify report.

use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::listen_port::{DEFAULT_PORT, load_preferred_listen_port};
use crate::sites::{create_site, default_docroot, normalize_domain, resolve_parent_domain};

use super::catalog::{PanelSpec, spec_by_id};
use super::scan::{Inventory, SiteHit, inventory_for, rel};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SiteVerify {
    pub domain: String,
    pub source: String,
    pub dest: String,
    pub files: u64,
    pub adopt_in_place: bool,
    pub registered: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VerifyReport {
    pub panel_id: String,
    pub panel_name: String,
    pub verified: bool,
    pub sites: Vec<SiteVerify>,
    pub databases: Vec<String>,
    pub mail_copied: Vec<String>,
    pub zones_copied: Vec<String>,
    pub users: Vec<String>,
    pub delete_paths: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ApplyOptions {
    pub port_ok: bool,
    pub register_sites: bool,
}

pub fn report_path(panel_id: &str) -> PathBuf {
    crate::account::data_dir()
        .join("panel-migrate")
        .join(format!("{panel_id}.json"))
}

pub fn load_report(panel_id: &str) -> Result<VerifyReport, String> {
    let path = report_path(panel_id);
    let raw = fs::read_to_string(&path)
        .map_err(|_| format!("No verify report for `{panel_id}`. Run apply first."))?;
    serde_json::from_str(&raw).map_err(|error| format!("Verify report is unreadable: {error}"))
}

pub fn save_report(report: &VerifyReport) -> Result<(), String> {
    let path = report_path(&report.panel_id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Could not create {}: {error}", parent.display()))?;
    }
    let body = serde_json::to_string_pretty(report)
        .map_err(|error| format!("Could not encode verify report: {error}"))?;
    fs::write(&path, body).map_err(|error| format!("Could not write {}: {error}", path.display()))
}

pub fn apply_panel(
    root: &Path,
    panel_id: &str,
    options: &ApplyOptions,
) -> Result<VerifyReport, String> {
    let spec = spec_by_id(panel_id).ok_or_else(|| format!("Unknown panel `{panel_id}`"))?;
    if !super::detect::marker_present(root, spec) {
        return Err(format!(
            "{} is not installed under {}",
            spec.name,
            root.display()
        ));
    }
    let port = load_preferred_listen_port().unwrap_or(DEFAULT_PORT);
    if spec.listen_port != 0 && spec.listen_port == port && !options.port_ok {
        return Err(format!(
            "{} listens on {port}, the same port as CPN. Stop that panel or pass --port-ok after choosing a CPN port.",
            spec.name
        ));
    }
    let inventory = inventory_for(root, spec);
    if !inventory.mapped && inventory.sites.is_empty() {
        return Err(format!("{}: detected, inventory not mapped yet", spec.name));
    }
    if inventory.sites.is_empty() {
        return Err(format!("No websites found for {}", spec.name));
    }
    let mut report = VerifyReport {
        panel_id: spec.id.to_string(),
        panel_name: spec.name.to_string(),
        verified: true,
        sites: Vec::new(),
        databases: Vec::new(),
        mail_copied: Vec::new(),
        zones_copied: Vec::new(),
        users: Vec::new(),
        delete_paths: Vec::new(),
        warnings: Vec::new(),
    };
    if !inventory.note.is_empty() {
        report.warnings.push(inventory.note);
    }
    for site in &inventory.sites {
        match copy_one(root, spec, site, options, &mut report) {
            Ok(()) => {}
            Err(error) => {
                report.verified = false;
                report.warnings.push(error);
            }
        }
    }
    for rel_path in spec.wipe_rel {
        let path = rel(root, rel_path);
        if path.exists() {
            report
                .delete_paths
                .push(path.to_string_lossy().into_owned());
        }
    }
    save_report(&report)?;
    Ok(report)
}

fn copy_one(
    _root: &Path,
    spec: &PanelSpec,
    site: &SiteHit,
    options: &ApplyOptions,
    report: &mut VerifyReport,
) -> Result<(), String> {
    let domain = normalize_domain(&site.domain)?;
    let source = PathBuf::from(&site.docroot);
    if !source.is_dir() {
        return Err(format!(
            "{domain}: source docroot missing ({})",
            source.display()
        ));
    }
    let parent = resolve_parent_domain(&domain).ok().flatten();
    let dest_text = default_docroot(&domain, parent.as_deref());
    let dest = PathBuf::from(&dest_text);
    let adopt = same_path(&source, &dest);
    let files = if adopt {
        count_files(&source).unwrap_or(0)
    } else {
        copy_tree(&source, &dest)?
    };
    if !adopt {
        report
            .delete_paths
            .push(source.to_string_lossy().into_owned());
    }
    let mut registered = false;
    if options.register_sites {
        match create_site(
            &domain,
            if site.owner.trim().is_empty() {
                "cpnowner"
            } else {
                site.owner.trim()
            },
            Some(&dest_text),
            None,
            Some(&format!("Migrated from {}", spec.name)),
        ) {
            Ok(_) => registered = true,
            Err(error) if error.contains("already exists") => {
                report
                    .warnings
                    .push(format!("{domain}: already registered in CPN"));
                registered = true;
            }
            Err(error) => {
                report.verified = false;
                report.warnings.push(format!("{domain}: {error}"));
            }
        }
    }
    import_databases(site, report);
    copy_mail(&domain, site, report);
    copy_zones(&domain, site, report);
    if !site.owner.trim().is_empty() && !report.users.iter().any(|user| user == &site.owner) {
        report.users.push(site.owner.clone());
    }
    report.sites.push(SiteVerify {
        domain,
        source: source.to_string_lossy().into_owned(),
        dest: dest_text,
        files,
        adopt_in_place: adopt,
        registered,
    });
    let _ = spec;
    Ok(())
}

fn import_databases(site: &SiteHit, report: &mut VerifyReport) {
    for name in &site.databases {
        if report.databases.iter().any(|have| have == name) {
            continue;
        }
        let dump = crate::account::data_dir()
            .join("panel-migrate")
            .join("dumps")
            .join(format!("{name}.sql"));
        match dump_database(name, &dump) {
            Ok(()) => {
                if let Err(error) = import_sql(&dump) {
                    report.warnings.push(format!(
                        "Database `{name}` dumped but import failed: {error}"
                    ));
                }
                report.databases.push(name.clone());
            }
            Err(error) => report.warnings.push(format!("Database `{name}`: {error}")),
        }
    }
    for sql in &site.sql_files {
        let path = PathBuf::from(sql);
        match import_sql(&path) {
            Ok(()) => report.databases.push(path.to_string_lossy().into_owned()),
            Err(error) => report
                .warnings
                .push(format!("SQL {}: {error}", path.display())),
        }
    }
}

fn dump_database(name: &str, dest: &Path) -> Result<(), String> {
    if !valid_ident(name) {
        return Err("database name rejected".into());
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let bin = if command_exists("mariadb-dump") {
        "mariadb-dump"
    } else if command_exists("mysqldump") {
        "mysqldump"
    } else {
        return Err("mariadb-dump is not installed; database left on the source host".into());
    };
    let output = Command::new(bin)
        .args(["--single-transaction", "--skip-lock-tables", name])
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    fs::write(dest, &output.stdout).map_err(|error| error.to_string())
}

fn import_sql(path: &Path) -> Result<(), String> {
    let bin = if command_exists("mariadb") {
        "mariadb"
    } else if command_exists("mysql") {
        "mysql"
    } else {
        return Err("MariaDB client is not installed; SQL file was not imported".into());
    };
    let file = fs::File::open(path).map_err(|error| error.to_string())?;
    let mut child = Command::new(bin)
        .stdin(std::process::Stdio::from(file))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|error| error.to_string())?;
    let status = child.wait().map_err(|error| error.to_string())?;
    if status.success() {
        Ok(())
    } else {
        let mut err = String::new();
        if let Some(mut stderr) = child.stderr.take() {
            let _ = stderr.read_to_string(&mut err);
        }
        Err(if err.trim().is_empty() {
            format!("exit {}", status.code().unwrap_or(1))
        } else {
            err.trim().to_string()
        })
    }
}

fn copy_mail(domain: &str, site: &SiteHit, report: &mut VerifyReport) {
    for source in &site.mail_dirs {
        let source_path = PathBuf::from(source);
        let dest = crate::sites::hosting_home_root()
            .join(domain)
            .join("mail-import");
        if same_path(&source_path, &dest) {
            continue;
        }
        match copy_tree(&source_path, &dest) {
            Ok(files) if files > 0 => {
                report.mail_copied.push(dest.to_string_lossy().into_owned());
                report
                    .delete_paths
                    .push(source_path.to_string_lossy().into_owned());
            }
            Ok(_) => {}
            Err(error) => report.warnings.push(format!("{domain} mail: {error}")),
        }
    }
}

fn copy_zones(domain: &str, site: &SiteHit, report: &mut VerifyReport) {
    for source in &site.zone_files {
        let source_path = PathBuf::from(source);
        let dest = crate::account::data_dir()
            .join("panel-migrate")
            .join("zones")
            .join(format!("{domain}.zone"));
        if let Some(parent) = dest.parent()
            && fs::create_dir_all(parent).is_err()
        {
            report
                .warnings
                .push(format!("{domain} DNS: could not create zone directory"));
            continue;
        }
        match fs::copy(&source_path, &dest) {
            Ok(_) => {
                report
                    .zones_copied
                    .push(dest.to_string_lossy().into_owned());
                report
                    .delete_paths
                    .push(source_path.to_string_lossy().into_owned());
            }
            Err(error) => report.warnings.push(format!("{domain} DNS: {error}")),
        }
    }
}

pub fn copy_tree(source: &Path, dest: &Path) -> Result<u64, String> {
    fs::create_dir_all(dest)
        .map_err(|error| format!("Could not create {}: {error}", dest.display()))?;
    let mut count = 0u64;
    copy_tree_inner(source, dest, &mut count)?;
    Ok(count)
}

fn copy_tree_inner(source: &Path, dest: &Path, count: &mut u64) -> Result<(), String> {
    let entries = fs::read_dir(source)
        .map_err(|error| format!("Could not read {}: {error}", source.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let from = entry.path();
        let name = entry.file_name();
        let to = dest.join(&name);
        let meta = entry
            .metadata()
            .map_err(|error| format!("{}: {error}", from.display()))?;
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() {
            fs::create_dir_all(&to).map_err(|error| error.to_string())?;
            copy_tree_inner(&from, &to, count)?;
        } else if meta.is_file() {
            copy_file(&from, &to)?;
            *count += 1;
        }
    }
    Ok(())
}

fn copy_file(from: &Path, to: &Path) -> Result<(), String> {
    let mut input = fs::File::open(from)
        .map_err(|error| format!("Could not read {}: {error}", from.display()))?;
    let mut output = fs::File::create(to)
        .map_err(|error| format!("Could not write {}: {error}", to.display()))?;
    io::copy(&mut input, &mut output).map_err(|error| error.to_string())?;
    output.flush().map_err(|error| error.to_string())?;
    Ok(())
}

fn count_files(dir: &Path) -> Result<u64, String> {
    let mut count = 0u64;
    count_inner(dir, &mut count)?;
    Ok(count)
}

fn count_inner(dir: &Path, count: &mut u64) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|error| error.to_string())?;
    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let meta = entry.metadata().map_err(|error| error.to_string())?;
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() {
            count_inner(&entry.path(), count)?;
        } else if meta.is_file() {
            *count += 1;
        }
    }
    Ok(())
}

fn same_path(left: &Path, right: &Path) -> bool {
    if let (Ok(a), Ok(b)) = (left.canonicalize(), right.canonicalize()) {
        return a == b;
    }
    left == right
}

fn command_exists(name: &str) -> bool {
    Command::new(name)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn valid_ident(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

pub fn inventory_note(inventory: &Inventory) -> String {
    if !inventory.mapped {
        inventory.note.clone()
    } else {
        format!("{} website(s)", inventory.sites.len())
    }
}
