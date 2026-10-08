//! CPN-only plugin installs: account-scoped under `$CPN_DATA_DIR/user-plugins/<user>/<id>/`.
//! Not a public site app (outside `public_html`; data dir is not web-served).

use crate::account::{data_dir, now_unix};
use crate::plugins::{
    CatalogEntry, CpnPluginManifest, InstalledPlugin, normalize_plugin_id, sanitize_user_text,
};
use crate::plugins_catalog::{CATALOG_TARBALL, curl_bytes, parse_meta_xml};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const SCHEMA_VERSION: u32 = 1;
const CATALOG_REPO: &str = "Control-Panel-Network/CPN-Plugins";

/// URL / form domain token for CPN-only installs: `_cpn:<username>`.
pub const CPN_DOMAIN_PREFIX: &str = "_cpn:";

/// Panel utilities that default to CPN-only (and may also allow Host via catalog scope).
const CPN_SCOPED_ALLOWLIST: &[&str] = &["autoBanSecurityAlerts", "autoBan"];

pub fn cpn_domain_for_user(username: &str) -> String {
    format!(
        "{}{}",
        CPN_DOMAIN_PREFIX,
        sanitize_username(username)
    )
}

pub fn parse_cpn_owner(domain_raw: &str) -> Option<String> {
    let d = domain_raw.trim();
    let rest = d.strip_prefix(CPN_DOMAIN_PREFIX)?;
    let user = sanitize_username(rest);
    if user.is_empty() {
        None
    } else {
        Some(user)
    }
}

pub fn is_cpn_domain(domain_raw: &str) -> bool {
    parse_cpn_owner(domain_raw).is_some()
}

fn sanitize_username(raw: &str) -> String {
    raw.trim()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-' || *c == '.')
        .collect::<String>()
        .to_ascii_lowercase()
}

pub fn user_plugins_root() -> PathBuf {
    data_dir().join("user-plugins")
}

pub fn cpn_user_plugins_dir(username: &str) -> PathBuf {
    user_plugins_root().join(sanitize_username(username))
}

pub fn cpn_plugin_path(username: &str, plugin_id: &str) -> PathBuf {
    cpn_user_plugins_dir(username).join(plugin_id.trim())
}

pub fn cpn_plugin_installed(username: &str, plugin_id: &str) -> bool {
    let id = plugin_id.trim();
    if id.is_empty() || sanitize_username(username).is_empty() {
        return false;
    }
    let root = cpn_plugin_path(username, id);
    root.join("cpn-plugin.json").is_file() || root.join("meta.xml").is_file()
}

pub fn is_cpn_scoped_plugin(id: &str) -> bool {
    let id = id.trim();
    !id.is_empty()
        && CPN_SCOPED_ALLOWLIST
            .iter()
            .any(|known| known.eq_ignore_ascii_case(id))
}

pub fn catalog_entry_is_cpn_installable(entry: &CatalogEntry) -> bool {
    entry.cpn_installable || is_cpn_scoped_plugin(&entry.id)
}

/// Comma-separated scopes for `data-store-target` (host / cpn / site).
pub fn store_target_attr(entry: &CatalogEntry) -> String {
    let host = entry.host_scoped || crate::plugin_activation::is_host_scoped_plugin(&entry.id);
    let cpn = catalog_entry_is_cpn_installable(entry);
    let site = crate::plugin_activation::catalog_entry_is_site_installable(entry);
    let mut parts = Vec::new();
    if host {
        parts.push("host");
    }
    if cpn {
        parts.push("cpn");
    }
    if site {
        parts.push("site");
    }
    if parts.is_empty() {
        "site".into()
    } else {
        parts.join(",")
    }
}

pub fn scope_badges_html(entry: &CatalogEntry) -> String {
    let host = entry.host_scoped || crate::plugin_activation::is_host_scoped_plugin(&entry.id);
    let cpn = catalog_entry_is_cpn_installable(entry);
    let site = crate::plugin_activation::catalog_entry_is_site_installable(entry);
    let mut out = String::new();
    if host {
        out.push_str(r#"<span class="plugin-badge">Host</span>"#);
    }
    if cpn {
        out.push_str(r#"<span class="plugin-badge cat">CPN</span>"#);
    }
    if site {
        out.push_str(r#"<span class="plugin-badge cat">Site</span>"#);
    }
    if out.is_empty() {
        out.push_str(r#"<span class="plugin-badge cat">Site</span>"#);
    }
    out
}

fn ensure_private_dir(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path).map_err(|e| format!("Could not create {}: {e}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o700));
    }
    // Defense in depth if tree is ever placed under a web-adjacent path.
    let deny = path.join(".htaccess");
    if !deny.is_file() {
        let _ = fs::write(
            &deny,
            "Require all denied\nDeny from all\nOptions -Indexes\n",
        );
    }
    let note = path.join("CPN-ONLY.txt");
    if !note.is_file() {
        let _ = fs::write(
            &note,
            "CPN-only plugin install. Account-scoped under /var/lib/cpn/user-plugins/. Not a public site app.\n",
        );
    }
    Ok(())
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

fn write_cpn_manifest(
    username: &str,
    plugin_id: &str,
    entry: &CatalogEntry,
) -> Result<CpnPluginManifest, String> {
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
        source: "cpn-account".into(),
        catalog_repo: CATALOG_REPO.into(),
        domain: cpn_domain_for_user(username),
        uninstall_impacts: entry.uninstall_impacts.clone(),
    };
    let dest = cpn_plugin_path(username, plugin_id);
    let path = dest.join("cpn-plugin.json");
    let raw = serde_json::to_string_pretty(&manifest)
        .map_err(|e| format!("Could not serialize CPN plugin manifest: {e}"))?;
    fs::write(&path, raw).map_err(|e| format!("Could not write CPN plugin manifest: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    Ok(manifest)
}

fn load_cpn_manifest(username: &str, plugin_id: &str) -> Result<CpnPluginManifest, String> {
    let path = cpn_plugin_path(username, plugin_id).join("cpn-plugin.json");
    if path.is_file() {
        let raw = fs::read_to_string(&path)
            .map_err(|e| format!("Could not read CPN plugin manifest: {e}"))?;
        return serde_json::from_str(&raw)
            .map_err(|e| format!("Invalid CPN plugin manifest: {e}"));
    }
    let meta = cpn_plugin_path(username, plugin_id).join("meta.xml");
    let body = fs::read_to_string(&meta)
        .map_err(|e| format!("Could not read meta.xml: {e}"))?;
    let entry = parse_meta_xml(plugin_id, &body)?;
    write_cpn_manifest(username, plugin_id, &entry)
}

/// Install for the signed-in CPN user under `$CPN_DATA_DIR/user-plugins/<user>/<id>/`.
pub fn install_cpn_plugin(
    username: &str,
    plugin_id: &str,
) -> Result<CpnPluginManifest, String> {
    let user = sanitize_username(username);
    if user.is_empty() {
        return Err("Missing CPN username for account-scoped install".into());
    }
    let id = normalize_plugin_id(plugin_id)?;
    if !is_cpn_scoped_plugin(&id) {
        // Allow catalog-declared CPN scope even when not on the allowlist.
        // Route layer still checks catalog_entry_is_cpn_installable when possible.
    }
    if cpn_plugin_installed(&user, &id) {
        return Err(format!("CPN plugin `{id}` is already installed for `{user}`"));
    }
    let root = cpn_user_plugins_dir(&user);
    ensure_private_dir(&root)?;
    let bytes = curl_bytes(CATALOG_TARBALL)?;
    let tar_path =
        std::env::temp_dir().join(format!("cpn-user-plugin-{}.tar.gz", std::process::id()));
    let extract = std::env::temp_dir().join(format!("cpn-user-plugin-{}", std::process::id()));
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
    if !catalog_entry_is_cpn_installable(&entry) && !is_cpn_scoped_plugin(&id) {
        let _ = fs::remove_dir_all(&extract);
        return Err(format!(
            "Plugin `{id}` does not support CPN-only install. Use Site or Host instead."
        ));
    }
    let dest = cpn_plugin_path(&user, &id);
    if dest.exists() {
        let _ = fs::remove_dir_all(&dest);
    }
    copy_dir_recursive(&src, &dest)?;
    let _ = fs::remove_dir_all(&extract);
    ensure_private_dir(&dest)?;
    let _ = sanitize_user_text(&entry.name);
    write_cpn_manifest(&user, &id, &entry)
}

pub fn uninstall_cpn_plugin(username: &str, plugin_id: &str) -> Result<(), String> {
    let user = sanitize_username(username);
    let id = normalize_plugin_id(plugin_id)?;
    let dest = cpn_plugin_path(&user, &id);
    if !dest.exists() {
        return Err(format!("CPN plugin `{id}` is not installed for `{user}`"));
    }
    fs::remove_dir_all(&dest).map_err(|error| format!("Could not remove CPN plugin: {error}"))?;
    Ok(())
}

pub fn set_cpn_plugin_enabled(
    username: &str,
    plugin_id: &str,
    enabled: bool,
) -> Result<CpnPluginManifest, String> {
    let user = sanitize_username(username);
    let id = normalize_plugin_id(plugin_id)?;
    let mut manifest = load_cpn_manifest(&user, &id)?;
    manifest.enabled = enabled;
    manifest.domain = cpn_domain_for_user(&user);
    let path = cpn_plugin_path(&user, &id).join("cpn-plugin.json");
    let raw = serde_json::to_string_pretty(&manifest)
        .map_err(|e| format!("Could not serialize CPN plugin manifest: {e}"))?;
    fs::write(&path, raw).map_err(|e| format!("Could not write CPN plugin manifest: {e}"))?;
    Ok(manifest)
}

pub fn list_cpn_installed_plugins(username: &str) -> Vec<InstalledPlugin> {
    let user = sanitize_username(username);
    if user.is_empty() {
        return Vec::new();
    }
    let root = cpn_user_plugins_dir(&user);
    let Ok(entries) = fs::read_dir(&root) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let id = entry.file_name().to_string_lossy().to_string();
        if id.is_empty() || id.starts_with('.') {
            continue;
        }
        if !cpn_plugin_installed(&user, &id) {
            continue;
        }
        if let Ok(manifest) = load_cpn_manifest(&user, &id) {
            out.push(InstalledPlugin {
                manifest,
                path: path.clone(),
                domain: cpn_domain_for_user(&user),
            });
        }
    }
    out.sort_by(|a, b| {
        a.manifest
            .name
            .to_lowercase()
            .cmp(&b.manifest.name.to_lowercase())
    });
    out
}

pub fn cpn_settings_path(username: &str, plugin_id: &str) -> PathBuf {
    cpn_plugin_path(username, plugin_id).join("settings.json")
}

pub fn cpn_manifest_path(username: &str, plugin_id: &str) -> PathBuf {
    cpn_plugin_path(username, plugin_id).join("cpn-plugin.json")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;

    #[test]
    fn cpn_domain_roundtrip() {
        let d = cpn_domain_for_user("CpnOwner");
        assert_eq!(d, "_cpn:cpnowner");
        assert_eq!(parse_cpn_owner(&d).as_deref(), Some("cpnowner"));
        assert!(is_cpn_domain(&d));
        assert!(!is_cpn_domain("example.com"));
    }

    #[test]
    fn store_target_attr_combines_scopes() {
        let mut entry = CatalogEntry {
            id: "autoBanSecurityAlerts".into(),
            name: "Auto Ban".into(),
            category: "Security".into(),
            version: "1.0.0".into(),
            description: "x".into(),
            author: "master3395".into(),
            pricing: "paid".into(),
            released_on: String::new(),
            updated_on: String::new(),
            install_count: 0,
            featured: false,
            uninstall_impacts: vec![],
            host_scoped: true,
            site_installable: false,
            cpn_installable: true,
            keywords: vec![],
        };
        assert_eq!(store_target_attr(&entry), "host,cpn");
        entry.host_scoped = false;
        entry.site_installable = true;
        assert_eq!(store_target_attr(&entry), "cpn,site");
    }

    #[test]
    fn list_empty_without_installs() {
        with_test_data_dir(|| {
            assert!(list_cpn_installed_plugins("alice").is_empty());
            assert!(!cpn_plugin_installed("alice", "autoBanSecurityAlerts"));
        });
    }

    #[test]
    fn ensure_private_dir_writes_deny() {
        with_test_data_dir(|| {
            let dir = cpn_user_plugins_dir("bob");
            ensure_private_dir(&dir).unwrap();
            assert!(dir.join(".htaccess").is_file());
            assert!(dir.join("CPN-ONLY.txt").is_file());
        });
    }
}
