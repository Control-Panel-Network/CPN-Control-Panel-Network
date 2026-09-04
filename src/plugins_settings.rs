//! Per-plugin settings under `/home/<domain>/plugins/<id>/settings.json`.

use crate::plugins::{normalize_plugin_id, plugins_dir_for_domain};
use crate::site_acl::{SitePerm, can_manage_site, sites_manageable_by};
use crate::sites::load_site;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

const SETTINGS_FILE: &str = "settings.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginSettingField {
    pub key: String,
    pub label: String,
    /// `text`, `checkbox`, or `number`.
    #[serde(default = "default_field_type")]
    pub field_type: String,
    #[serde(default)]
    pub default: String,
}

fn default_field_type() -> String {
    "text".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PluginSettings {
    #[serde(default)]
    pub show_in_sidebar: bool,
    #[serde(default)]
    pub fields: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct SidebarPluginLink {
    pub id: String,
    pub name: String,
    pub domain: String,
    pub href: String,
}

fn settings_path(domain: &str, plugin_id: &str) -> Result<PathBuf, String> {
    Ok(plugins_dir_for_domain(domain)?
        .join(plugin_id)
        .join(SETTINGS_FILE))
}

fn manifest_path(domain: &str, plugin_id: &str) -> Result<PathBuf, String> {
    Ok(plugins_dir_for_domain(domain)?
        .join(plugin_id)
        .join("cpn-plugin.json"))
}

/// Optional schema + sidebar default from `cpn-plugin.json` (extra keys ignored by core loader).
#[derive(Debug, Clone, Deserialize)]
struct ManifestExtras {
    #[serde(default)]
    settings_fields: Vec<PluginSettingField>,
    #[serde(default)]
    show_in_sidebar: Option<bool>,
    #[serde(default)]
    has_dashboard: bool,
}

fn load_manifest_extras(domain: &str, plugin_id: &str) -> ManifestExtras {
    let Ok(path) = manifest_path(domain, plugin_id) else {
        return ManifestExtras {
            settings_fields: Vec::new(),
            show_in_sidebar: None,
            has_dashboard: false,
        };
    };
    let Ok(raw) = fs::read_to_string(&path) else {
        return ManifestExtras {
            settings_fields: Vec::new(),
            show_in_sidebar: None,
            has_dashboard: false,
        };
    };
    serde_json::from_str(&raw).unwrap_or(ManifestExtras {
        settings_fields: Vec::new(),
        show_in_sidebar: None,
        has_dashboard: false,
    })
}

pub fn declared_settings_fields(domain: &str, plugin_id: &str) -> Vec<PluginSettingField> {
    load_manifest_extras(domain, plugin_id).settings_fields
}

pub fn manifest_has_dashboard(domain: &str, plugin_id: &str) -> bool {
    load_manifest_extras(domain, plugin_id).has_dashboard
}

pub fn default_show_in_sidebar(domain: &str, plugin_id: &str) -> bool {
    load_manifest_extras(domain, plugin_id)
        .show_in_sidebar
        .unwrap_or(false)
}

pub fn load_plugin_settings(
    domain_raw: &str,
    plugin_id_raw: &str,
) -> Result<PluginSettings, String> {
    let domain = load_site(domain_raw)?.domain;
    let id = normalize_plugin_id(plugin_id_raw)?;
    let path = settings_path(&domain, &id)?;
    if !path.is_file() {
        let mut settings = PluginSettings {
            show_in_sidebar: default_show_in_sidebar(&domain, &id),
            ..Default::default()
        };
        for field in declared_settings_fields(&domain, &id) {
            settings.fields.insert(field.key, field.default);
        }
        return Ok(settings);
    }
    let raw = fs::read_to_string(&path)
        .map_err(|error| format!("Could not read plugin settings: {error}"))?;
    let mut settings: PluginSettings =
        serde_json::from_str(&raw).map_err(|error| format!("Invalid settings.json: {error}"))?;
    // Ensure declared defaults exist when keys are missing.
    for field in declared_settings_fields(&domain, &id) {
        settings
            .fields
            .entry(field.key.clone())
            .or_insert_with(|| field.default.clone());
    }
    Ok(settings)
}

pub fn save_plugin_settings(
    domain_raw: &str,
    plugin_id_raw: &str,
    settings: &PluginSettings,
) -> Result<(), String> {
    let domain = load_site(domain_raw)?.domain;
    let id = normalize_plugin_id(plugin_id_raw)?;
    let path = settings_path(&domain, &id)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Could not create plugin dir: {error}"))?;
    }
    let raw = serde_json::to_string_pretty(settings)
        .map_err(|error| format!("Could not serialize settings: {error}"))?;
    fs::write(&path, raw).map_err(|error| format!("Could not write settings.json: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

/// Active plugins with Show in sidebar enabled, for sites the user may manage.
pub fn sidebar_plugin_links(username: &str) -> Vec<SidebarPluginLink> {
    let Ok(sites) = sites_manageable_by(username) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for site in sites {
        let Ok(allowed) = can_manage_site(username, &site.domain, SitePerm::Enable) else {
            continue;
        };
        if !allowed {
            continue;
        }
        let Ok(installed) = crate::plugins::list_installed(&site.domain) else {
            continue;
        };
        for item in installed {
            if !item.manifest.enabled {
                continue;
            }
            let settings =
                load_plugin_settings(&site.domain, &item.manifest.id).unwrap_or_default();
            if !settings.show_in_sidebar {
                continue;
            }
            let href = format!(
                "/plugins/dashboard?domain={}&id={}",
                urlencoding_simple(&site.domain),
                urlencoding_simple(&item.manifest.id)
            );
            out.push(SidebarPluginLink {
                id: item.manifest.id.clone(),
                name: item.manifest.name.clone(),
                domain: site.domain.clone(),
                href,
            });
        }
    }
    out.sort_by(|a, b| {
        (
            a.name.to_ascii_lowercase(),
            a.domain.to_ascii_lowercase(),
            a.id.to_ascii_lowercase(),
        )
            .cmp(&(
                b.name.to_ascii_lowercase(),
                b.domain.to_ascii_lowercase(),
                b.id.to_ascii_lowercase(),
            ))
    });
    out
}

fn urlencoding_simple(value: &str) -> String {
    let mut out = String::with_capacity(value.len() * 3);
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;
    use crate::plugins::CpnPluginManifest;
    use crate::sites::create_site;

    #[test]
    fn settings_roundtrip_and_sidebar_default() {
        with_test_data_dir(|| {
            let sites_home = std::env::temp_dir().join(format!(
                "cpn-pset-{}-{}",
                std::process::id(),
                crate::account::now_unix()
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
                description: "Demo".into(),
                author: "master3395".into(),
                pricing: "free".into(),
                enabled: true,
                installed_at_unix: 1,
                source: "test".into(),
                catalog_repo: "master3395/cyberpanel-plugins".into(),
                domain: "example.com".into(),
            };
            let raw = serde_json::to_string_pretty(&serde_json::json!({
                "schema_version": manifest.schema_version,
                "id": manifest.id,
                "name": manifest.name,
                "category": manifest.category,
                "version": manifest.version,
                "description": manifest.description,
                "author": manifest.author,
                "pricing": manifest.pricing,
                "enabled": true,
                "installed_at_unix": 1,
                "source": "test",
                "catalog_repo": manifest.catalog_repo,
                "domain": "example.com",
                "show_in_sidebar": true,
                "has_dashboard": true,
                "settings_fields": [{
                    "key": "greeting",
                    "label": "Greeting",
                    "field_type": "text",
                    "default": "Hello"
                }]
            }))
            .unwrap();
            fs::write(dir.join("cpn-plugin.json"), raw).unwrap();
            let loaded = load_plugin_settings("example.com", "demoPlugin").unwrap();
            assert!(loaded.show_in_sidebar);
            assert_eq!(
                loaded.fields.get("greeting").map(String::as_str),
                Some("Hello")
            );
            let mut next = loaded;
            next.show_in_sidebar = false;
            next.fields.insert("greeting".into(), "Hi".into());
            save_plugin_settings("example.com", "demoPlugin", &next).unwrap();
            let again = load_plugin_settings("example.com", "demoPlugin").unwrap();
            assert!(!again.show_in_sidebar);
            assert_eq!(again.fields.get("greeting").map(String::as_str), Some("Hi"));
            unsafe {
                std::env::remove_var("CPN_SITES_HOME");
            }
            let _ = fs::remove_dir_all(&sites_home);
        });
    }
}
