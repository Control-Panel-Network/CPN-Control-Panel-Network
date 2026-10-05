//! Site preview thumbnails: disk cache under `$CPN_DATA_DIR/site-previews/`.
//!
//! Capture uses headless Chromium when available (see `site_preview_capture`).
//! Served only through authenticated panel routes for registry domains.
//! Successful shots stay Fresh for [`PREVIEW_TTL`] (7 days). List pages serve
//! disk cache or placeholders; remote APIs run only on Refresh preview.

use crate::account::data_dir;
use crate::sites::normalize_domain;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Default freshness window before a successful cached shot is considered stale.
/// Long enough that websites / sub-domains list reloads do not thrash capture
/// backends or remote screenshot quota.
pub const PREVIEW_TTL: Duration = Duration::from_secs(7 * 24 * 60 * 60);

/// Reject oversized screenshot files (bytes).
pub const PREVIEW_MAX_BYTES: u64 = 5 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PreviewMeta {
    #[serde(default)]
    pub captured_at: u64,
    /// Unix seconds when this successful shot expires (captured_at + TTL).
    /// Zero means older cache entries without an explicit expiry; freshness
    /// then falls back to captured_at + [`PREVIEW_TTL`].
    #[serde(default)]
    pub expires_at: u64,
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub error: String,
    #[serde(default)]
    pub backend: String,
    #[serde(default)]
    pub content_type: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewFreshness {
    Fresh,
    Stale,
    Missing,
}

pub fn preview_dir() -> PathBuf {
    data_dir().join("site-previews")
}

pub fn ensure_preview_dir() -> Result<PathBuf, String> {
    let dir = preview_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("Could not create site-previews dir: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&dir, fs::Permissions::from_mode(0o700));
    }
    Ok(dir)
}

fn safe_domain_key(domain_raw: &str) -> Result<String, String> {
    let domain = normalize_domain(domain_raw)?;
    if domain.is_empty() || domain.contains('/') || domain.contains('\\') || domain.contains("..") {
        return Err("Invalid domain for site preview".into());
    }
    Ok(domain)
}

pub fn image_path(domain_raw: &str) -> Result<PathBuf, String> {
    let domain = safe_domain_key(domain_raw)?;
    Ok(preview_dir().join(format!("{domain}.png")))
}

pub fn meta_path(domain_raw: &str) -> Result<PathBuf, String> {
    let domain = safe_domain_key(domain_raw)?;
    Ok(preview_dir().join(format!("{domain}.json")))
}

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn load_meta(domain_raw: &str) -> PreviewMeta {
    let Ok(path) = meta_path(domain_raw) else {
        return PreviewMeta::default();
    };
    let Ok(raw) = fs::read_to_string(path) else {
        return PreviewMeta::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

pub fn save_meta(domain_raw: &str, meta: &PreviewMeta) -> Result<(), String> {
    ensure_preview_dir()?;
    let path = meta_path(domain_raw)?;
    let raw = serde_json::to_string_pretty(meta)
        .map_err(|e| format!("Could not serialize preview meta: {e}"))?;
    fs::write(&path, raw).map_err(|e| format!("Could not write preview meta: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

/// Effective expiry for a meta record (explicit `expires_at`, else captured_at + TTL).
pub fn meta_expires_at(meta: &PreviewMeta) -> u64 {
    if meta.expires_at > 0 {
        meta.expires_at
    } else if meta.captured_at > 0 {
        meta.captured_at.saturating_add(PREVIEW_TTL.as_secs())
    } else {
        0
    }
}

pub fn freshness(domain_raw: &str) -> PreviewFreshness {
    let meta = load_meta(domain_raw);
    let Ok(img) = image_path(domain_raw) else {
        return PreviewFreshness::Missing;
    };
    if !meta.ok || !img.is_file() {
        if meta.captured_at == 0 {
            return PreviewFreshness::Missing;
        }
        // Failed attempt within TTL: treat as stale so UI shows placeholder + refresh.
        // Do not auto-retry remote APIs from list loads; Refresh is explicit.
        if now_unix() < meta_expires_at(&meta) {
            return PreviewFreshness::Stale;
        }
        return PreviewFreshness::Missing;
    }
    if now_unix() < meta_expires_at(&meta) {
        PreviewFreshness::Fresh
    } else {
        PreviewFreshness::Stale
    }
}

/// True when a successful image file is on disk (Fresh or Stale).
/// List pages prefer this over remote capture so reloads stay quota-safe.
pub fn cached_shot_usable(domain_raw: &str) -> bool {
    let meta = load_meta(domain_raw);
    meta.ok
        && image_path(domain_raw)
            .map(|path| path.is_file())
            .unwrap_or(false)
}

pub fn read_cached_image(domain_raw: &str) -> Result<(Vec<u8>, String), String> {
    let path = image_path(domain_raw)?;
    if !path.is_file() {
        return Err("Site preview image not found".into());
    }
    let meta = fs::metadata(&path).map_err(|e| format!("Cannot stat preview: {e}"))?;
    if meta.len() > PREVIEW_MAX_BYTES {
        return Err("Site preview image exceeds size cap".into());
    }
    let bytes = fs::read(&path).map_err(|e| format!("Cannot read preview: {e}"))?;
    if bytes.is_empty() {
        return Err("Site preview image is empty".into());
    }
    let ctype = load_meta(domain_raw).content_type;
    let ctype = if ctype.starts_with("image/") {
        ctype
    } else {
        "image/png".into()
    };
    Ok((bytes, ctype))
}

pub fn write_cached_image(domain_raw: &str, bytes: &[u8], backend: &str) -> Result<(), String> {
    write_cached_image_typed(domain_raw, bytes, backend, "image/png")
}

/// Store a cached shot with an explicit content type (remote shots may be JPEG).
pub fn write_cached_image_typed(
    domain_raw: &str,
    bytes: &[u8],
    backend: &str,
    content_type: &str,
) -> Result<(), String> {
    if bytes.is_empty() {
        return Err("Empty screenshot".into());
    }
    if bytes.len() as u64 > PREVIEW_MAX_BYTES {
        return Err("Screenshot exceeds size cap".into());
    }
    ensure_preview_dir()?;
    let path = image_path(domain_raw)?;
    let tmp = path.with_extension("png.tmp");
    fs::write(&tmp, bytes).map_err(|e| format!("Could not write preview temp: {e}"))?;
    fs::rename(&tmp, &path).map_err(|e| format!("Could not finalize preview: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    let content_type = if content_type.starts_with("image/") {
        content_type.to_string()
    } else {
        "image/png".to_string()
    };
    let captured_at = now_unix();
    save_meta(
        domain_raw,
        &PreviewMeta {
            captured_at,
            expires_at: captured_at.saturating_add(PREVIEW_TTL.as_secs()),
            ok: true,
            error: String::new(),
            backend: backend.to_string(),
            content_type,
        },
    )?;
    Ok(())
}

/// Drop a cached screenshot so Refresh preview always recaptures.
pub fn invalidate_cached_image(domain_raw: &str) {
    if let Ok(path) = image_path(domain_raw) {
        let _ = fs::remove_file(path);
    }
    if let Ok(path) = meta_path(domain_raw) {
        let _ = fs::remove_file(path);
    }
}

pub fn record_capture_failure(domain_raw: &str, error: &str, backend: &str) -> Result<(), String> {
    // Keep prior image if any; mark meta so UI can show the error.
    // Short failure cooldown (1h) so a bad remote attempt does not block Refresh
    // for the full success TTL, but list loads still avoid tight retry loops.
    let captured_at = now_unix();
    let fail_ttl = 60 * 60;
    save_meta(
        domain_raw,
        &PreviewMeta {
            captured_at,
            expires_at: captured_at.saturating_add(fail_ttl),
            ok: false,
            error: error.chars().take(400).collect(),
            backend: backend.to_string(),
            content_type: String::new(),
        },
    )
}

/// Tiny SVG placeholder when no screenshot is available.
pub fn placeholder_svg(domain: &str, detail: &str) -> String {
    let d = xml_escape(domain);
    let m = xml_escape(detail);
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="640" height="400" viewBox="0 0 640 400" role="img" aria-label="Site preview unavailable">
  <defs>
    <linearGradient id="g" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0%" stop-color="#1a1d26"/>
      <stop offset="100%" stop-color="#2a3140"/>
    </linearGradient>
  </defs>
  <rect width="640" height="400" fill="url(#g)"/>
  <rect x="24" y="24" width="592" height="352" rx="12" fill="none" stroke="#3b82f6" stroke-opacity=".35" stroke-width="2" stroke-dasharray="8 6"/>
  <text x="320" y="175" text-anchor="middle" fill="#f2f4f7" font-family="Segoe UI, system-ui, sans-serif" font-size="22" font-weight="700">Site preview</text>
  <text x="320" y="210" text-anchor="middle" fill="#98a2b3" font-family="Segoe UI, system-ui, sans-serif" font-size="14">{d}</text>
  <text x="320" y="245" text-anchor="middle" fill="#667085" font-family="Segoe UI, system-ui, sans-serif" font-size="12">{m}</text>
</svg>"##
    )
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn image_mtime_secs(path: &Path) -> Option<u64> {
    path.metadata()
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_path_tricks_in_domain() {
        assert!(image_path("../etc/passwd").is_err());
        assert!(image_path("evil/../../x").is_err());
    }

    #[test]
    fn cache_roundtrip_under_data_dir() {
        crate::account::with_test_data_dir(|| {
            let domain = "preview-lab.example";
            write_cached_image(domain, b"\x89PNG\r\n\x1a\nfake", "test").unwrap();
            assert_eq!(freshness(domain), PreviewFreshness::Fresh);
            assert!(cached_shot_usable(domain));
            let meta = load_meta(domain);
            assert!(meta.expires_at > meta.captured_at);
            assert_eq!(
                meta.expires_at,
                meta.captured_at.saturating_add(PREVIEW_TTL.as_secs())
            );
            let (bytes, ctype) = read_cached_image(domain).unwrap();
            assert_eq!(bytes, b"\x89PNG\r\n\x1a\nfake");
            assert_eq!(ctype, "image/png");
            record_capture_failure(domain, "boom", "test").unwrap();
            let meta = load_meta(domain);
            assert!(!meta.ok);
            assert!(meta.error.contains("boom"));
            assert!(meta.expires_at > meta.captured_at);
        });
    }

    #[test]
    fn preview_ttl_is_seven_days() {
        assert_eq!(PREVIEW_TTL.as_secs(), 7 * 24 * 60 * 60);
    }

    #[test]
    fn stale_ok_image_still_usable() {
        crate::account::with_test_data_dir(|| {
            let domain = "stale-ok.example";
            write_cached_image(domain, b"\x89PNG\r\n\x1a\nstale", "test").unwrap();
            let mut meta = load_meta(domain);
            meta.captured_at = now_unix().saturating_sub(PREVIEW_TTL.as_secs() + 10);
            meta.expires_at = meta.captured_at.saturating_add(PREVIEW_TTL.as_secs());
            save_meta(domain, &meta).unwrap();
            assert_eq!(freshness(domain), PreviewFreshness::Stale);
            assert!(cached_shot_usable(domain));
        });
    }

    #[test]
    fn placeholder_mentions_site_preview() {
        let svg = placeholder_svg("example.com", "Refresh to capture");
        assert!(svg.contains("Site preview"));
        assert!(svg.contains("example.com"));
        assert!(svg.contains("Refresh to capture"));
    }
}
