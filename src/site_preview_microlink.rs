//! Remote Site preview screenshots via the Microlink screenshot API.
//!
//! Local headless Chromium capture stays primary (see `site_preview_capture`).
//! This module is the fallback for two cases:
//!
//! 1. The panel host has no browser binary, or every local candidate URL fails.
//!    `capture_via_microlink` then fetches the shot server-side and stores it in
//!    the same authenticated cache under `$CPN_DATA_DIR/site-previews/`.
//! 2. No cached shot exists yet. The Websites list points the thumbnail at the
//!    public image URL so the operator browser renders a real screenshot while
//!    the local capture is still missing.
//!
//! Only publicly resolvable domains are sent. Lab-only hosts (loopback, private
//! IPs, reserved suffixes such as `.local` or `.test`) never leave the host, and
//! operators can switch the whole remote path off in Websites preferences.

use crate::panel_prefs::load_panel_ui_prefs;
use crate::site_preview_thumb::{PREVIEW_MAX_BYTES, image_path, write_cached_image_typed};
use crate::sites::normalize_domain;
use crate::website_preview::is_blocked_preview_host;
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// Public screenshot endpoint (no API key needed for basic usage).
pub const MICROLINK_ENDPOINT: &str = "https://api.microlink.io/";

/// Cache hint sent to the API: 24h, matching the local preview TTL.
pub const MICROLINK_TTL_MS: u64 = 86_400_000;

/// Backend label stored in preview meta for remote shots.
pub const MICROLINK_BACKEND: &str = "microlink";

const FETCH_TIMEOUT_SECS: u64 = 30;

/// Suffixes that never resolve on the public internet.
const PRIVATE_SUFFIXES: &[&str] = &[
    ".local",
    ".localhost",
    ".localdomain",
    ".test",
    ".example",
    ".invalid",
    ".internal",
    ".intranet",
    ".lan",
    ".home",
    ".home.arpa",
    ".corp",
    ".private",
    ".vbox",
];

fn percent_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len() * 2);
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// True when the domain can plausibly be reached by an external screenshot service.
pub fn microlink_supported(domain_raw: &str) -> bool {
    let Ok(domain) = normalize_domain(domain_raw) else {
        return false;
    };
    if is_blocked_preview_host(&domain) {
        return false;
    }
    if PRIVATE_SUFFIXES
        .iter()
        .any(|suffix| domain.ends_with(suffix))
    {
        return false;
    }
    let Some(tld) = domain.rsplit('.').next() else {
        return false;
    };
    tld.len() >= 2 && tld.chars().all(|ch| ch.is_ascii_alphabetic())
}

/// True when remote previews are enabled for the panel and allowed for this domain.
pub fn remote_preview_ready(domain_raw: &str) -> bool {
    load_panel_ui_prefs().remote_site_previews && microlink_supported(domain_raw)
}

/// Public image URL for a domain, usable directly as an `<img src>`.
///
/// `bust` busts the browser cache after a Refresh; `force` asks the API for a
/// new shot instead of its cached copy.
pub fn microlink_image_url(domain_raw: &str, bust: u64, force: bool) -> Option<String> {
    let domain = normalize_domain(domain_raw).ok()?;
    if !microlink_supported(&domain) {
        return None;
    }
    let target = percent_encode(&format!("https://{domain}"));
    let mut url = format!(
        "{MICROLINK_ENDPOINT}?url={target}&screenshot=true&meta=false&embed=screenshot.url&ttl={MICROLINK_TTL_MS}"
    );
    if force {
        url.push_str("&force=true");
    }
    if bust > 0 {
        url.push_str(&format!("&t={bust}"));
    }
    Some(url)
}

/// Same URL, but only when the operator left remote previews enabled.
pub fn enabled_image_url(domain_raw: &str, bust: u64) -> Option<String> {
    if !load_panel_ui_prefs().remote_site_previews {
        return None;
    }
    microlink_image_url(domain_raw, bust, false)
}

fn sniff_image_type(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some("image/png");
    }
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some("image/jpeg");
    }
    if bytes.len() > 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        return Some("image/webp");
    }
    None
}

/// Fetch a screenshot for the domain. Returns image bytes and content type.
pub fn fetch_microlink_screenshot(
    domain_raw: &str,
    force: bool,
) -> Result<(Vec<u8>, String), String> {
    let domain = normalize_domain(domain_raw)?;
    if !load_panel_ui_prefs().remote_site_previews {
        return Err("Remote Site preview is disabled in Websites preferences".into());
    }
    let url = microlink_image_url(&domain, 0, force)
        .ok_or_else(|| format!("{domain} is not publicly resolvable for a remote screenshot"))?;

    // No `--fail`: error bodies carry the reason (for example a lab domain that
    // resolves to a private IP), which is nicer than a bare status code.
    let output = Command::new("curl")
        .args([
            "--silent",
            "--show-error",
            "--location",
            "--max-time",
            &FETCH_TIMEOUT_SECS.to_string(),
            "--max-filesize",
            &PREVIEW_MAX_BYTES.to_string(),
            "-A",
            "cpn-installer-site-preview",
            &url,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|err| format!("Could not run curl for remote screenshot: {err}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let detail: String = stderr.trim().chars().take(160).collect();
        let detail = if detail.is_empty() {
            format!("exit status {}", output.status.code().unwrap_or(-1))
        } else {
            detail
        };
        return Err(format!("Remote screenshot request failed ({detail})"));
    }

    let bytes = output.stdout;
    if bytes.len() < 64 {
        return Err("Remote screenshot response was empty".into());
    }
    if bytes.len() as u64 > PREVIEW_MAX_BYTES {
        return Err("Remote screenshot exceeds size cap".into());
    }
    let Some(ctype) = sniff_image_type(&bytes) else {
        return Err(remote_screenshot_error(&bytes));
    };
    Ok((bytes, ctype.to_string()))
}

fn clip(value: &str, max: usize) -> String {
    value.trim().chars().take(max).collect()
}

/// Turn a non-image response into an operator-readable reason.
fn remote_screenshot_error(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
        let reason = value
            .pointer("/data/url")
            .and_then(|field| field.as_str())
            .or_else(|| value.get("message").and_then(|field| field.as_str()));
        if let Some(reason) = reason {
            return format!("screenshot service declined ({})", clip(reason, 200));
        }
    }
    format!(
        "screenshot service returned no image ({})",
        clip(&text, 120)
    )
}

/// Fetch and store a remote screenshot in the authenticated preview cache.
pub fn capture_via_microlink(domain_raw: &str, force: bool) -> Result<PathBuf, String> {
    let domain = normalize_domain(domain_raw)?;
    let (bytes, ctype) = fetch_microlink_screenshot(&domain, force)?;
    write_cached_image_typed(&domain, &bytes, MICROLINK_BACKEND, &ctype)?;
    image_path(&domain)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_domains_are_supported() {
        assert!(microlink_supported("example.com"));
        assert!(microlink_supported("test2.example.com"));
    }

    #[test]
    fn lab_and_private_hosts_are_skipped() {
        assert!(!microlink_supported("panel.local"));
        assert!(!microlink_supported("site.test"));
        assert!(!microlink_supported("box.localhost"));
        assert!(!microlink_supported("127.0.0.1"));
        assert!(!microlink_supported("10.0.2.15"));
        assert!(!microlink_supported("no-dot-host"));
    }

    #[test]
    fn image_url_targets_https_and_embeds_screenshot() {
        let url = microlink_image_url("example.com", 0, false).expect("url");
        assert!(url.starts_with("https://api.microlink.io/?url=https%3A%2F%2Fexample.com"));
        assert!(url.contains("screenshot=true"));
        assert!(url.contains("embed=screenshot.url"));
        assert!(url.contains("meta=false"));
        assert!(!url.contains("force=true"));
        assert!(!url.contains("&t="));
    }

    #[test]
    fn refresh_adds_force_and_bust() {
        let url = microlink_image_url("example.com", 1758900000, true).expect("url");
        assert!(url.contains("force=true"));
        assert!(url.contains("&t=1758900000"));
    }

    #[test]
    fn private_domains_have_no_image_url() {
        assert!(microlink_image_url("lab.test", 0, false).is_none());
    }

    #[test]
    fn json_error_body_becomes_readable_reason() {
        let body = br#"{"status":"fail","data":{"url":"The URL `https://lab.example/` is being resolved into an IP address whose range is not allowed."},"code":"EFORBIDDENURL"}"#;
        let message = remote_screenshot_error(body);
        assert!(message.contains("declined"));
        assert!(message.contains("range is not allowed"));
    }

    #[test]
    fn non_json_error_body_is_clipped() {
        let message = remote_screenshot_error(b"<html>502 Bad Gateway</html>");
        assert!(message.contains("no image"));
        assert!(message.contains("502"));
    }

    #[test]
    fn sniffs_common_screenshot_formats() {
        assert_eq!(
            sniff_image_type(b"\x89PNG\r\n\x1a\nrest"),
            Some("image/png")
        );
        assert_eq!(
            sniff_image_type(&[0xFF, 0xD8, 0xFF, 0x00]),
            Some("image/jpeg")
        );
        assert_eq!(sniff_image_type(b"<html><body>nope"), None);
    }
}
