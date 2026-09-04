//! Hosting packages and per-account quota ACL.
//!
//! Registry: `$CPN_DATA_DIR/packages.json` and `$CPN_DATA_DIR/package-assignments.json`.
//! Unlimited limits use the sentinel `-1`.

use crate::account::{data_dir, load_bootstrap, now_unix};
use crate::account_mgmt::list_accounts;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const SCHEMA_VERSION: u32 = 1;
/// Sentinel for unlimited resource limits.
pub const UNLIMITED: i64 = -1;
pub const DEFAULT_PACKAGE_ID: &str = "pkg-default";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuotaResource {
    Domains,
    Emails,
    Databases,
    FtpAccounts,
    DiskMb,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Package {
    pub id: String,
    pub name: String,
    /// Disk quota in MB (`-1` = unlimited).
    pub disk_mb: i64,
    /// Bandwidth quota in MB (`-1` = unlimited). Metering is not enforced yet.
    pub bandwidth_mb: i64,
    pub domains: i64,
    pub emails: i64,
    pub databases: i64,
    pub ftp_accounts: i64,
    pub fqdn_enabled: bool,
    #[serde(default)]
    pub notes: String,
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

#[derive(Debug, Clone, Default)]
pub struct PackageInput {
    pub name: String,
    pub disk_mb: i64,
    pub bandwidth_mb: i64,
    pub domains: i64,
    pub emails: i64,
    pub databases: i64,
    pub ftp_accounts: i64,
    pub fqdn_enabled: bool,
    pub notes: String,
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
    pub bandwidth_mb_limit: i64,
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
    serde_json::from_str(&raw).unwrap_or(PackagesFile {
        schema_version: SCHEMA_VERSION,
        packages: Vec::new(),
    })
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
            "{name} must be -1 (unlimited) or a non-negative number"
        ));
    }
    Ok(())
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
    if has_control_chars(&input.notes) {
        return Err("Notes cannot include control characters".into());
    }
    Ok(name.to_string())
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
        fqdn_enabled: true,
        notes: "Created automatically on first boot".into(),
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

pub fn create_package(input: PackageInput) -> Result<Package, String> {
    let name = validate_input(&input)?;
    let mut file = load_packages_file();
    if file.packages.iter().any(|p| names_equal(&p.name, &name)) {
        return Err(format!("Package `{name}` already exists"));
    }
    let now = now_unix();
    let pkg = Package {
        id: format!("pkg-{now}"),
        name,
        disk_mb: input.disk_mb,
        bandwidth_mb: input.bandwidth_mb,
        domains: input.domains,
        emails: input.emails,
        databases: input.databases,
        ftp_accounts: input.ftp_accounts,
        fqdn_enabled: input.fqdn_enabled,
        notes: input.notes.trim().to_string(),
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
    pkg.name = name;
    pkg.disk_mb = input.disk_mb;
    pkg.bandwidth_mb = input.bandwidth_mb;
    pkg.domains = input.domains;
    pkg.emails = input.emails;
    pkg.databases = input.databases;
    pkg.ftp_accounts = input.ftp_accounts;
    pkg.fqdn_enabled = input.fqdn_enabled;
    pkg.notes = input.notes.trim().to_string();
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

pub fn format_limit_display(limit: i64, unit: &str) -> String {
    if limit == UNLIMITED {
        "Unlimited".into()
    } else if unit.is_empty() {
        limit.to_string()
    } else {
        format!("{limit} {unit}")
    }
}

pub use crate::package_quota::{require_quota, require_site_create_allowed, usage_for_account};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;
    use crate::account_mgmt::create_account;
    use crate::model::PasswordPolicy;

    fn policy() -> PasswordPolicy {
        PasswordPolicy {
            min_length: 8,
            require_special: true,
            require_uppercase: true,
            require_number: true,
        }
    }

    #[test]
    fn delete_blocked_when_assigned() {
        with_test_data_dir(|| {
            create_account(
                "ops",
                Some("OpsPass1!"),
                false,
                "ops@example.com",
                policy(),
                "en",
            )
            .unwrap();
            let pkg = create_package(PackageInput {
                name: "Reseller".into(),
                disk_mb: 500,
                bandwidth_mb: 500,
                domains: 2,
                emails: 10,
                databases: 2,
                ftp_accounts: 2,
                fqdn_enabled: true,
                notes: String::new(),
            })
            .unwrap();
            assign_package("ops", &pkg.id).unwrap();
            assert!(delete_package(&pkg.id).is_err());
            assign_package("ops", DEFAULT_PACKAGE_ID).unwrap();
            delete_package(&pkg.id).unwrap();
        });
    }
}
