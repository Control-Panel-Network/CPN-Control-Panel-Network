//! Host-shared plugin installs and per-domain activation.
//! Host-scoped plugins install once under `$CPN_DATA_DIR/host-plugins/<id>/`.
//! Sites Activate/Deactivate that shared copy (no second full install).

use crate::account::{data_dir, now_unix};
use crate::plugins::{
    CatalogEntry, CpnPluginManifest, InstalledPlugin, normalize_plugin_id, sanitize_user_text,
};
use crate::plugins_catalog::{CATALOG_TARBALL, curl_bytes, parse_meta_xml};
use crate::sites::{load_site, site_plugins_dir};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const SCHEMA_VERSION: u32 = 1;
const CATALOG_REPO: &str = "Control-Panel-Network/CPN-Plugins";
const HOST_SCOPED_ALLOWLIST: &[&str] = &[
    "clamav",
    "fail2ban",
    "autoBan",
    "autoSnapshot",
    "malwareScanner",
    "roundcubeWebmail",
    "roundcube",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginActivation {
    pub domain: String,
    pub plugin_id: String,
    pub activated_at_unix: u64,
    #[serde(default = "default_source_host")]
    pub source: String,
}

fn default_source_host() -> String {
    "host".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ActivationsFile {
    #[serde(default)]
    schema_version: u32,
    #[serde(default)]
    activations: Vec<PluginActivation>,
}

fn activations_path() -> PathBuf {
    data_dir().join("plugin-activations.json")
}

fn load_activations() -> ActivationsFile {
    let Ok(raw) = fs::read_to_string(activations_path()) else {
        return ActivationsFile {
            schema_version: SCHEMA_VERSION,
            activations: Vec::new(),
        };
    };
    serde_json::from_str(&raw).unwrap_or(ActivationsFile {
        schema_version: SCHEMA_VERSION,
        activations: Vec::new(),
    })
}

fn save_activations(file: &ActivationsFile) -> Result<(), String> {
    let path = activations_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Could not create data dir: {e}"))?;
    }
    let mut out = file.clone();
    out.schema_version = SCHEMA_VERSION;
    let raw = serde_json::to_string_pretty(&out)
        .map_err(|e| format!("Could not serialize plugin activations: {e}"))?;
    fs::write(&path, raw).map_err(|e| format!("Could not write plugin activations: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

pub fn host_plugins_dir() -> PathBuf {
    data_dir().join("host-plugins")
}

pub fn host_plugin_path(plugin_id: &str) -> PathBuf {
    host_plugins_dir().join(plugin_id.trim())
}

pub fn host_plugin_installed(plugin_id: &str) -> bool {
    let id = plugin_id.trim();
    if id.is_empty() {
        return false;
    }
    host_plugin_path(id).join("cpn-plugin.json").is_file()
        || host_plugin_path(id).join("meta.xml").is_file()
}

pub fn is_host_scoped_plugin(id: &str) -> bool {
    let id = id.trim();
    !id.is_empty()
        && HOST_SCOPED_ALLOWLIST
            .iter()
            .any(|known| known.eq_ignore_ascii_case(id))
}

pub fn catalog_entry_is_host_scoped(entry: &CatalogEntry) -> bool {
    entry.host_scoped || is_host_scoped_plugin(&entry.id)
}

pub fn is_activated(domain_raw: &str, plugin_id: &str) -> bool {
    let domain = domain_raw.trim();
    let id = plugin_id.trim();
    if domain.is_empty() || id.is_empty() {
        return false;
    }
    load_activations().activations.iter().any(|a| {
        a.domain.eq_ignore_ascii_case(domain) && a.plugin_id.eq_ignore_ascii_case(id)
    })
}

pub fn list_activations_for_domain(domain_raw: &str) -> Vec<PluginActivation> {
    let domain = domain_raw.trim();
    load_activations()
        .activations
        .into_iter()
        .filter(|a| a.domain.eq_ignore_ascii_case(domain))
        .collect()
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

fn write_host_manifest(plugin_id: &str, entry: &CatalogEntry) -> Result<CpnPluginManifest, String> {
    let manifest = CpnPluginManifest {
        schema_version: SCHEMA_VERSION,
        id: entry.id.clone(),
        name: entry.name.clone(),
        category: entry.category.clone(),
        version: entry.version.clone(),
        description: entry.description.clone(),
        author: entry.author.clone(),
        pricing: entry.pricing.clone(),
        enabled: true,
        installed_at_unix: now_unix(),
        source: "host-catalog".into(),
        catalog_repo: CATALOG_REPO.into(),
        domain: String::new(),
    };
    let dest = host_plugin_path(plugin_id);
    let path = dest.join("cpn-plugin.json");
    let raw = serde_json::to_string_pretty(&manifest)
        .map_err(|e| format!("Could not serialize host plugin manifest: {e}"))?;
    fs::write(&path, raw).map_err(|e| format!("Could not write host plugin manifest: {e}"))?;
    Ok(manifest)
}

/// Install once for the whole host (admin-only at the route layer).
pub fn install_host_plugin(plugin_id: &str) -> Result<CpnPluginManifest, String> {
    let id = normalize_plugin_id(plugin_id)?;
    if host_plugin_installed(&id) {
        return Err(format!("Host plugin `{id}` is already installed"));
    }
    let bytes = curl_bytes(CATALOG_TARBALL)?;
    let tar_path =
        std::env::temp_dir().join(format!("cpn-host-plugin-{}.tar.gz", std::process::id()));
    let extract = std::env::temp_dir().join(format!("cpn-host-plugin-{}", std::process::id()));
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
    let dest = host_plugin_path(&id);
    if dest.exists() {
        let _ = fs::remove_dir_all(&dest);
    }
    copy_dir_recursive(&src, &dest)?;
    let _ = fs::remove_dir_all(&extract);
    write_host_manifest(&id, &entry)
}

/// Remove host-shared plugin (admin only). Clears domain activations for it.
pub fn uninstall_host_plugin(plugin_id: &str) -> Result<(), String> {
    let id = normalize_plugin_id(plugin_id)?;
    let dest = host_plugin_path(&id);
    if !dest.exists() {
        return Err(format!("Host plugin `{id}` is not installed"));
    }
    fs::remove_dir_all(&dest).map_err(|error| format!("Could not remove host plugin: {error}"))?;
    let mut file = load_activations();
    file.activations
        .retain(|a| !a.plugin_id.eq_ignore_ascii_case(&id));
    save_activations(&file)?;
    Ok(())
}

fn write_site_activation_stub(
    domain: &str,
    entry: &CpnPluginManifest,
) -> Result<PathBuf, String> {
    let site = load_site(domain)?;
    let dest = site_plugins_dir(&site).join(&entry.id);
    fs::create_dir_all(&dest).map_err(|e| format!("Could not create {}: {e}", dest.display()))?;
    let stub = serde_json::json!({
        "schema_version": SCHEMA_VERSION,
        "id": entry.id,
        "name": entry.name,
        "category": entry.category,
        "version": entry.version,
        "description": entry.description,
        "author": entry.author,
        "pricing": entry.pricing,
        "enabled": true,
        "installed_at_unix": now_unix(),
        "source": "host-activation",
        "catalog_repo": CATALOG_REPO,
        "domain": site.domain,
        "host_shared": true,
    });
    let path = dest.join("cpn-plugin.json");
    fs::write(
        &path,
        serde_json::to_string_pretty(&stub).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("Could not write activation stub: {e}"))?;
    let note = format!(
        "Activated from host install at {}.\nSite-scoped content stays under this domain; do not share host data across tenants.\n",
        host_plugin_path(&entry.id).display()
    );
    fs::write(dest.join("HOST-ACTIVATION.txt"), note)
        .map_err(|e| format!("Could not write activation note: {e}"))?;
    Ok(dest)
}

fn load_host_manifest(plugin_id: &str) -> Result<CpnPluginManifest, String> {
    let path = host_plugin_path(plugin_id).join("cpn-plugin.json");
    if path.is_file() {
        let raw = fs::read_to_string(&path)
            .map_err(|e| format!("Could not read host plugin manifest: {e}"))?;
        return serde_json::from_str(&raw)
            .map_err(|e| format!("Could not parse host plugin manifest: {e}"));
    }
    let meta = host_plugin_path(plugin_id).join("meta.xml");
    let body =
        fs::read_to_string(&meta).map_err(|e| format!("Could not read host meta.xml: {e}"))?;
    let entry = parse_meta_xml(plugin_id, &body)?;
    Ok(CpnPluginManifest {
        schema_version: SCHEMA_VERSION,
        id: entry.id,
        name: sanitize_user_text(&entry.name),
        category: entry.category,
        version: entry.version,
        description: entry.description,
        author: entry.author,
        pricing: entry.pricing,
        enabled: true,
        installed_at_unix: now_unix(),
        source: "host-catalog".into(),
        catalog_repo: CATALOG_REPO.into(),
        domain: String::new(),
    })
}

pub fn activate_host_plugin_for_domain(
    domain_raw: &str,
    plugin_id: &str,
) -> Result<CpnPluginManifest, String> {
    let site = load_site(domain_raw)?;
    let id = normalize_plugin_id(plugin_id)?;
    if !host_plugin_installed(&id) {
        return Err(format!(
            "Host plugin `{id}` is not installed. Ask the panel admin to install it on the Host first."
        ));
    }
    if is_activated(&site.domain, &id) {
        return Err(format!(
            "Plugin `{id}` is already activated for `{}`",
            site.domain
        ));
    }
    let manifest = load_host_manifest(&id)?;
    write_site_activation_stub(&site.domain, &manifest)?;
    let mut file = load_activations();
    file.activations.push(PluginActivation {
        domain: site.domain.clone(),
        plugin_id: id.clone(),
        activated_at_unix: now_unix(),
        source: "host".into(),
    });
    save_activations(&file)?;
    let mut out = manifest;
    out.domain = site.domain;
    out.enabled = true;
    out.source = "host-activation".into();
    Ok(out)
}

pub fn deactivate_host_plugin_for_domain(
    domain_raw: &str,
    plugin_id: &str,
) -> Result<(), String> {
    let site = load_site(domain_raw)?;
    let id = normalize_plugin_id(plugin_id)?;
    if !is_activated(&site.domain, &id) {
        return Err(format!(
            "Plugin `{id}` is not activated for `{}`",
            site.domain
        ));
    }
    let dest = site_plugins_dir(&site).join(&id);
    if dest.exists() {
        let stub = dest.join("cpn-plugin.json");
        let is_host_stub = fs::read_to_string(&stub)
            .ok()
            .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
            .map(|v| {
                v.get("source").and_then(|s| s.as_str()) == Some("host-activation")
                    || v.get("host_shared").and_then(|s| s.as_bool()) == Some(true)
            })
            .unwrap_or(false);
        if is_host_stub {
            let _ = fs::remove_dir_all(&dest);
        }
    }
    let mut file = load_activations();
    file.activations.retain(|a| {
        !(a.domain.eq_ignore_ascii_case(&site.domain) && a.plugin_id.eq_ignore_ascii_case(&id))
    });
    save_activations(&file)?;
    Ok(())
}

pub fn activated_as_installed(domain_raw: &str) -> Vec<InstalledPlugin> {
    let domain = domain_raw.trim();
    let mut out = Vec::new();
    for act in list_activations_for_domain(domain) {
        if let Ok(mut manifest) = load_host_manifest(&act.plugin_id) {
            manifest.domain = domain.to_string();
            manifest.enabled = true;
            manifest.source = "host-activation".into();
            let path = host_plugin_path(&act.plugin_id);
            out.push(InstalledPlugin {
                manifest,
                path,
                domain: domain.to_string(),
            });
        }
    }
    out
}

pub fn is_host_owned_install(domain_raw: &str, plugin_id: &str) -> bool {
    let id = plugin_id.trim();
    if host_plugin_installed(id) && is_activated(domain_raw, id) {
        return true;
    }
    if host_plugin_installed(id) && domain_raw.trim().is_empty() {
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;
    use crate::sites::create_site;

    #[test]
    fn allowlist_marks_clamav_host_scoped() {
        assert!(is_host_scoped_plugin("clamav"));
        assert!(is_host_scoped_plugin("fail2ban"));
        assert!(!is_host_scoped_plugin("bimi"));
    }

    #[test]
    fn activate_deactivate_roundtrip_without_catalog() {
        with_test_data_dir(|| {
            let home = std::env::temp_dir().join(format!(
                "cpn-act-{}-{}",
                std::process::id(),
                now_unix()
            ));
            let _ = fs::remove_dir_all(&home);
            fs::create_dir_all(&home).unwrap();
            unsafe {
                std::env::set_var("CPN_SITES_HOME", &home);
            }
            create_site("example.com", "admin", None, None, None).unwrap();
            let dest = host_plugin_path("clamav");
            fs::create_dir_all(&dest).unwrap();
            let entry = CatalogEntry {
                id: "clamav".into(),
                name: "ClamAV".into(),
                category: "Security".into(),
                version: "1.0.0".into(),
                description: "Host scanner".into(),
                author: "master3395".into(),
                pricing: "free".into(),
                released_on: String::new(),
                updated_on: String::new(),
                install_count: 0,
                featured: false,
                uninstall_impacts: vec![],
                host_scoped: true,
            };
            write_host_manifest("clamav", &entry).unwrap();
            assert!(host_plugin_installed("clamav"));
            assert!(!is_activated("example.com", "clamav"));
            activate_host_plugin_for_domain("example.com", "clamav").unwrap();
            assert!(is_activated("example.com", "clamav"));
            assert!(is_host_owned_install("example.com", "clamav"));
            deactivate_host_plugin_for_domain("example.com", "clamav").unwrap();
            assert!(!is_activated("example.com", "clamav"));
            assert!(host_plugin_installed("clamav"));
            unsafe {
                std::env::remove_var("CPN_SITES_HOME");
            }
            let _ = fs::remove_dir_all(&home);
        });
    }
}
