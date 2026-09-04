//! Website registry JSON under `$CPN_DATA_DIR/sites/` plus document roots on disk.
//!
//! # Path convention
//!
//! - Primary domain `example.com`:
//!   - Home: `/home/example.com/`
//!   - Docroot: `/home/example.com/public_html/`
//! - Subdomain `blog.example.com` (parent site `example.com` must already exist):
//!   - Home: `/home/example.com/blog.example.com/`
//!   - Docroot: `/home/example.com/blog.example.com/public_html/`
//!
//! Machine-readable records stay under `/var/lib/cpn/sites/<domain>.json` (or
//! `$CPN_DATA_DIR/sites/`) and point at `docroot`. Override the hosting home root
//! with `CPN_SITES_HOME` (labs and unit tests).
//!
//! Vhost wiring is applied later by panel recipes (`vhost_wired=false` until then).

use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::account::{data_dir, now_unix};

const SCHEMA_VERSION: u32 = 1;

const DEFAULT_INDEX_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Site ready</title>
</head>
<body>
  <h1>Site ready</h1>
  <p>This document root was created by CPN. Replace this file with your site.</p>
</body>
</html>
"#;

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

/// Root under which domain homes are created (`/home` on Unix by default).
pub fn hosting_home_root() -> PathBuf {
    if let Ok(value) = std::env::var("CPN_SITES_HOME") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    #[cfg(unix)]
    {
        PathBuf::from("/home")
    }
    #[cfg(not(unix))]
    {
        PathBuf::from(r"C:\CPN\SitesHome")
    }
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

/// Parent domain candidates for a FQDN, longest first (for example
/// `a.b.example.com` -> `b.example.com`, `example.com`).
pub fn parent_domain_candidates(domain: &str) -> Vec<String> {
    let parts: Vec<&str> = domain.split('.').collect();
    let mut out = Vec::new();
    if parts.len() <= 2 {
        return out;
    }
    for skip in 1..=(parts.len() - 2) {
        out.push(parts[skip..].join("."));
    }
    out
}

/// Resolve an existing parent site for a subdomain, or `None` for a primary domain.
pub fn resolve_parent_domain(domain: &str) -> Result<Option<String>, String> {
    let candidates = parent_domain_candidates(domain);
    if candidates.is_empty() {
        return Ok(None);
    }
    for candidate in &candidates {
        if site_path(candidate).is_file() {
            return Ok(Some(candidate.clone()));
        }
    }
    Err(format!(
        "Parent domain must exist before creating subdomain `{domain}`. Create one of: {}",
        candidates.join(", ")
    ))
}

/// Domain home directory (not the public_html docroot).
pub fn site_home_dir(domain: &str, parent: Option<&str>) -> PathBuf {
    let root = hosting_home_root();
    match parent {
        Some(parent_domain) => root.join(parent_domain).join(domain),
        None => root.join(domain),
    }
}

/// Default document root under the domain home.
pub fn default_docroot(domain: &str, parent: Option<&str>) -> String {
    site_home_dir(domain, parent)
        .join("public_html")
        .to_string_lossy()
        .into_owned()
}

/// Domain home derived from a site record (parent of `public_html` when standard).
pub fn site_home_from_record(site: &SiteRecord) -> PathBuf {
    let path = Path::new(&site.docroot);
    if path.file_name().and_then(|name| name.to_str()) == Some("public_html") {
        if let Some(parent) = path.parent() {
            return parent.to_path_buf();
        }
    }
    let parent = resolve_parent_domain(&site.domain).ok().flatten();
    site_home_dir(&site.domain, parent.as_deref())
}

/// Per-site backup directory: `/home/<domain>/backups` (or nested for subdomains).
pub fn site_backups_dir(site: &SiteRecord) -> PathBuf {
    site_home_from_record(site).join("backups")
}

/// Per-site plugins directory: `/home/<domain>/plugins`.
pub fn site_plugins_dir(site: &SiteRecord) -> PathBuf {
    site_home_from_record(site).join("plugins")
}

/// True when `docroot` does not follow the current `/home/.../public_html` layout
/// (legacy `/var/www/...` or custom paths still shown in list/UI).
pub fn is_legacy_docroot(docroot: &str) -> bool {
    let root = hosting_home_root();
    let path = Path::new(docroot);
    if !path.starts_with(&root) {
        return true;
    }
    path.file_name().and_then(|name| name.to_str()) != Some("public_html")
}

fn validate_docroot(path: &str) -> Result<(), String> {
    let path = path.trim();
    if path.is_empty() {
        return Err("Docroot cannot be empty".into());
    }
    if has_control_chars(path) {
        return Err("Docroot cannot include control characters".into());
    }
    if path.contains("..") {
        return Err("Docroot cannot contain '..'".into());
    }
    let p = Path::new(path);
    if !p.is_absolute() {
        return Err("Docroot must be an absolute path".into());
    }
    Ok(())
}

fn set_dir_mode(path: &Path, mode: u32) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(mode));
    }
    let _ = path;
    let _ = mode;
}

fn try_chown_root(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        if let Ok(c_path) = std::ffi::CString::new(path.as_os_str().as_bytes()) {
            // Best effort: root:root when running as root; ignore failures in labs.
            unsafe {
                let _ = libc::chown(c_path.as_ptr(), 0, 0);
            }
        }
    }
    let _ = path;
}

/// Create home + `public_html`, set safe modes, and write a placeholder index if missing.
pub fn ensure_site_directories(docroot: &str) -> Result<(), String> {
    validate_docroot(docroot)?;
    let docroot_path = Path::new(docroot);
    fs::create_dir_all(docroot_path)
        .map_err(|error| format!("Could not create {}: {error}", docroot_path.display()))?;
    set_dir_mode(docroot_path, 0o755);
    try_chown_root(docroot_path);
    if let Some(home) = docroot_path.parent() {
        set_dir_mode(home, 0o755);
        try_chown_root(home);
        if let Some(parent_home) = home.parent() {
            if parent_home != hosting_home_root() {
                set_dir_mode(parent_home, 0o755);
                try_chown_root(parent_home);
            }
        }
    }
    let index = docroot_path.join("index.html");
    if !index.is_file() {
        fs::write(&index, DEFAULT_INDEX_HTML)
            .map_err(|error| format!("Could not write {}: {error}", index.display()))?;
        set_dir_mode(&index, 0o644);
        try_chown_root(&index);
    }
    Ok(())
}

fn persist_site(path: &Path, site: &SiteRecord) -> Result<(), String> {
    fs::create_dir_all(sites_dir())
        .map_err(|error| format!("Could not create {}: {error}", sites_dir().display()))?;
    let json = serde_json::to_string_pretty(site)
        .map_err(|error| format!("Could not serialize site record: {error}"))?;
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
        .map_err(|error| format!("Could not write {}: {error}", path.display()))?;
    file.write_all(json.as_bytes())
        .map_err(|error| format!("Could not save site record: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
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
    sites.sort_by(|a, b| a.domain.cmp(&b.domain));
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
    let parent = resolve_parent_domain(&domain)?;
    let resolved_docroot = match docroot.map(str::trim).filter(|value| !value.is_empty()) {
        Some(custom) => {
            validate_docroot(custom)?;
            custom.to_string()
        }
        None => default_docroot(&domain, parent.as_deref()),
    };
    ensure_site_directories(&resolved_docroot)?;
    let now = now_unix();
    let site = SiteRecord {
        schema_version: SCHEMA_VERSION,
        domain: domain.clone(),
        owner: owner.to_string(),
        docroot: resolved_docroot,
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
        validate_docroot(&docroot)?;
        ensure_site_directories(docroot.trim())?;
        site.docroot = docroot.trim().to_string();
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

/// Remove the registry JSON only. Document root files under `/home/...` are kept.
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
        let home = dir.join("home");
        fs::create_dir_all(&home).unwrap();
        // SAFETY: tests hold LOCK; only this thread sets CPN_DATA_DIR / CPN_SITES_HOME.
        unsafe {
            std::env::set_var("CPN_DATA_DIR", &dir);
            std::env::set_var("CPN_SITES_HOME", &home);
        }
        let result = f();
        unsafe {
            std::env::remove_var("CPN_DATA_DIR");
            std::env::remove_var("CPN_SITES_HOME");
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
    fn parent_candidates_order() {
        assert_eq!(
            parent_domain_candidates("a.b.example.com"),
            vec!["b.example.com".to_string(), "example.com".to_string()]
        );
        assert!(parent_domain_candidates("example.com").is_empty());
    }

    #[test]
    fn site_crud_roundtrip_primary_and_subdomain() {
        with_temp_data(|| {
            let home = hosting_home_root();
            let created =
                create_site("example.com", "admin", None, Some("nginx"), None).expect("create");
            assert_eq!(created.domain, "example.com");
            assert!(!created.vhost_wired);
            let expected = home
                .join("example.com")
                .join("public_html")
                .to_string_lossy()
                .into_owned();
            assert_eq!(created.docroot, expected);
            assert!(Path::new(&created.docroot).join("index.html").is_file());

            let sub = create_site("blog.example.com", "admin", None, None, None).expect("sub");
            let expected_sub = home
                .join("example.com")
                .join("blog.example.com")
                .join("public_html")
                .to_string_lossy()
                .into_owned();
            assert_eq!(sub.docroot, expected_sub);
            assert!(!is_legacy_docroot(&sub.docroot));

            assert!(
                create_site("orphan.other.com", "admin", None, None, None).is_err(),
                "subdomain without parent must fail"
            );

            let listed = list_sites().unwrap();
            assert_eq!(listed.len(), 2);

            let updated = modify_site(
                "example.com",
                SiteModify {
                    owner: Some("ops".into()),
                    enabled: Some(false),
                    ..SiteModify::default()
                },
            )
            .unwrap();
            assert_eq!(updated.owner, "ops");
            assert!(!updated.enabled);

            delete_site("blog.example.com").unwrap();
            delete_site("example.com").unwrap();
            assert!(list_sites().unwrap().is_empty());
            // Files remain after registry delete.
            assert!(Path::new(&expected).join("index.html").is_file());
        });
    }

    #[test]
    fn legacy_docroot_detection() {
        with_temp_data(|| {
            assert!(is_legacy_docroot("/var/www/old.example.com/public_html"));
            assert!(is_legacy_docroot("/var/lib/cpn/sites/old.example.com"));
        });
    }
}
