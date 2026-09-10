//! CPN Design themes catalog fetch/cache.
//!
//! Catalog archive: https://github.com/Control-Panel-Network/CPN-Themes

use crate::account::{data_dir, now_unix};
use crate::panel_theme::DesignTokens;
use crate::plugins_catalog::curl_bytes;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

const CACHE_SECS: u64 = 3600;
const CATALOG_REPO: &str = "Control-Panel-Network/CPN-Themes";
const CATALOG_TARBALL: &str =
    "https://codeload.github.com/Control-Panel-Network/CPN-Themes/tar.gz/refs/heads/main";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeCatalogEntry {
    pub id: String,
    pub name: String,
    pub description: String,
    pub author: String,
    pub version: String,
    pub tokens: DesignTokens,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ThemesCache {
    fetched_at_unix: u64,
    entries: Vec<ThemeCatalogEntry>,
}

pub fn themes_repo_url() -> &'static str {
    "https://github.com/Control-Panel-Network/CPN-Themes"
}

pub fn themes_repo_slug() -> &'static str {
    CATALOG_REPO
}

fn cache_path() -> PathBuf {
    data_dir().join("theme-catalog-cache.json")
}

fn cache_is_fresh(cache: &ThemesCache) -> bool {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|v| v.as_secs())
        .unwrap_or(0);
    now.saturating_sub(cache.fetched_at_unix) < CACHE_SECS
}

fn load_cache() -> Option<ThemesCache> {
    let raw = fs::read_to_string(cache_path()).ok()?;
    serde_json::from_str(&raw).ok()
}

fn write_cache(entries: &[ThemeCatalogEntry]) -> Result<(), String> {
    let cache = ThemesCache {
        fetched_at_unix: now_unix(),
        entries: entries.to_vec(),
    };
    let path = cache_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Could not create data dir: {error}"))?;
    }
    let raw = serde_json::to_string_pretty(&cache)
        .map_err(|error| format!("Could not serialize theme cache: {error}"))?;
    fs::write(&path, raw).map_err(|error| format!("Could not write theme cache: {error}"))?;
    Ok(())
}

#[derive(Debug, Deserialize)]
struct ThemeJsonFile {
    #[serde(default)]
    id: String,
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    author: String,
    #[serde(default)]
    version: String,
    tokens: DesignTokens,
}

fn parse_theme_file(fallback_id: &str, body: &str) -> Result<ThemeCatalogEntry, String> {
    let parsed: ThemeJsonFile =
        serde_json::from_str(body).map_err(|error| format!("Invalid theme.json: {error}"))?;
    let id = if parsed.id.trim().is_empty() {
        fallback_id.to_string()
    } else {
        parsed.id.trim().to_string()
    };
    if id.is_empty()
        || !id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        return Err("Theme id is invalid".into());
    }
    let tokens = parsed.tokens.validate()?;
    Ok(ThemeCatalogEntry {
        id,
        name: parsed.name.trim().to_string(),
        description: if parsed.description.trim().is_empty() {
            "No description provided.".into()
        } else {
            parsed.description.trim().to_string()
        },
        author: if parsed.author.trim().is_empty() {
            "Control Panel Network".into()
        } else {
            parsed.author.trim().to_string()
        },
        version: if parsed.version.trim().is_empty() {
            "1.0.0".into()
        } else {
            parsed.version.trim().to_string()
        },
        tokens,
    })
}

fn walk_themes(dir: &Path, out: &mut Vec<ThemeCatalogEntry>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let theme_file = path.join("theme.json");
        if theme_file.is_file() {
            let Some(id) = path.file_name().and_then(|v| v.to_str()) else {
                continue;
            };
            if let Ok(raw) = fs::read(&theme_file) {
                let body = if raw.starts_with(&[0xEF, 0xBB, 0xBF]) {
                    String::from_utf8_lossy(&raw[3..]).into_owned()
                } else {
                    String::from_utf8_lossy(&raw).into_owned()
                };
                if let Ok(theme) = parse_theme_file(id, &body) {
                    out.push(theme);
                    continue;
                }
            }
        }
        // Recurse into nested folders (e.g. themes/<id>/).
        walk_themes(&path, out);
    }
}

fn extract_theme_entries(tarball: &Path) -> Result<Vec<ThemeCatalogEntry>, String> {
    let tmp = std::env::temp_dir().join(format!("cpn-theme-cat-{}", std::process::id()));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).map_err(|error| format!("Could not create temp dir: {error}"))?;
    let status = Command::new("tar")
        .args(["-xzf"])
        .arg(tarball)
        .arg("-C")
        .arg(&tmp)
        .status()
        .map_err(|error| format!("Could not extract themes archive: {error}"))?;
    if !status.success() {
        let _ = fs::remove_dir_all(&tmp);
        return Err("Failed to extract themes archive".into());
    }
    let mut entries = Vec::new();
    let Ok(roots) = fs::read_dir(&tmp) else {
        let _ = fs::remove_dir_all(&tmp);
        return Err("Themes archive was empty".into());
    };
    for root in roots.flatten() {
        let root_path = root.path();
        if root_path.is_dir() {
            walk_themes(&root_path, &mut entries);
        }
    }
    let _ = fs::remove_dir_all(&tmp);
    entries.sort_by_key(|a| a.name.to_lowercase());
    Ok(entries)
}

/// Fetch (or return cached) design themes from the CPN-Themes GitHub archive.
pub fn fetch_themes_catalog(force_refresh: bool) -> Result<(Vec<ThemeCatalogEntry>, u64), String> {
    if !force_refresh
        && let Some(cache) = load_cache()
        && cache_is_fresh(&cache)
    {
        return Ok((cache.entries, cache.fetched_at_unix));
    }
    let bytes = curl_bytes(CATALOG_TARBALL)?;
    let tar_path =
        std::env::temp_dir().join(format!("cpn-themes-catalog-{}.tar.gz", std::process::id()));
    fs::write(&tar_path, &bytes).map_err(|error| format!("Could not write tarball: {error}"))?;
    let entries = extract_theme_entries(&tar_path);
    let _ = fs::remove_file(&tar_path);
    let entries = entries?;
    if entries.is_empty() {
        return Err("Themes catalog contained no theme.json packages".into());
    }
    write_cache(&entries)?;
    Ok((entries, now_unix()))
}

pub fn find_theme(id: &str, force_refresh: bool) -> Result<ThemeCatalogEntry, String> {
    let want = id.trim();
    let (entries, _) = fetch_themes_catalog(force_refresh)?;
    entries
        .into_iter()
        .find(|t| t.id.eq_ignore_ascii_case(want))
        .ok_or_else(|| format!("Theme `{want}` was not found in the CPN-Themes catalog"))
}

pub fn themes_next_refresh_unix(fetched_at: u64) -> u64 {
    fetched_at.saturating_add(CACHE_SECS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_theme_json_validates_tokens() {
        let body = r##"{
          "id": "ocean-blue",
          "name": "Ocean Blue",
          "description": "Blue accents",
          "author": "Control Panel Network",
          "version": "1.0.0",
          "tokens": {
            "accent": "#2563eb",
            "accent_focus": "#1d4ed8",
            "radius_px": 12,
            "density": "comfortable",
            "font_scale": 1.0
          }
        }"##;
        let theme = parse_theme_file("ocean-blue", body).unwrap();
        assert_eq!(theme.id, "ocean-blue");
        assert_eq!(theme.tokens.accent, "#2563eb");
    }
}
