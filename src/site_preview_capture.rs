//! Capture site homepage screenshots via headless Chromium (or wkhtmltoimage).
//!
//! Lab hosts often lack public DNS. Capture maps the managed domain to loopback
//! (and optional LAN IPs) with Chromium `--host-resolver-rules` so local vhosts
//! still render when the panel can reach them.

use crate::site_preview_thumb::{
    PREVIEW_MAX_BYTES, image_path, record_capture_failure, write_cached_image,
};
use crate::sites::normalize_domain;
use crate::website_preview::{public_site_url, ssl_material_present};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

const CAPTURE_TIMEOUT: Duration = Duration::from_secs(25);

/// Run a capture for a registry domain. Overwrites cache on success.
pub fn capture_site_preview(domain_raw: &str) -> Result<PathBuf, String> {
    let domain = normalize_domain(domain_raw)?;
    let backends = discover_backends();
    if backends.is_empty() {
        let err = "No headless browser found (install chromium or google-chrome for Site preview)";
        let _ = record_capture_failure(&domain, err, "none");
        return Err(err.into());
    }

    let urls = candidate_urls(&domain);
    let mut last_err = String::from("Capture failed");
    for backend in &backends {
        for url in &urls {
            match run_capture(backend, &domain, url) {
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
    let chromium_names = [
        "chromium-browser",
        "chromium",
        "google-chrome",
        "google-chrome-stable",
        "chrome",
        "chromium-headless-shell",
    ];
    for name in chromium_names {
        if let Some(bin) = find_bin(name) {
            out.push(CaptureBackend {
                label: "chromium",
                kind: BackendKind::Chromium,
                bin,
            });
            break;
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

fn find_bin(name: &str) -> Option<PathBuf> {
    // Absolute common paths first (AlmaLinux / container installs).
    let absolutes = [
        format!("/usr/bin/{name}"),
        format!("/usr/local/bin/{name}"),
        format!("/opt/google/chrome/{name}"),
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

fn candidate_urls(domain: &str) -> Vec<String> {
    let mut urls = Vec::new();
    if let Ok(public) = public_site_url(domain) {
        urls.push(format!("{}/", public.trim_end_matches('/')));
    }
    // Prefer mapped local HTTP for labs without public DNS / TLS.
    urls.push(format!("http://{domain}/"));
    if ssl_material_present(domain) {
        urls.push(format!("https://{domain}/"));
    }
    urls.sort();
    urls.dedup();
    // Put http local first when no certs (common in VBox labs).
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

fn run_capture(backend: &CaptureBackend, domain: &str, url: &str) -> Result<PathBuf, String> {
    let dir = crate::site_preview_thumb::ensure_preview_dir()?;
    let out = dir.join(format!("{domain}.capture.png"));
    let _ = fs::remove_file(&out);

    match backend.kind {
        BackendKind::Chromium => run_chromium(&backend.bin, domain, url, &out)?,
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

fn run_chromium(bin: &Path, domain: &str, url: &str, out: &Path) -> Result<(), String> {
    let rules = host_resolver_rules(domain);
    let mut cmd = Command::new(bin);
    cmd.args([
        "--headless=new",
        "--disable-gpu",
        "--no-first-run",
        "--no-default-browser-check",
        "--disable-dev-shm-usage",
        "--hide-scrollbars",
        "--window-size=1280,800",
        "--virtual-time-budget=10000",
        "--ignore-certificate-errors",
        "--allow-insecure-localhost",
    ]);
    // Root / service accounts often need this on AlmaLinux.
    cmd.arg("--no-sandbox");
    cmd.arg(format!("--host-resolver-rules={rules}"));
    cmd.arg(format!("--screenshot={}", out.display()));
    cmd.arg(url);
    cmd.stdout(Stdio::null()).stderr(Stdio::piped());
    run_with_timeout(&mut cmd, CAPTURE_TIMEOUT)?;
    if !out.is_file() {
        return Err("Chromium did not write a screenshot".into());
    }
    Ok(())
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
        let urls = candidate_urls("lab-preview.example");
        assert!(
            urls.iter()
                .any(|u| u.starts_with("http://lab-preview.example"))
        );
    }

    #[test]
    fn resolver_rules_map_loopback() {
        let rules = host_resolver_rules("lab.example");
        assert!(rules.contains("MAP lab.example 127.0.0.1"));
        assert!(!rules.to_lowercase().contains("cyberpanel"));
    }
}
