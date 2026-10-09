//! Catalog icon metadata: `<icon>` in `meta.xml` or a bundled `icon.*` file per plugin folder.
//!
//! Resolved to an `https://` URL the browser can load. Relative names map to the raw
//! GitHub path of the CPN-Plugins catalog so plugin authors only need to drop an
//! `icon.svg` / `icon.png` next to `meta.xml`.

use std::path::Path;

/// Raw file base for the catalog repository (main branch).
pub(crate) const CATALOG_RAW_BASE: &str =
    "https://raw.githubusercontent.com/Control-Panel-Network/CPN-Plugins/main";

/// Bundled icon file names probed inside each plugin folder (first match wins).
pub(crate) const ICON_FILE_CANDIDATES: &[&str] = &[
    "icon.svg",
    "icon.png",
    "icon.webp",
    "logo.svg",
    "logo.png",
    "logo.webp",
];

fn is_safe_relative_name(name: &str) -> bool {
    if name.is_empty() || name.len() > 120 {
        return false;
    }
    if name.contains("..") || name.starts_with('/') || name.starts_with('\\') {
        return false;
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/'))
    {
        return false;
    }
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".svg")
        || lower.ends_with(".png")
        || lower.ends_with(".webp")
        || lower.ends_with(".jpg")
        || lower.ends_with(".jpeg")
        || lower.ends_with(".gif")
}

/// Normalise a raw `<icon>` value to a safe `https://` URL.
///
/// Accepts absolute `https://` URLs (no `http:`, `data:`, or `javascript:`)
/// or a relative image file name inside the plugin folder of the catalog repo.
pub(crate) fn resolve_catalog_icon_url(plugin_id: &str, raw: &str) -> Option<String> {
    let value = raw.trim();
    if value.is_empty() {
        return None;
    }
    let lower = value.to_ascii_lowercase();
    if lower.starts_with("https://") {
        if value.len() > 512
            || value
                .chars()
                .any(|c| c.is_control() || c.is_whitespace() || matches!(c, '"' | '<' | '>'))
        {
            return None;
        }
        return Some(value.to_string());
    }
    if lower.contains(':') {
        // http:, data:, javascript:, file: and friends are refused.
        return None;
    }
    let rel = value.trim_start_matches("./");
    if !is_safe_relative_name(rel) {
        return None;
    }
    let id = plugin_id.trim();
    if id.is_empty()
        || !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
    {
        return None;
    }
    Some(format!("{CATALOG_RAW_BASE}/{id}/{rel}"))
}

/// Detect a bundled icon file inside an extracted plugin folder.
pub(crate) fn detect_bundled_icon(plugin_id: &str, plugin_dir: &Path) -> Option<String> {
    for candidate in ICON_FILE_CANDIDATES {
        let path = plugin_dir.join(candidate);
        if path.is_file() {
            return resolve_catalog_icon_url(plugin_id, candidate);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn https_urls_pass_through() {
        assert_eq!(
            resolve_catalog_icon_url("docker", "https://example.com/i.svg").as_deref(),
            Some("https://example.com/i.svg")
        );
    }

    #[test]
    fn unsafe_schemes_rejected() {
        assert!(resolve_catalog_icon_url("x", "http://example.com/i.png").is_none());
        assert!(resolve_catalog_icon_url("x", "data:image/svg+xml;base64,AAAA").is_none());
        assert!(resolve_catalog_icon_url("x", "javascript:alert(1)").is_none());
        assert!(resolve_catalog_icon_url("x", "https://e.com/a\"onerror=1").is_none());
    }

    #[test]
    fn relative_names_map_to_raw_github() {
        assert_eq!(
            resolve_catalog_icon_url("clamav", "icon.svg").as_deref(),
            Some(
                "https://raw.githubusercontent.com/Control-Panel-Network/CPN-Plugins/main/clamav/icon.svg"
            )
        );
        assert_eq!(
            resolve_catalog_icon_url("clamav", "./static/logo.png").as_deref(),
            Some(
                "https://raw.githubusercontent.com/Control-Panel-Network/CPN-Plugins/main/clamav/static/logo.png"
            )
        );
        assert!(resolve_catalog_icon_url("clamav", "../other/icon.svg").is_none());
        assert!(resolve_catalog_icon_url("clamav", "icon.exe").is_none());
        assert!(resolve_catalog_icon_url("bad id", "icon.svg").is_none());
        assert!(resolve_catalog_icon_url("clamav", "").is_none());
    }

    #[test]
    fn detect_bundled_icon_finds_first_candidate() {
        let dir = std::env::temp_dir().join(format!("cpn-icon-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        assert!(detect_bundled_icon("demo", &dir).is_none());
        std::fs::write(dir.join("icon.png"), b"png").unwrap();
        let url = detect_bundled_icon("demo", &dir).unwrap();
        assert!(url.ends_with("/demo/icon.png"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
