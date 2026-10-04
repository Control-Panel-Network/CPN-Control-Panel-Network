//! Hosting packages and per-account quota ACL.
//!
//! Registry: `$CPN_DATA_DIR/packages.json` and `$CPN_DATA_DIR/package-assignments.json`.
//! Unlimited limits use the sentinel `-1`. `0` means none allowed.
//! Schema upgrades from v1 migrate historical `0` (old unlimited) to `-1`.

use crate::account::{data_dir, load_bootstrap, now_unix};
use crate::account_mgmt::list_accounts;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub use crate::package_limits::{
    UNLIMITED, format_limit_display, is_unlimited, migrate_legacy_zero_unlimited, normalize_limit,
};

/// Schema 2: `0` means hard zero (none allowed); only `-1` is unlimited.
/// On upgrade from schema 1, stored `0` limits are migrated to `-1`.
const SCHEMA_VERSION: u32 = 2;
pub const DEFAULT_PACKAGE_ID: &str = "pkg-default";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuotaResource {
    Domains,
    Emails,
    Databases,
    FtpAccounts,
    DiskMb,
    BandwidthMb,
    MailingLists,
    Autoresponders,
    Forwarders,
    EmailFilters,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Package {
    pub id: String,
    pub name: String,
    /// Disk quota in MB (`-1` = unlimited; `0` is accepted and stored as `-1`).
    pub disk_mb: i64,
    /// Monthly bandwidth quota in MB (`-1` = unlimited), metered from site access logs.
    pub bandwidth_mb: i64,
    pub domains: i64,
    pub emails: i64,
    pub databases: i64,
    pub ftp_accounts: i64,
    /// CPN distribution lists (virtual alias expansion).
    #[serde(default = "crate::package_limits::default_unlimited")]
    pub mailing_lists: i64,
    /// Vacation / auto-reply scripts per mailbox.
    #[serde(default = "crate::package_limits::default_unlimited")]
    pub autoresponders: i64,
    /// Address forwarders (aliases).
    #[serde(default = "crate::package_limits::default_unlimited")]
    pub forwarders: i64,
    /// Sieve email filter rules.
    #[serde(default = "crate::package_limits::default_unlimited")]
    pub email_filters: i64,
    pub fqdn_enabled: bool,
    #[serde(default)]
    pub notes: String,
    /// Top-level sidebar nav ids hidden for accounts on this package.
    #[serde(default)]
    pub sidebar_hidden_nav_ids: Vec<String>,
    pub created_at_unix: u64,
    pub updated_at_unix: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct PackagesFile {
    #[serde(default)]
    schema_version: u32,
    #[serde(default)]
    packages: Vec<Package>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PackageAssignment {
    pub username: String,
    pub package_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct AssignmentsFile {
    #[serde(default)]
    schema_version: u32,
    #[serde(default)]
    assignments: Vec<PackageAssignment>,
}

#[derive(Debug, Clone)]
pub struct PackageInput {
    pub name: String,
    pub disk_mb: i64,
    pub bandwidth_mb: i64,
    pub domains: i64,
    pub emails: i64,
    pub databases: i64,
    pub ftp_accounts: i64,
    pub mailing_lists: i64,
    pub autoresponders: i64,
    pub forwarders: i64,
    pub email_filters: i64,
    pub fqdn_enabled: bool,
    pub notes: String,
    pub sidebar_hidden_nav_ids: Vec<String>,
}

impl Default for PackageInput {
    fn default() -> Self {
        Self {
            name: String::new(),
            disk_mb: UNLIMITED,
            bandwidth_mb: UNLIMITED,
            domains: UNLIMITED,
            emails: UNLIMITED,
            databases: UNLIMITED,
            ftp_accounts: UNLIMITED,
            mailing_lists: UNLIMITED,
            autoresponders: UNLIMITED,
            forwarders: UNLIMITED,
            email_filters: UNLIMITED,
            fqdn_enabled: true,
            notes: String::new(),
            sidebar_hidden_nav_ids: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PackageUsage {
    pub package_id: String,
    pub package_name: String,
    pub domains_used: u64,
    pub domains_limit: i64,
    pub emails_used: u64,
    pub emails_limit: i64,
    pub databases_used: u64,
    pub databases_limit: i64,
    pub ftp_used: u64,
    pub ftp_limit: i64,
    pub disk_mb_used: u64,
    pub disk_mb_limit: i64,
    pub bandwidth_mb_used: u64,
    pub bandwidth_mb_limit: i64,
    pub mailing_lists_used: u64,
    pub mailing_lists_limit: i64,
    pub autoresponders_used: u64,
    pub autoresponders_limit: i64,
    pub forwarders_used: u64,
    pub forwarders_limit: i64,
    pub email_filters_used: u64,
    pub email_filters_limit: i64,
    /// Sum of owned MariaDB schema sizes in bytes (informational; not a package count limit).
    pub database_disk_bytes: u64,
    pub fqdn_enabled: bool,
}

fn packages_path() -> PathBuf {
    data_dir().join("packages.json")
}

fn assignments_path() -> PathBuf {
    data_dir().join("package-assignments.json")
}

fn names_equal(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

fn has_control_chars(value: &str) -> bool {
    value.chars().any(|ch| ch.is_control())
}

pub use crate::package_naming::{
    normalize_owned_package_name, package_custom_name_for_edit, package_owner_from_name,
    sanitize_package_custom_name,
};
pub use crate::package_owned::{create_package_for, update_package_for};

fn write_json(path: &Path, raw: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Could not create data dir: {e}"))?;
    }
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    use std::io::Write;
    let mut file = options
        .open(path)
        .map_err(|e| format!("Could not write {}: {e}", path.display()))?;
    file.write_all(raw.as_bytes())
        .map_err(|e| format!("Could not save {}: {e}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn load_packages_file() -> PackagesFile {
    let Ok(raw) = fs::read_to_string(packages_path()) else {
        return PackagesFile {
            schema_version: SCHEMA_VERSION,
            packages: Vec::new(),
        };
    };
    let mut file: PackagesFile = serde_json::from_str(&raw).unwrap_or(PackagesFile {
        schema_version: SCHEMA_VERSION,
        packages: Vec::new(),
    });
    // Schema 1 treated `0` as unlimited. Migrate those zeros to `-1` once.
    if file.schema_version < 2 {
        for pkg in &mut file.packages {
            migrate_package_zero_unlimited(pkg);
        }
        file.schema_version = SCHEMA_VERSION;
        let _ = save_packages_file(&file);
    }
    file
}

fn save_packages_file(file: &PackagesFile) -> Result<(), String> {
    let mut out = file.clone();
    out.schema_version = SCHEMA_VERSION;
    let raw = serde_json::to_string_pretty(&out)
        .map_err(|e| format!("Could not serialize packages: {e}"))?;
    write_json(&packages_path(), &raw)
}

fn load_assignments_file() -> AssignmentsFile {
    let Ok(raw) = fs::read_to_string(assignments_path()) else {
        return AssignmentsFile {
            schema_version: SCHEMA_VERSION,
            assignments: Vec::new(),
        };
    };
    serde_json::from_str(&raw).unwrap_or(AssignmentsFile {
        schema_version: SCHEMA_VERSION,
        assignments: Vec::new(),
    })
}

fn save_assignments_file(file: &AssignmentsFile) -> Result<(), String> {
    let mut out = file.clone();
    out.schema_version = SCHEMA_VERSION;
    let raw = serde_json::to_string_pretty(&out)
        .map_err(|e| format!("Could not serialize package assignments: {e}"))?;
    write_json(&assignments_path(), &raw)
}

fn validate_limit(name: &str, value: i64) -> Result<(), String> {
    if value < UNLIMITED {
        return Err(format!(
            "{name} must be -1 (unlimited), 0 (none), or a positive number"
        ));
    }
    Ok(())
}

fn migrate_package_zero_unlimited(pkg: &mut Package) {
    pkg.disk_mb = migrate_legacy_zero_unlimited(pkg.disk_mb);
    pkg.bandwidth_mb = migrate_legacy_zero_unlimited(pkg.bandwidth_mb);
    pkg.domains = migrate_legacy_zero_unlimited(pkg.domains);
    pkg.emails = migrate_legacy_zero_unlimited(pkg.emails);
    pkg.databases = migrate_legacy_zero_unlimited(pkg.databases);
    pkg.ftp_accounts = migrate_legacy_zero_unlimited(pkg.ftp_accounts);
    pkg.mailing_lists = migrate_legacy_zero_unlimited(pkg.mailing_lists);
    pkg.autoresponders = migrate_legacy_zero_unlimited(pkg.autoresponders);
    pkg.forwarders = migrate_legacy_zero_unlimited(pkg.forwarders);
    pkg.email_filters = migrate_legacy_zero_unlimited(pkg.email_filters);
}

fn validate_input(input: &PackageInput) -> Result<String, String> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err("Package name is required".into());
    }
    if name.chars().count() > 128 {
        return Err("Package name is too long (max 128 characters)".into());
    }
    if has_control_chars(name) {
        return Err("Package name cannot include control characters".into());
    }
    validate_limit("disk_mb", input.disk_mb)?;
    validate_limit("bandwidth_mb", input.bandwidth_mb)?;
    validate_limit("domains", input.domains)?;
    validate_limit("emails", input.emails)?;
    validate_limit("databases", input.databases)?;
    validate_limit("ftp_accounts", input.ftp_accounts)?;
    validate_limit("mailing_lists", input.mailing_lists)?;
    validate_limit("autoresponders", input.autoresponders)?;
    validate_limit("forwarders", input.forwarders)?;
    validate_limit("email_filters", input.email_filters)?;
    if has_control_chars(&input.notes) {
        return Err("Notes cannot include control characters".into());
    }
    let _ = sanitize_sidebar_hidden(&input.sidebar_hidden_nav_ids)?;
    Ok(name.to_string())
}

fn sanitize_sidebar_hidden(raw: &[String]) -> Result<Vec<String>, String> {
    // Keep validation local to avoid a packages <-> sidebar_visibility cycle.
    let known: std::collections::HashSet<&str> = [
        "websites",
        "wordpress",
        "email",
        "databases",
        "ftp",
        "dns",
        "backups",
        "ssl",
        "plugins",
        "docker",
        "users",
        "packages",
        "root-files",
        "server",
        "mariadb",
        "php",
        "services",
        "processes",
        "logs",
        "package-manager",
        "litespeed",
        "security",
        "settings",
    ]
    .into_iter()
    .collect();
    let mut out = Vec::new();
    for id in raw {
        let trimmed = id.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed == "dashboard" {
            return Err("Dashboard cannot be hidden".into());
        }
        if !known.contains(trimmed) {
            return Err(format!("Unknown sidebar section `{trimmed}`"));
        }
        if !out.iter().any(|x: &String| x == trimmed) {
            out.push(trimmed.to_string());
        }
    }
    Ok(out)
}

fn default_package() -> Package {
    let now = now_unix();
    Package {
        id: DEFAULT_PACKAGE_ID.into(),
        name: "Default".into(),
        disk_mb: 1000,
        bandwidth_mb: 1000,
        domains: 20,
        emails: 1000,
        databases: 1000,
        ftp_accounts: 1000,
        mailing_lists: 1000,
        autoresponders: 1000,
        forwarders: 1000,
        email_filters: 1000,
        fqdn_enabled: true,
        notes: "Created automatically on first boot".into(),
        sidebar_hidden_nav_ids: Vec::new(),
        created_at_unix: now,
        updated_at_unix: now,
    }
}

/// Ensure the Default package exists (idempotent).
pub fn ensure_default_package() -> Result<Package, String> {
    let mut file = load_packages_file();
    if let Some(existing) = file
        .packages
        .iter()
        .find(|p| p.id == DEFAULT_PACKAGE_ID || names_equal(&p.name, "Default"))
    {
        return Ok(existing.clone());
    }
    let pkg = default_package();
    file.packages.push(pkg.clone());
    save_packages_file(&file)?;
    Ok(pkg)
}

/// Bootstrap account username is the panel admin.
pub fn is_panel_admin(username: &str) -> bool {
    match load_bootstrap() {
        Some(boot) => names_equal(&boot.username, username),
        None => false,
    }
}

pub fn list_packages() -> Result<Vec<Package>, String> {
    ensure_default_package()?;
    let mut packages = load_packages_file().packages;
    packages.sort_by_key(|a| a.name.to_lowercase());
    Ok(packages)
}

pub fn get_package(id_or_name: &str) -> Result<Package, String> {
    ensure_default_package()?;
    let key = id_or_name.trim();
    load_packages_file()
        .packages
        .into_iter()
        .find(|p| p.id == key || names_equal(&p.name, key))
        .ok_or_else(|| format!("Package `{key}` not found"))
}

fn allocate_package_id(file: &PackagesFile) -> String {
    let now = now_unix();
    for n in 0u32..10_000 {
        let id = if n == 0 {
            format!("pkg-{now}")
        } else {
            format!("pkg-{now}-{n}")
        };
        if !file.packages.iter().any(|p| p.id == id) {
            return id;
        }
    }
    format!("pkg-{now}-{}", file.packages.len())
}

pub fn create_package(input: PackageInput) -> Result<Package, String> {
    let name = validate_input(&input)?;
    if names_equal(&name, "Default") {
        return Err("Package name `Default` is reserved".into());
    }
    let mut file = load_packages_file();
    if file.packages.iter().any(|p| names_equal(&p.name, &name)) {
        return Err(format!("Package `{name}` already exists"));
    }
    let now = now_unix();
    let pkg = Package {
        id: allocate_package_id(&file),
        name,
        disk_mb: normalize_limit(input.disk_mb),
        bandwidth_mb: normalize_limit(input.bandwidth_mb),
        domains: normalize_limit(input.domains),
        emails: normalize_limit(input.emails),
        databases: normalize_limit(input.databases),
        ftp_accounts: normalize_limit(input.ftp_accounts),
        mailing_lists: normalize_limit(input.mailing_lists),
        autoresponders: normalize_limit(input.autoresponders),
        forwarders: normalize_limit(input.forwarders),
        email_filters: normalize_limit(input.email_filters),
        fqdn_enabled: input.fqdn_enabled,
        notes: input.notes.trim().to_string(),
        sidebar_hidden_nav_ids: sanitize_sidebar_hidden(&input.sidebar_hidden_nav_ids)?,
        created_at_unix: now,
        updated_at_unix: now,
    };
    file.packages.push(pkg.clone());
    save_packages_file(&file)?;
    Ok(pkg)
}

pub fn update_package(id: &str, input: PackageInput) -> Result<Package, String> {
    let name = validate_input(&input)?;
    let id = id.trim();
    let mut file = load_packages_file();
    if file
        .packages
        .iter()
        .any(|p| p.id != id && names_equal(&p.name, &name))
    {
        return Err(format!("Package `{name}` already exists"));
    }
    let Some(pkg) = file.packages.iter_mut().find(|p| p.id == id) else {
        return Err(format!("Package `{id}` not found"));
    };
    // Default package identity is fixed; never allow renaming away from Default.
    if pkg.id == DEFAULT_PACKAGE_ID {
        if !names_equal(&name, "Default") {
            return Err("The Default package name cannot be changed".into());
        }
        pkg.name = "Default".into();
    } else if names_equal(&name, "Default") {
        return Err("Package name `Default` is reserved".into());
    } else {
        pkg.name = name;
    }
    pkg.disk_mb = normalize_limit(input.disk_mb);
    pkg.bandwidth_mb = normalize_limit(input.bandwidth_mb);
    pkg.domains = normalize_limit(input.domains);
    pkg.emails = normalize_limit(input.emails);
    pkg.databases = normalize_limit(input.databases);
    pkg.ftp_accounts = normalize_limit(input.ftp_accounts);
    pkg.mailing_lists = normalize_limit(input.mailing_lists);
    pkg.autoresponders = normalize_limit(input.autoresponders);
    pkg.forwarders = normalize_limit(input.forwarders);
    pkg.email_filters = normalize_limit(input.email_filters);
    pkg.fqdn_enabled = input.fqdn_enabled;
    pkg.notes = input.notes.trim().to_string();
    pkg.sidebar_hidden_nav_ids = sanitize_sidebar_hidden(&input.sidebar_hidden_nav_ids)?;
    pkg.updated_at_unix = now_unix();
    let out = pkg.clone();
    save_packages_file(&file)?;
    Ok(out)
}

pub fn delete_package(id: &str) -> Result<(), String> {
    let id = id.trim();
    if id == DEFAULT_PACKAGE_ID {
        return Err("The Default package cannot be deleted".into());
    }
    let assigned = accounts_assigned_to(id)?;
    if !assigned.is_empty() {
        return Err(format!(
            "Cannot delete package `{id}` while assigned to: {}",
            assigned.join(", ")
        ));
    }
    let mut file = load_packages_file();
    let before = file.packages.len();
    file.packages.retain(|p| p.id != id);
    if file.packages.len() == before {
        return Err(format!("Package `{id}` not found"));
    }
    save_packages_file(&file)
}

pub fn accounts_assigned_to(package_id: &str) -> Result<Vec<String>, String> {
    let mut names: Vec<String> = load_assignments_file()
        .assignments
        .into_iter()
        .filter(|a| a.package_id == package_id)
        .map(|a| a.username)
        .collect();
    names.sort_by_key(|n| n.to_lowercase());
    Ok(names)
}

pub fn assign_package(username_raw: &str, package_id_raw: &str) -> Result<(), String> {
    let username = username_raw.trim();
    if username.is_empty() {
        return Err("Username is required".into());
    }
    let package = get_package(package_id_raw)?;
    let accounts = list_accounts()?;
    if !accounts.iter().any(|a| names_equal(&a.username, username)) {
        return Err(format!("Account `{username}` not found"));
    }
    let mut file = load_assignments_file();
    if let Some(existing) = file
        .assignments
        .iter_mut()
        .find(|a| names_equal(&a.username, username))
    {
        existing.package_id = package.id;
    } else {
        file.assignments.push(PackageAssignment {
            username: username.to_string(),
            package_id: package.id,
        });
    }
    save_assignments_file(&file)
}

pub fn package_for_account(username: &str) -> Result<Package, String> {
    ensure_default_package()?;
    let file = load_assignments_file();
    if let Some(assignment) = file
        .assignments
        .iter()
        .find(|a| names_equal(&a.username, username))
    {
        return get_package(&assignment.package_id);
    }
    get_package(DEFAULT_PACKAGE_ID)
}

pub use crate::package_quota::{require_quota, require_site_create_allowed, usage_for_account};

#[cfg(test)]
#[path = "packages_tests.rs"]
mod tests;
