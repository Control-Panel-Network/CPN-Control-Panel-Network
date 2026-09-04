//! CPN plugin registry: install under `/home/<domain>/plugins/<plugin-id>/`.
//!
//! Catalog source: https://github.com/master3395/cyberpanel-plugins (community plugin
//! archive). Internal adapters may map legacy `meta.xml` fields; user-facing copy is
//! always CPN-branded.
//!
//! Legacy installs under `$CPN_DATA_DIR/plugins/` are migrated into a chosen domain
//! home on first use (`migrate_legacy_plugins`).

use crate::account::{data_dir, now_unix};
use crate::plugins_catalog::{CATALOG_TARBALL, curl_bytes, parse_meta_xml};
use crate::sites::{load_site, site_plugins_dir};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

const SCHEMA_VERSION: u32 = 1;
const CATALOG_REPO: &str = "master3395/cyberpanel-plugins";
const SKIP_DIRS: &[&str] = &[".github", "docs", "scripts", "to-do", "test"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpnPluginManifest {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub category: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub pricing: String,
    pub enabled: bool,
    pub installed_at_unix: u64,
    pub source: String,
    pub catalog_repo: String,
    /// Domain this plugin is bound to (omitted on very old manifests).
    #[serde(default)]
    pub domain: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogEntry {
    pub id: String,
    pub name: String,
    pub category: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub pricing: String,
}

#[derive(Debug, Clone)]
pub struct InstalledPlugin {
    pub manifest: CpnPluginManifest,
    pub path: PathBuf,
    pub domain: String,
}

fn legacy_plugins_dir() -> PathBuf {
    data_dir().join("plugins")
}

/// Plugins root for a registered site: `/home/<domain>/plugins`.
pub fn plugins_dir_for_domain(domain_raw: &str) -> Result<PathBuf, String> {
    let site = load_site(domain_raw)?;
    Ok(site_plugins_dir(&site))
}

/// Absolute install root for docs and CLI help (example path).
pub fn plugins_install_path_display(domain: Option<&str>) -> String {
    match domain {
        Some(d) if !d.trim().is_empty() => match plugins_dir_for_domain(d) {
            Ok(path) => path.display().to_string(),
            Err(_) => format!("/home/{}/plugins", d.trim()),
        },
        _ => "/home/<domain>/plugins".into(),
    }
}

pub fn catalog_repo_url() -> &'static str {
    "https://github.com/master3395/cyberpanel-plugins"
}

pub fn catalog_repo_slug() -> &'static str {
    CATALOG_REPO
}

/// Strip legacy product names from user-facing plugin text.
pub fn sanitize_user_text(raw: &str) -> String {
    let mut out = raw.to_string();
    for (from, to) in [
        ("CyberPanel", "CPN Panel"),
        ("cyberpanel", "CPN"),
        ("CYBERPANEL", "CPN"),
    ] {
        out = out.replace(from, to);
    }
    out
}

fn has_control_chars(value: &str) -> bool {
    value.chars().any(|ch| ch.is_control())
}

/// Validate a plugin directory / catalog id.
pub fn normalize_plugin_id(raw: &str) -> Result<String, String> {
    let id = raw.trim().to_string();
    if id.is_empty() {
        return Err("Plugin id is required".into());
    }
    if id.chars().count() > 64 {
        return Err("Plugin id is too long".into());
    }
    if has_control_chars(&id) {
        return Err("Plugin id cannot include control characters".into());
    }
    if !id
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        return Err("Plugin id may only contain letters, digits, underscore, and hyphen".into());
    }
    if SKIP_DIRS.contains(&id.as_str()) {
        return Err("That name is reserved and cannot be installed as a plugin".into());
    }
    Ok(id)
}

fn require_domain(domain_raw: &str) -> Result<String, String> {
    let domain = domain_raw.trim().to_lowercase();
    if domain.is_empty() {
        return Err("Domain is required (plugins install under /home/<domain>/plugins)".into());
    }
    let site = load_site(&domain)?;
    Ok(site.domain)
}

fn manifest_path(domain: &str, plugin_id: &str) -> Result<PathBuf, String> {
    Ok(plugins_dir_for_domain(domain)?
        .join(plugin_id)
        .join("cpn-plugin.json"))
}

fn write_manifest(domain: &str, manifest: &CpnPluginManifest) -> Result<(), String> {
    let path = manifest_path(domain, &manifest.id)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Could not create plugin dir: {error}"))?;
    }
    let raw = serde_json::to_string_pretty(manifest)
        .map_err(|error| format!("Could not serialize plugin manifest: {error}"))?;
    fs::write(&path, raw).map_err(|error| format!("Could not write plugin manifest: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn load_manifest(domain: &str, plugin_id: &str) -> Result<CpnPluginManifest, String> {
    let path = manifest_path(domain, plugin_id)?;
    let raw = fs::read_to_string(&path)
        .map_err(|error| format!("Plugin `{plugin_id}` is not installed on `{domain}`: {error}"))?;
    let mut manifest: CpnPluginManifest =
        serde_json::from_str(&raw).map_err(|error| format!("Invalid cpn-plugin.json: {error}"))?;
    if manifest.domain.is_empty() {
        manifest.domain = domain.to_string();
    }
    Ok(manifest)
}

/// Move legacy `$CPN_DATA_DIR/plugins/*` into `/home/<domain>/plugins/` when present.
pub fn migrate_legacy_plugins(domain_raw: &str) -> Result<usize, String> {
    let domain = require_domain(domain_raw)?;
    let legacy = legacy_plugins_dir();
    if !legacy.is_dir() {
        return Ok(0);
    }
    let dest_root = plugins_dir_for_domain(&domain)?;
    fs::create_dir_all(&dest_root)
        .map_err(|error| format!("Could not create {}: {error}", dest_root.display()))?;
    let mut moved = 0usize;
    let entries = fs::read_dir(&legacy)
        .map_err(|error| format!("Could not read legacy plugins dir: {error}"))?;
    for entry in entries.flatten() {
        let from = entry.path();
        if !from.is_dir() {
            continue;
        }
        let Some(id) = from.file_name().and_then(|v| v.to_str()) else {
            continue;
        };
        if normalize_plugin_id(id).is_err() {
            continue;
        }
        let dest = dest_root.join(id);
        if dest.exists() {
            continue;
        }
        fs::rename(&from, &dest).or_else(|_| {
            copy_dir_recursive(&from, &dest)?;
            fs::remove_dir_all(&from).map_err(|error| {
                format!(
                    "Copied legacy plugin but could not remove {}: {error}",
                    from.display()
                )
            })
        })?;
        if let Ok(mut manifest) = load_manifest(&domain, id) {
            manifest.domain = domain.clone();
            let _ = write_manifest(&domain, &manifest);
        }
        moved += 1;
    }
    let _ = fs::remove_dir(legacy);
    Ok(moved)
}

fn list_installed_in_dir(domain: &str, dir: &Path) -> Result<Vec<InstalledPlugin>, String> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    let entries =
        fs::read_dir(dir).map_err(|error| format!("Could not read plugins dir: {error}"))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(id) = path.file_name().and_then(|v| v.to_str()) else {
            continue;
        };
        if normalize_plugin_id(id).is_err() {
            continue;
        }
        match load_manifest(domain, id) {
            Ok(manifest) => out.push(InstalledPlugin {
                domain: domain.to_string(),
                manifest,
                path: path.clone(),
            }),
            Err(_) => {
                let meta = path.join("meta.xml");
                if meta.is_file()
                    && let Ok(body) = fs::read_to_string(&meta)
                    && let Ok(entry) = parse_meta_xml(id, &body)
                {
                    let manifest = CpnPluginManifest {
                        schema_version: SCHEMA_VERSION,
                        id: entry.id.clone(),
                        name: entry.name,
                        category: entry.category,
                        version: entry.version,
                        description: entry.description,
                        author: entry.author,
                        pricing: entry.pricing,
                        enabled: true,
                        installed_at_unix: now_unix(),
                        source: "compat".into(),
                        catalog_repo: CATALOG_REPO.into(),
                        domain: domain.to_string(),
                    };
                    let _ = write_manifest(domain, &manifest);
                    out.push(InstalledPlugin {
                        domain: domain.to_string(),
                        manifest,
                        path,
                    });
                }
            }
        }
    }
    out.sort_by(|a, b| {
        a.manifest
            .name
            .to_lowercase()
            .cmp(&b.manifest.name.to_lowercase())
    });
    Ok(out)
}

/// List plugins for one domain (runs legacy migration first).
pub fn list_installed(domain_raw: &str) -> Result<Vec<InstalledPlugin>, String> {
    let domain = require_domain(domain_raw)?;
    let _ = migrate_legacy_plugins(&domain);
    let dir = plugins_dir_for_domain(&domain)?;
    list_installed_in_dir(&domain, &dir)
}

/// List plugins across all registered sites (optional CLI overview).
pub fn list_installed_all() -> Result<Vec<InstalledPlugin>, String> {
    let sites = crate::sites::list_sites()?;
    let mut out = Vec::new();
    for site in sites {
        let _ = migrate_legacy_plugins(&site.domain);
        let dir = site_plugins_dir(&site);
        out.extend(list_installed_in_dir(&site.domain, &dir)?);
    }
    out.sort_by(|a, b| {
        (a.domain.to_lowercase(), a.manifest.name.to_lowercase())
            .cmp(&(b.domain.to_lowercase(), b.manifest.name.to_lowercase()))
    });
    Ok(out)
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst).map_err(|error| format!("Could not create {dst:?}: {error}"))?;
    let entries = fs::read_dir(src).map_err(|error| format!("Could not read {src:?}: {error}"))?;
    for entry in entries.flatten() {
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            copy_dir_recursive(&from, &to)?;
        } else {
            if let Some(parent) = to.parent() {
                fs::create_dir_all(parent)
                    .map_err(|error| format!("Could not create {parent:?}: {error}"))?;
            }
            fs::copy(&from, &to).map_err(|error| format!("Could not copy {from:?}: {error}"))?;
        }
    }
    Ok(())
}

fn find_plugin_in_extract(root: &Path, plugin_id: &str) -> Option<PathBuf> {
    let Ok(entries) = fs::read_dir(root) else {
        return None;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let candidate = path.join(plugin_id);
        if candidate.is_dir() && candidate.join("meta.xml").is_file() {
            return Some(candidate);
        }
        if path.file_name().and_then(|v| v.to_str()) == Some(plugin_id)
            && path.join("meta.xml").is_file()
        {
            return Some(path);
        }
        if let Some(found) = find_plugin_in_extract(&path, plugin_id) {
            return Some(found);
        }
    }
    None
}

pub fn install_plugin(domain_raw: &str, plugin_id: &str) -> Result<CpnPluginManifest, String> {
    let domain = require_domain(domain_raw)?;
    let id = normalize_plugin_id(plugin_id)?;
    let _ = migrate_legacy_plugins(&domain);
    if manifest_path(&domain, &id)?.is_file() {
        return Err(format!("Plugin `{id}` is already installed on `{domain}`"));
    }
    let bytes = curl_bytes(CATALOG_TARBALL)?;
    let tar_path =
        std::env::temp_dir().join(format!("cpn-plugin-install-{}.tar.gz", std::process::id()));
    let extract = std::env::temp_dir().join(format!("cpn-plugin-install-{}", std::process::id()));
    let _ = fs::remove_dir_all(&extract);
    fs::create_dir_all(&extract).map_err(|error| format!("Could not create temp dir: {error}"))?;
    fs::write(&tar_path, &bytes).map_err(|error| format!("Could not write tarball: {error}"))?;
    let status = Command::new("tar")
        .args(["-xzf"])
        .arg(&tar_path)
        .arg("-C")
        .arg(&extract)
        .status()
        .map_err(|error| format!("Could not extract plugin archive: {error}"))?;
    let _ = fs::remove_file(&tar_path);
    if !status.success() {
        let _ = fs::remove_dir_all(&extract);
        return Err("Failed to extract plugin archive".into());
    }
    let Some(src) = find_plugin_in_extract(&extract, &id) else {
        let _ = fs::remove_dir_all(&extract);
        return Err(format!(
            "Plugin `{id}` was not found in the catalog archive"
        ));
    };
    let meta_body = fs::read_to_string(src.join("meta.xml"))
        .map_err(|error| format!("Could not read meta.xml: {error}"))?;
    let entry = parse_meta_xml(&id, &meta_body)?;
    let dest = plugins_dir_for_domain(&domain)?.join(&id);
    if dest.exists() {
        let _ = fs::remove_dir_all(&dest);
    }
    copy_dir_recursive(&src, &dest)?;
    let _ = fs::remove_dir_all(&extract);
    let manifest = CpnPluginManifest {
        schema_version: SCHEMA_VERSION,
        id: entry.id,
        name: entry.name,
        category: entry.category,
        version: entry.version,
        description: entry.description,
        author: entry.author,
        pricing: entry.pricing,
        enabled: true,
        installed_at_unix: now_unix(),
        source: "catalog".into(),
        catalog_repo: CATALOG_REPO.into(),
        domain: domain.clone(),
    };
    write_manifest(&domain, &manifest)?;
    Ok(manifest)
}

pub fn uninstall_plugin(domain_raw: &str, plugin_id: &str) -> Result<(), String> {
    let domain = require_domain(domain_raw)?;
    let id = normalize_plugin_id(plugin_id)?;
    let dest = plugins_dir_for_domain(&domain)?.join(&id);
    if !dest.exists() {
        return Err(format!("Plugin `{id}` is not installed on `{domain}`"));
    }
    fs::remove_dir_all(&dest).map_err(|error| format!("Could not remove plugin: {error}"))?;
    Ok(())
}

pub fn set_plugin_enabled(
    domain_raw: &str,
    plugin_id: &str,
    enabled: bool,
) -> Result<CpnPluginManifest, String> {
    let domain = require_domain(domain_raw)?;
    let id = normalize_plugin_id(plugin_id)?;
    let mut manifest = load_manifest(&domain, &id)?;
    manifest.enabled = enabled;
    manifest.domain = domain.clone();
    write_manifest(&domain, &manifest)?;
    Ok(manifest)
}

// Re-exports used by panel + CLI.
pub use crate::plugins_catalog::{catalog_next_refresh_unix, fetch_catalog, format_unix_local};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;
    use crate::sites::create_site;

    #[test]
    fn sanitize_strips_legacy_brand() {
        let text = sanitize_user_text("Discord login for CyberPanel admins");
        assert!(!text.to_ascii_lowercase().contains("cyberpanel"));
        assert!(text.contains("CPN Panel"));
    }

    #[test]
    fn install_enable_uninstall_roundtrip_per_domain() {
        with_test_data_dir(|| {
            let sites_home = std::env::temp_dir().join(format!(
                "cpn-sites-{}-{}",
                std::process::id(),
                now_unix()
            ));
            let _ = fs::remove_dir_all(&sites_home);
            fs::create_dir_all(&sites_home).unwrap();
            unsafe {
                std::env::set_var("CPN_SITES_HOME", &sites_home);
            }
            create_site("example.com", "admin", None, None, None).unwrap();
            let dir = plugins_dir_for_domain("example.com")
                .unwrap()
                .join("demoPlugin");
            fs::create_dir_all(&dir).unwrap();
            let manifest = CpnPluginManifest {
                schema_version: 1,
                id: "demoPlugin".into(),
                name: "Demo".into(),
                category: "Utility".into(),
                version: "1.0.0".into(),
                description: "Demo plugin".into(),
                author: "master3395".into(),
                pricing: "free".into(),
                enabled: true,
                installed_at_unix: 1,
                source: "test".into(),
                catalog_repo: CATALOG_REPO.into(),
                domain: "example.com".into(),
            };
            write_manifest("example.com", &manifest).unwrap();
            assert_eq!(list_installed("example.com").unwrap().len(), 1);
            assert!(
                !set_plugin_enabled("example.com", "demoPlugin", false)
                    .unwrap()
                    .enabled
            );
            uninstall_plugin("example.com", "demoPlugin").unwrap();
            assert!(list_installed("example.com").unwrap().is_empty());
            unsafe {
                std::env::remove_var("CPN_SITES_HOME");
            }
            let _ = fs::remove_dir_all(&sites_home);
        });
    }
}
