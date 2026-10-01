//! Paid CPN theme entitlements (Shop Grants / activation keys via api.newstargeted.com).

use crate::account::{data_dir, now_unix};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf, process::Command};

const API_BASE: &str = "https://api.newstargeted.com";
const VERIFY_PATH: &str = "/api/cpn-themes/v1/verify";
const PACKAGE_PATH: &str = "/api/cpn-themes/v1/package";
const PAID_CATALOG_PATH: &str = "/api/cpn-themes/v1/catalog";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ThemeLicenseFile {
    #[serde(default)]
    pub schema_version: u32,
    #[serde(default)]
    pub license_email: String,
    #[serde(default)]
    pub entries: Vec<ThemeLicenseEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeLicenseEntry {
    pub theme_id: String,
    pub grant_plugin: String,
    #[serde(default)]
    pub activation_key: String,
    #[serde(default)]
    pub entitled: bool,
    #[serde(default)]
    pub verified_at_unix: u64,
    #[serde(default)]
    pub via: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaidThemeMeta {
    pub id: String,
    pub name: String,
    pub description: String,
    pub author: String,
    pub version: String,
    pub grant_plugin: String,
    #[serde(default)]
    pub purchase_url: String,
    #[serde(default)]
    pub store_url: String,
    pub tokens: crate::panel_theme::DesignTokens,
    #[serde(default)]
    pub background: Option<crate::panel_theme::ThemeBackground>,
}

fn licenses_path() -> PathBuf {
    data_dir().join("theme-licenses.json")
}

pub fn load_theme_licenses() -> ThemeLicenseFile {
    let Ok(raw) = fs::read_to_string(licenses_path()) else {
        return ThemeLicenseFile {
            schema_version: 1,
            ..ThemeLicenseFile::default()
        };
    };
    serde_json::from_str(&raw).unwrap_or_else(|_| ThemeLicenseFile {
        schema_version: 1,
        ..ThemeLicenseFile::default()
    })
}

fn save_theme_licenses(file: &ThemeLicenseFile) -> Result<(), String> {
    let path = licenses_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Could not create data dir: {e}"))?;
    }
    let raw = serde_json::to_string_pretty(file)
        .map_err(|e| format!("Could not serialize theme licenses: {e}"))?;
    fs::write(&path, raw).map_err(|e| format!("Could not write theme licenses: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

pub fn theme_is_paid(pricing: &str) -> bool {
    let lower = pricing.trim().to_ascii_lowercase();
    lower == "paid" || lower == "premium"
}

pub fn local_theme_entitled(theme_id: &str) -> bool {
    let id = theme_id.trim().to_ascii_lowercase();
    load_theme_licenses()
        .entries
        .iter()
        .any(|e| e.theme_id.eq_ignore_ascii_case(&id) && e.entitled)
}

fn curl_json_post(url: &str, body: &str) -> Result<serde_json::Value, String> {
    let output = Command::new("curl")
        .args([
            "--fail",
            "--silent",
            "--show-error",
            "--max-time",
            "25",
            "-X",
            "POST",
            "-H",
            "Content-Type: application/json",
            "-H",
            "Accept: application/json",
            "-H",
            "User-Agent: cpn-panel-themes/1.0",
            "--data-binary",
            body,
            url,
        ])
        .output()
        .map_err(|e| format!("Could not call theme API: {e}"))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "Theme API request failed: {}",
            err.trim().chars().take(160).collect::<String>()
        ));
    }
    let raw = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(&raw).map_err(|e| format!("Invalid theme API JSON: {e}"))
}

fn curl_bytes_post(url: &str, body: &str) -> Result<Vec<u8>, String> {
    let output = Command::new("curl")
        .args([
            "--fail",
            "--silent",
            "--show-error",
            "--max-time",
            "60",
            "-X",
            "POST",
            "-H",
            "Content-Type: application/json",
            "-H",
            "Accept: application/gzip, application/json",
            "-H",
            "User-Agent: cpn-panel-themes/1.0",
            "--data-binary",
            body,
            url,
        ])
        .output()
        .map_err(|e| format!("Could not download theme package: {e}"))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        let body_hint = String::from_utf8_lossy(&output.stdout);
        if body_hint.trim_start().starts_with('{') {
            return Err(body_hint.chars().take(240).collect());
        }
        return Err(format!(
            "Theme package download failed: {}",
            err.trim().chars().take(160).collect::<String>()
        ));
    }
    Ok(output.stdout)
}

/// Fetch paid theme metadata from api.newstargeted.com (public catalog).
pub fn fetch_paid_themes_catalog() -> Result<Vec<PaidThemeMeta>, String> {
    let url = format!("{API_BASE}{PAID_CATALOG_PATH}");
    let output = Command::new("curl")
        .args([
            "--fail",
            "--silent",
            "--show-error",
            "--max-time",
            "20",
            "-H",
            "Accept: application/json",
            "-H",
            "User-Agent: cpn-panel-themes/1.0",
            &url,
        ])
        .output()
        .map_err(|e| format!("Could not fetch paid themes catalog: {e}"))?;
    if !output.status.success() {
        return Err("Paid themes catalog request failed".into());
    }
    let raw = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("Invalid paid catalog JSON: {e}"))?;
    let Some(arr) = parsed.get("themes").and_then(|v| v.as_array()) else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for item in arr {
        let id = item
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if id.is_empty() {
            continue;
        }
        let tokens: crate::panel_theme::DesignTokens = match serde_json::from_value(
            item.get("tokens")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({})),
        ) {
            Ok(t) => t,
            Err(_) => continue,
        };
        let Ok(tokens) = tokens.validate() else {
            continue;
        };
        let background = item
            .get("background")
            .cloned()
            .and_then(|v| serde_json::from_value::<crate::panel_theme::ThemeBackground>(v).ok())
            .and_then(|bg| bg.validate().ok())
            .filter(|bg| !bg.is_empty());
        out.push(PaidThemeMeta {
            id,
            name: item
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("Paid theme")
                .to_string(),
            description: item
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            author: item
                .get("author")
                .and_then(|v| v.as_str())
                .unwrap_or("master3395")
                .to_string(),
            version: item
                .get("version")
                .and_then(|v| v.as_str())
                .unwrap_or("1.0.0")
                .to_string(),
            grant_plugin: item
                .get("grant_plugin")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            purchase_url: item
                .get("purchase_url")
                .and_then(|v| v.as_str())
                .unwrap_or("https://shop.newstargeted.com/shop/catalog?category=cpn")
                .to_string(),
            store_url: item
                .get("store_url")
                .and_then(|v| v.as_str())
                .unwrap_or("https://newstargeted.com/store/products/category/cpn")
                .to_string(),
            tokens,
            background,
        });
    }
    Ok(out)
}

pub fn redeem_theme_activation_key(
    theme_id: &str,
    grant_plugin: &str,
    activation_key: &str,
    license_email: &str,
) -> Result<ThemeLicenseEntry, String> {
    let theme_id = theme_id.trim().to_ascii_lowercase();
    let key = activation_key.trim();
    if theme_id.is_empty() || key.is_empty() {
        return Err("theme_id and activation_key are required".into());
    }
    let payload = serde_json::json!({
        "theme_id": theme_id,
        "plugin_name": grant_plugin,
        "activation_key": key,
        "user_email": license_email.trim().to_ascii_lowercase(),
    });
    let url = format!("{API_BASE}{VERIFY_PATH}");
    let resp = curl_json_post(&url, &payload.to_string())?;
    let entitled = resp
        .get("has_access")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if !entitled {
        let msg = resp
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("Activation key was rejected");
        return Err(msg.to_string());
    }
    let entry = ThemeLicenseEntry {
        theme_id: theme_id.clone(),
        grant_plugin: grant_plugin.trim().to_string(),
        activation_key: key.to_string(),
        entitled: true,
        verified_at_unix: now_unix(),
        via: resp
            .get("via")
            .and_then(|v| v.as_str())
            .unwrap_or("activation_key")
            .to_string(),
    };
    let mut file = load_theme_licenses();
    if !license_email.trim().is_empty() {
        file.license_email = license_email.trim().to_ascii_lowercase();
    }
    file.schema_version = 1;
    file.entries
        .retain(|e| !e.theme_id.eq_ignore_ascii_case(&theme_id));
    file.entries.push(entry.clone());
    save_theme_licenses(&file)?;
    Ok(entry)
}

pub fn ensure_paid_theme_entitled(theme_id: &str, grant_plugin: &str) -> Result<(), String> {
    if local_theme_entitled(theme_id) {
        return Ok(());
    }
    let licenses = load_theme_licenses();
    let key = licenses
        .entries
        .iter()
        .find(|e| e.theme_id.eq_ignore_ascii_case(theme_id))
        .map(|e| e.activation_key.clone())
        .unwrap_or_default();
    if key.is_empty() && licenses.license_email.is_empty() {
        return Err(
            "This paid theme is locked. Purchase under the CPN shop category or redeem an activation key from Shop Grants."
                .into(),
        );
    }
    if !key.is_empty() {
        redeem_theme_activation_key(theme_id, grant_plugin, &key, &licenses.license_email)?;
        return Ok(());
    }
    let payload = serde_json::json!({
        "theme_id": theme_id,
        "plugin_name": grant_plugin,
        "user_email": licenses.license_email,
    });
    let url = format!("{API_BASE}{VERIFY_PATH}");
    let resp = curl_json_post(&url, &payload.to_string())?;
    let entitled = resp
        .get("has_access")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if !entitled {
        return Err(resp
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("Not entitled for this paid theme")
            .to_string());
    }
    let entry = ThemeLicenseEntry {
        theme_id: theme_id.to_ascii_lowercase(),
        grant_plugin: grant_plugin.to_string(),
        activation_key: String::new(),
        entitled: true,
        verified_at_unix: now_unix(),
        via: "shop_grant".into(),
    };
    let mut file = licenses;
    file.entries
        .retain(|e| !e.theme_id.eq_ignore_ascii_case(theme_id));
    file.entries.push(entry);
    save_theme_licenses(&file)?;
    Ok(())
}

pub fn download_paid_theme_package(theme_id: &str) -> Result<Vec<u8>, String> {
    let licenses = load_theme_licenses();
    let entry = licenses
        .entries
        .iter()
        .find(|e| e.theme_id.eq_ignore_ascii_case(theme_id) && e.entitled)
        .ok_or_else(|| "Not entitled to download this paid theme".to_string())?;
    let payload = serde_json::json!({
        "theme_id": theme_id,
        "activation_key": entry.activation_key,
        "user_email": licenses.license_email,
    });
    let url = format!("{API_BASE}{PACKAGE_PATH}");
    curl_bytes_post(&url, &payload.to_string())
}

pub fn shop_purchase_url() -> &'static str {
    "https://shop.newstargeted.com/shop/catalog?category=cpn"
}

pub fn store_category_url() -> &'static str {
    "https://newstargeted.com/store/products/category/cpn"
}

pub fn shop_grants_admin_url() -> &'static str {
    "https://api.newstargeted.com/admin/shop-grants"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;

    #[test]
    fn paid_pricing_helper() {
        assert!(theme_is_paid("paid"));
        assert!(theme_is_paid("Premium"));
        assert!(!theme_is_paid("free"));
    }

    #[test]
    fn local_entitlement_roundtrip() {
        with_test_data_dir(|| {
            assert!(!local_theme_entitled("obsidian-pro"));
            let mut file = ThemeLicenseFile {
                schema_version: 1,
                license_email: "ops@example.com".into(),
                entries: vec![ThemeLicenseEntry {
                    theme_id: "obsidian-pro".into(),
                    grant_plugin: "cpn-theme-obsidian-pro".into(),
                    activation_key: "TEST-KEY".into(),
                    entitled: true,
                    verified_at_unix: 1,
                    via: "test".into(),
                }],
            };
            save_theme_licenses(&file).unwrap();
            assert!(local_theme_entitled("obsidian-pro"));
            file.entries.clear();
            save_theme_licenses(&file).unwrap();
            assert!(!local_theme_entitled("obsidian-pro"));
        });
    }
}
