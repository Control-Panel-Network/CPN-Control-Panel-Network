//! Remote Site preview screenshots via the Microlink screenshot API.
//!
//! Local headless Chromium capture stays primary (see `site_preview_capture`).
//! Microlink is a last-resort fallback only when no browser binary exists, and
//! only on explicit Refresh preview (list pages never N+1 this API).
//!
//! Successful remote shots are written under `/var/lib/cpn/site-previews/` with
//! the same long local TTL as Chromium captures so repeats do not re-hit the API.
//!
//! Only publicly resolvable domains are sent. Lab-only hosts never leave the
//! host. Operators can switch the remote path off in Websites preferences.

use crate::panel_prefs::load_panel_ui_prefs;
use crate::site_preview_thumb::{
    PREVIEW_MAX_BYTES, PREVIEW_TTL, PreviewFreshness, cached_shot_usable, freshness, image_path,
    write_cached_image_typed,
};
use crate::sites::normalize_domain;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// Public screenshot endpoint (no API key needed for basic usage).
pub const MICROLINK_ENDPOINT: &str = "https://api.microlink.io/";

/// Cache hint sent to the API: match local preview TTL (7 days).
pub const MICROLINK_TTL_MS: u64 = PREVIEW_TTL.as_secs() * 1000;

/// Backend label stored in preview meta for remote shots.
pub const MICROLINK_BACKEND: &str = "microlink";

const FETCH_TIMEOUT_SECS: u64 = 30;

/// Minimum gap between Microlink HTTP calls host-wide (debounce / rate limit).
const MICROLINK_MIN_GAP: Duration = Duration::from_millis(750);

fn microlink_gate() -> &'static Mutex<Option<Instant>> {
    static GATE: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();
    GATE.get_or_init(|| Mutex::new(None))
}

/// Wait so concurrent Refresh / fallback captures do not stampede the API.
fn wait_microlink_slot() {
    loop {
        let sleep_for = {
            let Ok(mut guard) = microlink_gate().lock() else {
                return;
            };
            let now = Instant::now();
            if let Some(last) = *guard {
                let elapsed = now.saturating_duration_since(last);
                if elapsed < MICROLINK_MIN_GAP {
                    Some(MICROLINK_MIN_GAP - elapsed)
                } else {
                    *guard = Some(now);
                    None
                }
            } else {
                *guard = Some(now);
                None
            }
        };
        match sleep_for {
            Some(wait) => std::thread::sleep(wait),
            None => return,
        }
    }
}

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
    crate::website_preview_stub::is_public_internet_host(&domain)
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

    wait_microlink_slot();

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

/// Operator-facing remote screenshot errors (no vendor upsell / PRO-plan copy).
pub fn operator_remote_error(raw: &str) -> String {
    let lower = raw.to_ascii_lowercase();
    if lower.contains("rate limit")
        || lower.contains("pro plan")
        || lower.contains("quota")
        || lower.contains("upgrade to")
    {
        return "Remote screenshot quota is exhausted. Install a local headless browser and use Refresh preview.".into();
    }
    let clipped: String = raw
        .replace("Upgrade to a PRO plan.", "")
        .replace("Upgrade to a PRO plan", "")
        .chars()
        .take(180)
        .collect();
    let clipped = clipped.trim();
    if clipped.is_empty() {
        "Remote screenshot is unavailable.".into()
    } else {
        clipped.to_string()
    }
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
            return operator_remote_error(&format!("screenshot declined: {reason}"));
        }
    }
    format!(
        "screenshot service returned no image ({})",
        clip(&text, 120)
    )
}

/// Fetch and store a remote screenshot in the authenticated preview cache.
///
/// When `force` is false and a Fresh disk cache already exists, returns that
/// path without calling the remote API.
pub fn capture_via_microlink(domain_raw: &str, force: bool) -> Result<PathBuf, String> {
    let domain = normalize_domain(domain_raw)?;
    if !force && cached_shot_usable(&domain) && freshness(&domain) == PreviewFreshness::Fresh {
        return image_path(&domain);
    }
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
        assert!(url.contains(&format!("ttl={MICROLINK_TTL_MS}")));
        assert_eq!(MICROLINK_TTL_MS, PREVIEW_TTL.as_secs() * 1000);
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
        assert!(message.contains("declined") || message.contains("range is not allowed"));
        assert!(message.contains("range is not allowed"));
        assert!(!message.to_lowercase().contains("pro plan"));
    }

    #[test]
    fn rate_limit_upsell_is_rewritten() {
        let body = br#"{"status":"fail","message":"Your daily rate limit has been reached. Upgrade to a PRO plan."}"#;
        let message = remote_screenshot_error(body);
        assert!(!message.to_lowercase().contains("pro plan"));
        assert!(message.to_lowercase().contains("quota") || message.contains("headless"));
    }

    #[test]
    fn non_json_error_body_is_clipped() {
        let message = remote_screenshot_error(b"<html>502 Bad Gateway</html>");
        assert!(message.contains("no image"));
        assert!(message.contains("502"));
    }

    #[test]
    fn capture_reuses_fresh_disk_cache_without_force() {
        crate::account::with_test_data_dir(|| {
            write_cached_image_typed(
                "cached.example.com",
                b"\x89PNG\r\n\x1a\ncached-remote",
                MICROLINK_BACKEND,
                "image/png",
            )
            .unwrap();
            let path = capture_via_microlink("cached.example.com", false).expect("cached");
            assert!(path.is_file());
            assert_eq!(freshness("cached.example.com"), PreviewFreshness::Fresh);
        });
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
