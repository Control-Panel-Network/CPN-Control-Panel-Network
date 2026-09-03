//! Website records under `$CPN_DATA_DIR/sites/` (default `/var/lib/cpn/sites/`).
//!
//! These are structured JSON records for the operator CLI. Full Nginx / Caddy /
//! OpenLiteSpeed vhost wiring is applied later when panel recipes own that path.

use serde::{Deserialize, Serialize};
use std::{
    fs,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};

use crate::account::{data_dir, now_unix};

const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteRecord {
    pub schema_version: u32,
    pub domain: String,
    pub owner: String,
    pub docroot: String,
    pub enabled: bool,
    /// Optional engine hint (`openlitespeed`, `nginx`, `caddy`).
    pub engine: Option<String>,
    pub notes: String,
    pub created_at_unix: u64,
    pub updated_at_unix: u64,
    /// True until a future release writes real vhost files for this domain.
    pub vhost_wired: bool,
}

#[derive(Debug, Clone, Default)]
pub struct SiteModify {
    pub owner: Option<String>,
    pub docroot: Option<String>,
    pub enabled: Option<bool>,
    pub engine: Option<String>,
    pub notes: Option<String>,
}

fn sites_dir() -> PathBuf {
    data_dir().join("sites")
}

fn has_control_chars(value: &str) -> bool {
    value.chars().any(|ch| ch.is_control())
}

/// Normalize and validate a DNS-like domain label for site keys.
pub fn normalize_domain(raw: &str) -> Result<String, String> {
    let domain = raw.trim().to_lowercase();
    if domain.is_empty() {
        return Err("Domain is required".into());
    }
    if domain.chars().count() > 253 {
        return Err("Domain is too long (max 253 characters)".into());
    }
    if has_control_chars(&domain) {
        return Err("Domain cannot include control characters".into());
    }
    if domain.starts_with('.') || domain.ends_with('.') || domain.contains("..") {
        return Err("Domain format is invalid".into());
    }
    if !domain
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '.')
    {
        return Err("Domain may only contain letters, digits, hyphen, and dot".into());
    }
    if !domain.contains('.') {
        return Err("Domain must include a dot (example.com)".into());
    }
    Ok(domain)
}

fn site_path(domain: &str) -> PathBuf {
    sites_dir().join(format!("{domain}.json"))
}

fn default_docroot(domain: &str) -> String {
    format!("/var/www/{domain}/public_html")
}

fn persist_site(path: &Path, site: &SiteRecord) -> Result<(), String> {
    fs::create_dir_all(sites_dir())
        .map_err(|error| format!("Could not create {}: {error}", sites_dir().display()))?;
    let json = serde_json::to_string_pretty(site)
        .map_err(|error| format!("Could not serialize site record: {error}"))?;
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true).mode(0o600);
    use std::io::Write;
    let mut file = options
        .open(path)
        .map_err(|error| format!("Could not write {}: {error}", path.display()))?;
    file.write_all(json.as_bytes())
        .map_err(|error| format!("Could not save site record: {error}"))?;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    Ok(())
}

fn load_site_at(path: &Path) -> Result<SiteRecord, String> {
    let raw = fs::read_to_string(path)
        .map_err(|error| format!("Could not read {}: {error}", path.display()))?;
    serde_json::from_str(&raw)
        .map_err(|error| format!("Invalid site JSON in {}: {error}", path.display()))
}

pub fn load_site(domain_raw: &str) -> Result<SiteRecord, String> {
    let domain = normalize_domain(domain_raw)?;
    let path = site_path(&domain);
    if !path.is_file() {
        return Err(format!("Site `{domain}` not found"));
    }
    load_site_at(&path)
}

pub fn list_sites() -> Result<Vec<SiteRecord>, String> {
    let dir = sites_dir();
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut sites = Vec::new();
    let entries =
        fs::read_dir(&dir).map_err(|error| format!("Could not read {}: {error}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("Could not read site entry: {error}"))?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        sites.push(load_site_at(&path)?);
    }
    sites.sort_by_key(|a| a.domain.clone());
    Ok(sites)
}

pub fn create_site(
    domain_raw: &str,
    owner: &str,
    docroot: Option<&str>,
    engine: Option<&str>,
    notes: Option<&str>,
) -> Result<SiteRecord, String> {
    let domain = normalize_domain(domain_raw)?;
    let path = site_path(&domain);
    if path.is_file() {
        return Err(format!("Site `{domain}` already exists"));
    }
    let owner = owner.trim();
    if owner.is_empty() {
        return Err("Owner is required".into());
    }
    if has_control_chars(owner) {
        return Err("Owner cannot include control characters".into());
    }
    let now = now_unix();
    let site = SiteRecord {
        schema_version: SCHEMA_VERSION,
        domain: domain.clone(),
        owner: owner.to_string(),
        docroot: docroot
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| default_docroot(&domain)),
        enabled: true,
        engine: engine
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_lowercase()),
        notes: notes.unwrap_or("").trim().to_string(),
        created_at_unix: now,
        updated_at_unix: now,
        vhost_wired: false,
    };
    persist_site(&path, &site)?;
    Ok(site)
}

pub fn modify_site(domain_raw: &str, patch: SiteModify) -> Result<SiteRecord, String> {
    let domain = normalize_domain(domain_raw)?;
    let path = site_path(&domain);
    if !path.is_file() {
        return Err(format!("Site `{domain}` not found"));
    }
    let mut site = load_site_at(&path)?;
    if let Some(owner) = patch.owner {
        let owner = owner.trim();
        if owner.is_empty() {
            return Err("Owner cannot be empty".into());
        }
        if has_control_chars(owner) {
            return Err("Owner cannot include control characters".into());
        }
        site.owner = owner.to_string();
    }
    if let Some(docroot) = patch.docroot {
        let docroot = docroot.trim();
        if docroot.is_empty() {
            return Err("Docroot cannot be empty".into());
        }
        if has_control_chars(docroot) {
            return Err("Docroot cannot include control characters".into());
        }
        site.docroot = docroot.to_string();
    }
    if let Some(enabled) = patch.enabled {
        site.enabled = enabled;
    }
    if let Some(engine) = patch.engine {
        let engine = engine.trim();
        site.engine = if engine.is_empty() {
            None
        } else {
            Some(engine.to_lowercase())
        };
    }
    if let Some(notes) = patch.notes {
        site.notes = notes;
    }
    site.updated_at_unix = now_unix();
    persist_site(&path, &site)?;
    Ok(site)
}

pub fn delete_site(domain_raw: &str) -> Result<(), String> {
    let domain = normalize_domain(domain_raw)?;
    let path = site_path(&domain);
    if !path.is_file() {
        return Err(format!("Site `{domain}` not found"));
    }
    fs::remove_file(&path)
        .map_err(|error| format!("Could not delete {}: {error}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::DATA_DIR_TEST_LOCK;

    fn with_temp_data<T>(f: impl FnOnce() -> T) -> T {
        let _guard = DATA_DIR_TEST_LOCK.lock().unwrap();
        let dir = std::env::temp_dir().join(format!("cpn-sites-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        // SAFETY: tests hold LOCK; only this thread sets CPN_DATA_DIR.
        unsafe {
            std::env::set_var("CPN_DATA_DIR", &dir);
        }
        let result = f();
        unsafe {
            std::env::remove_var("CPN_DATA_DIR");
        }
        let _ = fs::remove_dir_all(&dir);
        result
    }

    #[test]
    fn domain_normalization() {
        assert_eq!(normalize_domain(" Example.COM ").unwrap(), "example.com");
        assert!(normalize_domain("nodot").is_err());
        assert!(normalize_domain("bad_domain.com").is_err());
    }

    #[test]
    fn site_crud_roundtrip() {
        with_temp_data(|| {
            let created =
                create_site("app.example.com", "admin", None, Some("nginx"), None).expect("create");
            assert_eq!(created.domain, "app.example.com");
            assert!(!created.vhost_wired);
            assert_eq!(created.docroot, "/var/www/app.example.com/public_html");

            let listed = list_sites().unwrap();
            assert_eq!(listed.len(), 1);

            let updated = modify_site(
                "app.example.com",
                SiteModify {
                    owner: Some("ops".into()),
                    enabled: Some(false),
                    ..SiteModify::default()
                },
            )
            .unwrap();
            assert_eq!(updated.owner, "ops");
            assert!(!updated.enabled);

            delete_site("app.example.com").unwrap();
            assert!(list_sites().unwrap().is_empty());
        });
    }
}
