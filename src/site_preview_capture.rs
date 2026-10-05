//! Capture site homepage screenshots via headless Chromium (or wkhtmltoimage).
//!
//! Lab hosts often lack public DNS. Capture maps the managed domain to loopback
//! (and optional LAN IPs) with Chromium `--host-resolver-rules` so local vhosts
//! still render when the panel can reach them.
//!
//! Local capture stays primary. Refresh always recaptures. Microlink is used
//! only when no browser binary exists and only on explicit Refresh (list pages
//! never N+1 remote APIs). Rate-limit / upsell copy is never shown.

use crate::site_preview_microlink::{
    capture_via_microlink, operator_remote_error, remote_preview_ready,
};
use crate::site_preview_thumb::{
    PREVIEW_MAX_BYTES, image_path, invalidate_cached_image, record_capture_failure,
    write_cached_image,
};
use crate::sites::{load_site, normalize_domain};
use crate::website_preview::ssl_material_present;
use crate::website_preview_live::live_public_origin;
use crate::website_preview_stub::{is_public_internet_host, preview_should_use_live_origin};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

const CAPTURE_TIMEOUT: Duration = Duration::from_secs(25);

/// Run a capture for a registry domain. Overwrites cache on success.
/// May fall back to Microlink when no local browser exists (Refresh path).
pub fn capture_site_preview(domain_raw: &str) -> Result<PathBuf, String> {
    capture_site_preview_inner(domain_raw, true)
}

/// Local Chromium / wkhtmltoimage only. Used by list-page background fills so
/// websites / sub-domains loads never spend remote screenshot quota.
pub fn capture_site_preview_local(domain_raw: &str) -> Result<PathBuf, String> {
    capture_site_preview_inner(domain_raw, false)
}

fn capture_site_preview_inner(domain_raw: &str, allow_remote: bool) -> Result<PathBuf, String> {
    let domain = normalize_domain(domain_raw)?;
    invalidate_cached_image(&domain);
    let live_origin = uses_live_origin(&domain);
    let backends = discover_backends();
    if backends.is_empty() {
        let local_err = "No headless browser found. Install Chromium (System Repair: Site preview browser) and try Refresh preview again.";
        if allow_remote {
            return finish_without_browser(&domain, local_err);
        }
        let _ = record_capture_failure(&domain, local_err, "none");
        return Err(local_err.to_string());
    }

    let urls = candidate_urls(&domain, live_origin);
    let mut last_err = String::from("Capture failed");
    for backend in &backends {
        for url in &urls {
            match run_capture(backend, &domain, url, live_origin) {
                Ok(path) => return Ok(path),
                Err(err) => {
                    last_err = format!("{} via {}: {err}", backend.label, url);
                }
            }
        }
    }
    let _ = record_capture_failure(&domain, &last_err, "failed");
    Err(last_err)
}

fn uses_live_origin(domain: &str) -> bool {
    if !is_public_internet_host(domain) {
        return false;
    }
    match load_site(domain) {
        Ok(site) => preview_should_use_live_origin(std::path::Path::new(&site.docroot), domain),
        Err(_) => true,
    }
}

/// When no local browser exists, try remote once. Never surface vendor upsell copy.
fn finish_without_browser(domain: &str, local_err: &str) -> Result<PathBuf, String> {
    if remote_preview_ready(domain) {
        match capture_via_microlink(domain, true) {
            Ok(path) => return Ok(path),
            Err(remote_err) => {
                let remote = operator_remote_error(&remote_err);
                let combined = format!("{local_err} {remote}");
                let _ = record_capture_failure(domain, &combined, "none");
                return Err(combined);
            }
        }
    }
    let _ = record_capture_failure(domain, local_err, "none");
    Err(local_err.to_string())
}

struct CaptureBackend {
    label: &'static str,
    kind: BackendKind,
    bin: PathBuf,
}

enum BackendKind {
    Chromium,
    WkHtml,
}

fn discover_backends() -> Vec<CaptureBackend> {
    let mut out = Vec::new();
    // Prefer PATH /usr/bin names, then AlmaLinux `chromium-headless` layout
    // (`/usr/lib64/chromium-browser/headless_shell`) which has no short name by default.
    let chromium_candidates = [
        "chromium-browser",
        "chromium",
        "google-chrome",
        "google-chrome-stable",
        "chrome",
        "chromium-headless-shell",
        "headless_shell",
    ];
    for name in chromium_candidates {
        if let Some(bin) = find_bin(name) {
            out.push(CaptureBackend {
                label: "chromium",
                kind: BackendKind::Chromium,
                bin,
            });
            break;
        }
    }
    if out.is_empty() {
        for path in almalinux_headless_paths() {
            let p = PathBuf::from(path);
            if p.is_file() {
                out.push(CaptureBackend {
                    label: "chromium",
                    kind: BackendKind::Chromium,
                    bin: p,
                });
                break;
            }
        }
    }
    if let Some(bin) = find_bin("wkhtmltoimage") {
        out.push(CaptureBackend {
            label: "wkhtmltoimage",
            kind: BackendKind::WkHtml,
            bin,
        });
    }
    out
}

fn almalinux_headless_paths() -> &'static [&'static str] {
    &[
        "/usr/lib64/chromium-browser/headless_shell",
        "/usr/lib/chromium-browser/headless_shell",
        "/usr/lib64/chromium-browser/chromium-headless-shell",
        "/usr/lib64/chromium-headless/headless_shell",
        "/usr/lib/chromium-headless/headless_shell",
        "/usr/lib64/chromium-headless-shell/headless_shell",
        "/opt/google/chrome/chrome",
        "/opt/google/chrome/google-chrome",
        "/usr/bin/google-chrome-stable",
        "/usr/bin/chromium-headless-shell",
    ]
}

/// First discovered browser binary, if any.
pub fn discovered_browser_path() -> Option<PathBuf> {
    discover_backends()
        .into_iter()
        .find(|b| matches!(b.kind, BackendKind::Chromium))
        .map(|b| b.bin)
}

fn find_bin(name: &str) -> Option<PathBuf> {
    // Absolute common paths first (AlmaLinux / container installs).
    let absolutes = [
        format!("/usr/bin/{name}"),
        format!("/usr/local/bin/{name}"),
        format!("/opt/google/chrome/{name}"),
        format!("/usr/lib64/chromium-browser/{name}"),
        format!("/usr/lib/chromium-browser/{name}"),
    ];
    for path in absolutes {
        let p = PathBuf::from(&path);
        if p.is_file() {
            return Some(p);
        }
    }
    which_cmd(name)
}

fn which_cmd(name: &str) -> Option<PathBuf> {
    let output = Command::new("which")
        .arg(name)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let line = String::from_utf8_lossy(&output.stdout);
    let path = line.lines().next()?.trim();
    if path.is_empty() {
        return None;
    }
    let p = PathBuf::from(path);
    if p.is_file() { Some(p) } else { None }
}

fn candidate_urls(domain: &str, live_origin: bool) -> Vec<String> {
    let mut urls = Vec::new();
    if live_origin {
        if let Ok(origin) = live_public_origin(domain) {
            urls.push(format!("{}/", origin.trim_end_matches('/')));
        }
        urls.push(format!("https://{domain}/"));
        urls.push(format!("http://{domain}/"));
        urls.sort();
        urls.dedup();
        urls.sort_by(|a, b| {
            let a_https = a.starts_with("https://") as i8;
            let b_https = b.starts_with("https://") as i8;
            b_https.cmp(&a_https)
        });
        return urls;
    }
    urls.push(format!("http://{domain}/"));
    if ssl_material_present(domain) {
        urls.push(format!("https://{domain}/"));
    }
    urls.sort();
    urls.dedup();
    if !ssl_material_present(domain) {
        urls.sort_by(|a, b| {
            let a_http = a.starts_with("http://") as i8;
            let b_http = b.starts_with("http://") as i8;
            b_http.cmp(&a_http)
        });
    }
    urls
}

fn host_resolver_rules(domain: &str) -> String {
    let mut maps = vec![
        format!("MAP {domain} 127.0.0.1"),
        format!("MAP www.{domain} 127.0.0.1"),
    ];
    for ip in local_listen_hints() {
        maps.push(format!("MAP {domain} {ip}"));
    }
    // First MAP wins in Chromium; keep 127.0.0.1 first for typical OLS/Apache labs.
    maps.join(", ")
}

fn local_listen_hints() -> Vec<String> {
    // Optional: primary IPv4 from `hostname -I` (best-effort, ignore failures).
    let output = Command::new("hostname")
        .arg("-I")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output();
    let Ok(output) = output else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(&output.stdout);
    text.split_whitespace()
        .filter(|ip| {
            let parts: Vec<_> = ip.split('.').collect();
            parts.len() == 4 && parts.iter().all(|p| p.parse::<u8>().is_ok()) && *ip != "127.0.0.1"
        })
        .take(2)
        .map(str::to_string)
        .collect()
}

fn run_capture(
    backend: &CaptureBackend,
    domain: &str,
    url: &str,
    live_origin: bool,
) -> Result<PathBuf, String> {
    let dir = crate::site_preview_thumb::ensure_preview_dir()?;
    let out = dir.join(format!("{domain}.capture.png"));
    let _ = fs::remove_file(&out);

    match backend.kind {
        BackendKind::Chromium => run_chromium(&backend.bin, domain, url, &out, live_origin)?,
        BackendKind::WkHtml => run_wkhtml(&backend.bin, url, &out)?,
    }

    let bytes = fs::read(&out).map_err(|e| format!("Cannot read capture output: {e}"))?;
    let _ = fs::remove_file(&out);
    if bytes.len() < 64 {
        return Err("Capture output too small".into());
    }
    if bytes.len() as u64 > PREVIEW_MAX_BYTES {
        return Err("Capture output exceeds size cap".into());
    }
    // PNG magic or accept as PNG from chromium.
    write_cached_image(domain, &bytes, backend.label)?;
    image_path(domain)
}

fn run_chromium(
    bin: &Path,
    domain: &str,
    url: &str,
    out: &Path,
    live_origin: bool,
) -> Result<(), String> {
    let mut last = String::from("Chromium capture failed");
    for headless in ["--headless=new", "--headless"] {
        let _ = fs::remove_file(out);
        let mut cmd = Command::new(bin);
        cmd.args([
            headless,
            "--disable-gpu",
            "--no-first-run",
            "--no-default-browser-check",
            "--disable-dev-shm-usage",
            "--hide-scrollbars",
            "--window-size=1280,800",
            "--virtual-time-budget=10000",
            "--ignore-certificate-errors",
            "--allow-insecure-localhost",
            "--no-sandbox",
            "--disable-notifications",
            "--deny-permission-prompts",
        ]);
        if !live_origin {
            cmd.arg(format!(
                "--host-resolver-rules={}",
                host_resolver_rules(domain)
            ));
        }
        cmd.arg(format!("--screenshot={}", out.display()));
        cmd.arg(url);
        cmd.stdout(Stdio::null()).stderr(Stdio::piped());
        match run_with_timeout(&mut cmd, CAPTURE_TIMEOUT) {
            Ok(()) if out.is_file() => return Ok(()),
            Ok(()) => last = "Chromium did not write a screenshot".into(),
            Err(err) => last = err,
        }
    }
    Err(last)
}

fn run_wkhtml(bin: &Path, url: &str, out: &Path) -> Result<(), String> {
    let mut cmd = Command::new(bin);
    cmd.args([
        "--quiet",
        "--width",
        "1280",
        "--height",
        "800",
        "--quality",
        "80",
        "--disable-javascript",
        url,
    ]);
    cmd.arg(out);
    cmd.stdout(Stdio::null()).stderr(Stdio::piped());
    run_with_timeout(&mut cmd, CAPTURE_TIMEOUT)?;
    if !out.is_file() {
        return Err("wkhtmltoimage did not write an image".into());
    }
    Ok(())
}

fn run_with_timeout(cmd: &mut Command, timeout: Duration) -> Result<(), String> {
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to start capture process: {e}"))?;
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if status.success() {
                    return Ok(());
                }
                let code = status.code().unwrap_or(-1);
                return Err(format!("Capture process exited with status {code}"));
            }
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err("Capture timed out".into());
                }
                std::thread::sleep(Duration::from_millis(120));
            }
            Err(e) => return Err(format!("Capture wait failed: {e}")),
        }
    }
}

/// True when at least one capture backend binary is present.
pub fn capture_available() -> bool {
    !discover_backends().is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidate_urls_include_http() {
        let urls = candidate_urls("lab-preview.example", false);
        assert!(
            urls.iter()
                .any(|u| u.starts_with("http://lab-preview.example"))
        );
    }

    #[test]
    fn live_origin_prefers_https() {
        let urls = candidate_urls("cmstest.newstargeted.com", true);
        assert!(
            urls.iter()
                .any(|u| u.starts_with("https://cmstest.newstargeted.com"))
        );
        assert_eq!(urls[0], "https://cmstest.newstargeted.com/");
    }

    #[test]
    fn resolver_rules_map_loopback() {
        let rules = host_resolver_rules("lab.example");
        assert!(rules.contains("MAP lab.example 127.0.0.1"));
        assert!(rules.contains("MAP www.lab.example 127.0.0.1"));
    }

    #[test]
    fn almalinux_headless_paths_cover_lib64() {
        let paths = almalinux_headless_paths();
        assert!(
            paths
                .iter()
                .any(|p| p.contains("/usr/lib64/chromium-browser/"))
        );
        assert!(paths.iter().any(|p| p.ends_with("headless_shell")));
    }
}
