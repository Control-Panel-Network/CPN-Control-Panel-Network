//! CPN plugin catalog fetch/cache and legacy `meta.xml` adapters.
//!
//! Catalog archive: https://github.com/master3395/cyberpanel-plugins

use crate::account::{data_dir, now_unix};
use crate::plugins::{CatalogEntry, catalog_repo_slug, normalize_plugin_id, sanitize_user_text};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

pub(crate) const CATALOG_CACHE_SECS: u64 = 3600;
pub(crate) const CATALOG_TARBALL: &str =
    "https://codeload.github.com/master3395/cyberpanel-plugins/tar.gz/refs/heads/main";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CatalogCache {
    fetched_at_unix: u64,
    entries: Vec<CatalogEntry>,
}

fn catalog_cache_path() -> PathBuf {
    data_dir().join("plugin-catalog-cache.json")
}

fn xml_tag(body: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = body.find(&open)? + open.len();
    let end = body[start..].find(&close)? + start;
    let value = body[start..end].trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn pricing_from_meta(body: &str) -> String {
    if let Some(pricing) = xml_tag(body, "pricing") {
        let lower = pricing.to_ascii_lowercase();
        if lower.contains("paid") || lower.contains("premium") {
            return "paid".into();
        }
        return "free".into();
    }
    if xml_tag(body, "paid")
        .map(|v| matches!(v.to_ascii_lowercase().as_str(), "true" | "1" | "yes"))
        .unwrap_or(false)
    {
        return "paid".into();
    }
    "free".into()
}

/// Parse a legacy catalog `meta.xml` into a CPN catalog entry.
pub fn parse_meta_xml(plugin_id: &str, body: &str) -> Result<CatalogEntry, String> {
    let name = xml_tag(body, "name")
        .or_else(|| xml_tag(body, "Name"))
        .unwrap_or_else(|| plugin_id.to_string());
    let category = xml_tag(body, "type")
        .or_else(|| xml_tag(body, "category"))
        .or_else(|| xml_tag(body, "Type"))
        .unwrap_or_else(|| "Utility".into());
    let version = xml_tag(body, "version")
        .or_else(|| xml_tag(body, "Version"))
        .unwrap_or_else(|| "0.0.0".into());
    let description = xml_tag(body, "description")
        .or_else(|| xml_tag(body, "Description"))
        .unwrap_or_else(|| "No description provided.".into());
    let author = xml_tag(body, "author")
        .or_else(|| xml_tag(body, "Author"))
        .unwrap_or_else(|| "unknown".into());
    Ok(CatalogEntry {
        id: plugin_id.to_string(),
        name: sanitize_user_text(&name),
        category: sanitize_user_text(&category),
        version,
        description: sanitize_user_text(&description),
        author: sanitize_user_text(&author),
        pricing: pricing_from_meta(body),
    })
}

pub(crate) fn curl_bytes(url: &str) -> Result<Vec<u8>, String> {
    let output = Command::new("curl")
        .args([
            "--fail",
            "--silent",
            "--show-error",
            "--location",
            "--max-time",
            "120",
            "-H",
            "User-Agent: cpn-installer",
            url,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| format!("Could not download catalog: {error}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "Catalog download failed ({})",
            stderr.trim().chars().take(160).collect::<String>()
        ));
    }
    Ok(output.stdout)
}

fn extract_catalog_entries(tarball: &Path) -> Result<Vec<CatalogEntry>, String> {
    let tmp = std::env::temp_dir().join(format!("cpn-plugin-cat-{}", std::process::id()));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).map_err(|error| format!("Could not create temp dir: {error}"))?;
    let status = Command::new("tar")
        .args(["-xzf"])
        .arg(tarball)
        .arg("-C")
        .arg(&tmp)
        .status()
        .map_err(|error| format!("Could not extract catalog archive: {error}"))?;
    if !status.success() {
        let _ = fs::remove_dir_all(&tmp);
        return Err("Failed to extract catalog archive".into());
    }
    let mut entries = Vec::new();
    let Ok(roots) = fs::read_dir(&tmp) else {
        let _ = fs::remove_dir_all(&tmp);
        return Err("Catalog archive was empty".into());
    };
    for root in roots.flatten() {
        let root_path = root.path();
        if !root_path.is_dir() {
            continue;
        }
        let Ok(children) = fs::read_dir(&root_path) else {
            continue;
        };
        for child in children.flatten() {
            let plugin_path = child.path();
            if !plugin_path.is_dir() {
                continue;
            }
            let Some(id) = plugin_path.file_name().and_then(|v| v.to_str()) else {
                continue;
            };
            if normalize_plugin_id(id).is_err() {
                continue;
            }
            let meta = plugin_path.join("meta.xml");
            if !meta.is_file() {
                continue;
            }
            if let Ok(body) = fs::read_to_string(&meta)
                && let Ok(entry) = parse_meta_xml(id, &body)
            {
                entries.push(entry);
            }
        }
    }
    let _ = fs::remove_dir_all(&tmp);
    entries.sort_by_key(|a| a.name.to_lowercase());
    Ok(entries)
}

fn write_catalog_cache(entries: &[CatalogEntry]) -> Result<(), String> {
    let cache = CatalogCache {
        fetched_at_unix: now_unix(),
        entries: entries.to_vec(),
    };
    let path = catalog_cache_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Could not create data dir: {error}"))?;
    }
    let raw = serde_json::to_string_pretty(&cache)
        .map_err(|error| format!("Could not serialize catalog cache: {error}"))?;
    fs::write(&path, raw).map_err(|error| format!("Could not write catalog cache: {error}"))?;
    Ok(())
}

fn load_catalog_cache() -> Option<CatalogCache> {
    let raw = fs::read_to_string(catalog_cache_path()).ok()?;
    serde_json::from_str(&raw).ok()
}

fn cache_is_fresh(cache: &CatalogCache) -> bool {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|v| v.as_secs())
        .unwrap_or(0);
    now.saturating_sub(cache.fetched_at_unix) < CATALOG_CACHE_SECS
}

/// Fetch (or return cached) plugin catalog from the community GitHub archive.
pub fn fetch_catalog(force_refresh: bool) -> Result<(Vec<CatalogEntry>, u64), String> {
    if !force_refresh
        && let Some(cache) = load_catalog_cache()
        && cache_is_fresh(&cache)
    {
        return Ok((cache.entries, cache.fetched_at_unix));
    }
    let bytes = curl_bytes(CATALOG_TARBALL)?;
    let tar_path =
        std::env::temp_dir().join(format!("cpn-plugins-catalog-{}.tar.gz", std::process::id()));
    fs::write(&tar_path, &bytes).map_err(|error| format!("Could not write tarball: {error}"))?;
    let entries = extract_catalog_entries(&tar_path);
    let _ = fs::remove_file(&tar_path);
    let entries = entries?;
    if entries.is_empty() {
        return Err("Catalog contained no plugins with meta.xml".into());
    }
    write_catalog_cache(&entries)?;
    Ok((entries, now_unix()))
}

pub fn catalog_next_refresh_unix(fetched_at: u64) -> u64 {
    fetched_at.saturating_add(CATALOG_CACHE_SECS)
}

pub fn format_unix_local(ts: u64) -> String {
    #[cfg(unix)]
    {
        let output = Command::new("date")
            .args(["-d", &format!("@{ts}"), "+%d.%m.%Y %H:%M:%S"])
            .output();
        if let Ok(out) = output
            && out.status.success()
        {
            let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !text.is_empty() {
                return text;
            }
        }
    }
    let _ = catalog_repo_slug();
    format!("{ts} (unix)")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_meta_builds_cpn_entry() {
        let xml = r#"
        <plugin>
          <name>CyberPanel Port Manager</name>
          <type>Utility</type>
          <version>1.0.1</version>
          <description>Ports for CyberPanel hosts</description>
          <author>master3395</author>
          <paid>false</paid>
        </plugin>
        "#;
        let entry = parse_meta_xml("port_manager", xml).unwrap();
        assert_eq!(entry.id, "port_manager");
        assert!(!entry.name.to_ascii_lowercase().contains("cyberpanel"));
        assert!(
            !entry
                .description
                .to_ascii_lowercase()
                .contains("cyberpanel")
        );
        assert_eq!(entry.pricing, "free");
    }
}
