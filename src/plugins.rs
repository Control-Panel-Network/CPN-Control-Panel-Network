//! CPN plugin registry and install under `$CPN_DATA_DIR/plugins/`.
//!
//! Catalog source: https://github.com/master3395/cyberpanel-plugins (community plugin
//! archive). Internal adapters may map legacy `meta.xml` fields; user-facing copy is
//! always CPN-branded.

use crate::account::{data_dir, now_unix};
use crate::plugins_catalog::{CATALOG_TARBALL, curl_bytes, parse_meta_xml};
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
}

fn plugins_dir() -> PathBuf {
    data_dir().join("plugins")
}

/// Absolute install root for docs and CLI help.
pub fn plugins_install_path_display() -> String {
    plugins_dir().display().to_string()
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

fn manifest_path(plugin_id: &str) -> PathBuf {
    plugins_dir().join(plugin_id).join("cpn-plugin.json")
}

fn write_manifest(manifest: &CpnPluginManifest) -> Result<(), String> {
    let path = manifest_path(&manifest.id);
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

fn load_manifest(plugin_id: &str) -> Result<CpnPluginManifest, String> {
    let path = manifest_path(plugin_id);
    let raw = fs::read_to_string(&path)
        .map_err(|error| format!("Plugin `{plugin_id}` is not installed: {error}"))?;
    serde_json::from_str(&raw).map_err(|error| format!("Invalid cpn-plugin.json: {error}"))
}

pub fn list_installed() -> Result<Vec<InstalledPlugin>, String> {
    let dir = plugins_dir();
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    let entries =
        fs::read_dir(&dir).map_err(|error| format!("Could not read plugins dir: {error}"))?;
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
        match load_manifest(id) {
            Ok(manifest) => out.push(InstalledPlugin {
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
                    };
                    let _ = write_manifest(&manifest);
                    out.push(InstalledPlugin { manifest, path });
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

pub fn install_plugin(plugin_id: &str) -> Result<CpnPluginManifest, String> {
    let id = normalize_plugin_id(plugin_id)?;
    if manifest_path(&id).is_file() {
        return Err(format!("Plugin `{id}` is already installed"));
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
    let dest = plugins_dir().join(&id);
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
    };
    write_manifest(&manifest)?;
    Ok(manifest)
}

pub fn uninstall_plugin(plugin_id: &str) -> Result<(), String> {
    let id = normalize_plugin_id(plugin_id)?;
    let dest = plugins_dir().join(&id);
    if !dest.exists() {
        return Err(format!("Plugin `{id}` is not installed"));
    }
    fs::remove_dir_all(&dest).map_err(|error| format!("Could not remove plugin: {error}"))?;
    Ok(())
}

pub fn set_plugin_enabled(plugin_id: &str, enabled: bool) -> Result<CpnPluginManifest, String> {
    let id = normalize_plugin_id(plugin_id)?;
    let mut manifest = load_manifest(&id)?;
    manifest.enabled = enabled;
    write_manifest(&manifest)?;
    Ok(manifest)
}

// Re-exports used by panel + CLI.
pub use crate::plugins_catalog::{catalog_next_refresh_unix, fetch_catalog, format_unix_local};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;

    #[test]
    fn sanitize_strips_legacy_brand() {
        let text = sanitize_user_text("Discord login for CyberPanel admins");
        assert!(!text.to_ascii_lowercase().contains("cyberpanel"));
        assert!(text.contains("CPN Panel"));
    }

    #[test]
    fn install_enable_uninstall_roundtrip() {
        with_test_data_dir(|| {
            let dir = plugins_dir().join("demoPlugin");
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
            };
            write_manifest(&manifest).unwrap();
            assert_eq!(list_installed().unwrap().len(), 1);
            assert!(!set_plugin_enabled("demoPlugin", false).unwrap().enabled);
            uninstall_plugin("demoPlugin").unwrap();
            assert!(list_installed().unwrap().is_empty());
        });
    }
}
