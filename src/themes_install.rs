//! Installed Design themes from Control-Panel-Network/CPN-Themes.
//!
//! Packages land under `/var/lib/cpn/installed-themes/<id>/` (theme.json + manifest).

use crate::account::{data_dir, now_unix};
use crate::panel_theme::{DesignTokens, ThemeBackground, sanitize_theme_extra_css};
use crate::plugins_catalog::curl_bytes;
use crate::themes_catalog::{ThemeCatalogEntry, themes_repo_slug};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

const CATALOG_TARBALL: &str =
    "https://codeload.github.com/Control-Panel-Network/CPN-Themes/tar.gz/refs/heads/main";
const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledThemeManifest {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub installed_at_unix: u64,
    pub source: String,
    pub catalog_repo: String,
    pub tokens: DesignTokens,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<ThemeBackground>,
    #[serde(default)]
    pub has_theme_css: bool,
}

fn installed_root() -> PathBuf {
    data_dir().join("installed-themes")
}

fn theme_dir(id: &str) -> PathBuf {
    installed_root().join(id)
}

fn manifest_path(id: &str) -> PathBuf {
    theme_dir(id).join("manifest.json")
}

fn theme_json_path(id: &str) -> PathBuf {
    theme_dir(id).join("theme.json")
}

fn normalize_theme_id(raw: &str) -> Result<String, String> {
    let id = raw.trim().to_ascii_lowercase();
    if id.is_empty()
        || !id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        return Err("Theme id is invalid".into());
    }
    Ok(id)
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Could not create theme dir: {e}"))?;
    }
    let raw = serde_json::to_string_pretty(value)
        .map_err(|e| format!("Could not serialize theme data: {e}"))?;
    fs::write(path, raw).map_err(|e| format!("Could not write {}: {e}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst).map_err(|e| format!("Could not create {}: {e}", dst.display()))?;
    for entry in fs::read_dir(src).map_err(|e| format!("Could not read {}: {e}", src.display()))? {
        let entry = entry.map_err(|e| format!("Could not read theme entry: {e}"))?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            copy_dir_recursive(&from, &to)?;
        } else {
            fs::copy(&from, &to).map_err(|e| format!("Could not copy {}: {e}", from.display()))?;
        }
    }
    Ok(())
}

fn find_theme_src(extract_root: &Path, theme_id: &str) -> Option<PathBuf> {
    let mut stack = vec![extract_root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let candidate = dir.join(theme_id);
        if candidate.is_dir() && candidate.join("theme.json").is_file() {
            return Some(candidate);
        }
        let themes_nested = dir.join("themes").join(theme_id);
        if themes_nested.is_dir() && themes_nested.join("theme.json").is_file() {
            return Some(themes_nested);
        }
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                }
            }
        }
    }
    None
}

pub fn theme_is_installed(id: &str) -> bool {
    let Ok(id) = normalize_theme_id(id) else {
        return false;
    };
    theme_json_path(&id).is_file()
}

fn theme_css_path(id: &str) -> PathBuf {
    theme_dir(id).join("theme.css")
}

pub fn load_installed_theme_extra_css(id: &str) -> Option<String> {
    let Ok(id) = normalize_theme_id(id) else {
        return None;
    };
    let raw = fs::read_to_string(theme_css_path(&id)).ok()?;
    let cleaned = sanitize_theme_extra_css(&raw);
    if cleaned.trim().is_empty() {
        None
    } else {
        Some(cleaned)
    }
}

pub fn load_installed_theme(id: &str) -> Result<InstalledThemeManifest, String> {
    let id = normalize_theme_id(id)?;
    if let Ok(raw) = fs::read_to_string(manifest_path(&id)) {
        if let Ok(mut manifest) = serde_json::from_str::<InstalledThemeManifest>(&raw) {
            if manifest.background.is_none() {
                if let Ok(theme_raw) = fs::read_to_string(theme_json_path(&id)) {
                    if let Ok(entry) =
                        crate::themes_catalog::parse_theme_json_for_install(&id, &theme_raw)
                    {
                        manifest.background = entry.background;
                    }
                }
            }
            manifest.has_theme_css = theme_css_path(&id).is_file();
            return Ok(manifest);
        }
    }
    let raw = fs::read_to_string(theme_json_path(&id))
        .map_err(|_| format!("Theme `{id}` is not installed"))?;
    let entry = crate::themes_catalog::parse_theme_json_for_install(&id, &raw)?;
    Ok(InstalledThemeManifest {
        schema_version: SCHEMA_VERSION,
        id: entry.id.clone(),
        name: entry.name.clone(),
        version: entry.version.clone(),
        description: entry.description.clone(),
        author: entry.author.clone(),
        installed_at_unix: now_unix(),
        source: "catalog".into(),
        catalog_repo: themes_repo_slug().into(),
        tokens: entry.tokens,
        background: entry.background,
        has_theme_css: theme_css_path(&id).is_file(),
    })
}

pub fn list_installed_themes() -> Vec<InstalledThemeManifest> {
    let root = installed_root();
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(id) = path.file_name().and_then(|v| v.to_str()) else {
            continue;
        };
        if let Ok(manifest) = load_installed_theme(id) {
            out.push(manifest);
        }
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

/// Install (or reinstall/update) a theme package from the CPN-Themes GitHub archive.
pub fn install_theme(theme_id: &str) -> Result<InstalledThemeManifest, String> {
    let id = normalize_theme_id(theme_id)?;
    let bytes = curl_bytes(CATALOG_TARBALL)?;
    let tar_path =
        std::env::temp_dir().join(format!("cpn-theme-install-{}.tar.gz", std::process::id()));
    let extract = std::env::temp_dir().join(format!("cpn-theme-install-{}", std::process::id()));
    let _ = fs::remove_dir_all(&extract);
    fs::create_dir_all(&extract).map_err(|e| format!("Could not create temp dir: {e}"))?;
    fs::write(&tar_path, &bytes).map_err(|e| format!("Could not write tarball: {e}"))?;
    let status = Command::new("tar")
        .args(["-xzf"])
        .arg(&tar_path)
        .arg("-C")
        .arg(&extract)
        .status()
        .map_err(|e| format!("Could not extract theme archive: {e}"))?;
    let _ = fs::remove_file(&tar_path);
    if !status.success() {
        let _ = fs::remove_dir_all(&extract);
        return Err("Failed to extract theme archive".into());
    }
    let Some(src) = find_theme_src(&extract, &id) else {
        let _ = fs::remove_dir_all(&extract);
        return Err(format!(
            "Theme `{id}` was not found in the CPN-Themes catalog archive"
        ));
    };
    let raw = fs::read_to_string(src.join("theme.json"))
        .map_err(|e| format!("Could not read theme.json: {e}"))?;
    let entry = crate::themes_catalog::parse_theme_json_for_install(&id, &raw)?;
    let dest = theme_dir(&id);
    if dest.exists() {
        let _ = fs::remove_dir_all(&dest);
    }
    copy_dir_recursive(&src, &dest)?;
    let _ = fs::remove_dir_all(&extract);
    let has_theme_css = dest.join("theme.css").is_file();
    let manifest = InstalledThemeManifest {
        schema_version: SCHEMA_VERSION,
        id: entry.id.clone(),
        name: entry.name.clone(),
        version: entry.version.clone(),
        description: entry.description.clone(),
        author: entry.author.clone(),
        installed_at_unix: now_unix(),
        source: "catalog".into(),
        catalog_repo: themes_repo_slug().into(),
        tokens: entry.tokens.clone(),
        background: entry.background.clone(),
        has_theme_css,
    };
    write_json(&manifest_path(&id), &manifest)?;
    // Keep a clean theme.json copy for operators inspecting the package.
    let mut theme_json = serde_json::json!({
        "schema_version": 1,
        "id": entry.id,
        "name": entry.name,
        "description": entry.description,
        "author": entry.author,
        "version": entry.version,
        "tokens": entry.tokens,
    });
    if let Some(bg) = entry.background.as_ref() {
        if let Some(obj) = theme_json.as_object_mut() {
            obj.insert(
                "background".into(),
                serde_json::to_value(bg).unwrap_or(serde_json::Value::Null),
            );
        }
    }
    write_json(&theme_json_path(&id), &theme_json)?;
    Ok(manifest)
}

pub fn uninstall_theme(theme_id: &str) -> Result<(), String> {
    let id = normalize_theme_id(theme_id)?;
    let dest = theme_dir(&id);
    if !dest.exists() {
        return Err(format!("Theme `{id}` is not installed"));
    }
    fs::remove_dir_all(&dest).map_err(|e| format!("Could not remove theme `{id}`: {e}"))?;
    Ok(())
}

/// Catalog row enriched with install/active state for Theme Store UI.
#[derive(Debug, Clone, Serialize)]
pub struct ThemeStoreRow {
    #[serde(flatten)]
    pub entry: ThemeCatalogEntry,
    pub installed: bool,
    pub installed_version: Option<String>,
    pub update_available: bool,
    pub active: bool,
    pub status: String,
}

pub fn enrich_catalog_for_store(
    entries: &[ThemeCatalogEntry],
    active_theme_id: Option<&str>,
) -> Vec<ThemeStoreRow> {
    let installed = list_installed_themes();
    entries
        .iter()
        .map(|entry| {
            let local = installed
                .iter()
                .find(|m| m.id.eq_ignore_ascii_case(&entry.id));
            let installed_flag = local.is_some();
            let installed_version = local.map(|m| m.version.clone());
            let update_available = local
                .map(|m| m.version.trim() != entry.version.trim() && !entry.version.is_empty())
                .unwrap_or(false);
            let active = active_theme_id
                .map(|a| a.eq_ignore_ascii_case(&entry.id))
                .unwrap_or(false);
            let status = if active {
                "Active".to_string()
            } else if update_available {
                "Update available".to_string()
            } else if installed_flag {
                "Installed".to_string()
            } else {
                "Available".to_string()
            };
            ThemeStoreRow {
                entry: entry.clone(),
                installed: installed_flag,
                installed_version,
                update_available,
                active,
                status,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;

    #[test]
    fn normalize_rejects_path_traversal() {
        assert!(normalize_theme_id("../etc").is_err());
        assert!(normalize_theme_id("ocean-blue").is_ok());
    }

    #[test]
    fn uninstall_missing_theme_errors() {
        with_test_data_dir(|| {
            let err = uninstall_theme("missing-theme").unwrap_err();
            assert!(err.contains("not installed"));
        });
    }
}
