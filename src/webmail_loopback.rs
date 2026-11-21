//! Loopback HTTP port for panel webmail (Tachyon / SnappyMail / Roundcube).
//!
//! CPN prefers `127.0.0.1:8080`. When a foreign process already owns that port
//! (for example a PM2 app bound to `0.0.0.0:8080`), LiteSpeed/Nginx/Caddy cannot
//! bind the webmail listener and the panel proxy serves a blank/404 page. This
//! module picks a free loopback port, persists it, and exposes helpers for the
//! proxy + engine snippets.

use serde::{Deserialize, Serialize};
use std::net::{SocketAddr, TcpListener};
use std::path::Path;
use std::process::Command;

pub const DEFAULT_PORT: u16 = 8080;
/// Prefer 8080, then nearby reserved loopback alternatives.
pub const CANDIDATE_PORTS: &[u16] = &[8080, 8088, 18081, 18082, 18083];
const STORE: &str = "/var/lib/cpn/webmail-loopback.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct LoopbackStore {
    port: u16,
    #[serde(default)]
    note: String,
}

pub fn loopback_port() -> u16 {
    load_store()
        .map(|s| s.port)
        .filter(|p| *p > 0)
        .unwrap_or(DEFAULT_PORT)
}

pub fn loopback_origin() -> String {
    format!("http://127.0.0.1:{}", loopback_port())
}

pub fn loopback_base_url() -> String {
    format!("{}/", loopback_origin())
}

pub fn host_header_value(port: u16) -> String {
    format!("127.0.0.1:{port}")
}

/// Ensure a usable loopback port is selected and persisted.
/// Keeps the saved port when it is free or already held by a CPN web server.
/// Otherwise picks the first free candidate and records why.
pub fn ensure_loopback_port() -> Result<u16, String> {
    let current = loopback_port();
    if port_usable_for_webmail(current) {
        persist(current, None)?;
        return Ok(current);
    }
    let holder = foreign_listener_summary(current).unwrap_or_else(|| "unknown process".into());
    for &port in CANDIDATE_PORTS {
        if port == current {
            continue;
        }
        if port_usable_for_webmail(port) {
            let note = format!(
                "moved off {current}: held by {holder}; using {port} for CPN webmail loopback"
            );
            persist(port, Some(note))?;
            return Ok(port);
        }
    }
    Err(format!(
        "No free loopback port for CPN webmail among {CANDIDATE_PORTS:?}. \
         Port {current} is held by {holder}. Stop that process or free one of the candidate ports."
    ))
}

fn port_usable_for_webmail(port: u16) -> bool {
    if can_bind_loopback(port) {
        return true;
    }
    listener_looks_like_cpn_web_server(port)
}

fn can_bind_loopback(port: u16) -> bool {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    TcpListener::bind(addr).is_ok()
}

fn listener_looks_like_cpn_web_server(port: u16) -> bool {
    let Some(summary) = foreign_listener_summary(port) else {
        return false;
    };
    let lower = summary.to_ascii_lowercase();
    lower.contains("litespeed")
        || lower.contains("openlitespeed")
        || lower.contains("lshttpd")
        || lower.contains("nginx")
        || lower.contains("caddy")
}

/// Best-effort process summary for who listens on `127.0.0.1:port` or `*:port`.
pub fn foreign_listener_summary(port: u16) -> Option<String> {
    let output = Command::new("ss").args(["-ltnp"]).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let needle_a = format!(":{port} ");
    let needle_b = format!(":{port}\n");
    for line in text.lines() {
        if !(line.contains(&needle_a)
            || line.contains(&needle_b)
            || line.ends_with(&format!(":{port}")))
        {
            continue;
        }
        // Require a listen on this exact port token (avoid :80800).
        if !line_has_port(line, port) {
            continue;
        }
        if let Some(users) = line.split("users:(").nth(1) {
            return Some(users.trim_end_matches(')').to_string());
        }
        return Some(line.trim().to_string());
    }
    None
}

fn line_has_port(line: &str, port: u16) -> bool {
    for token in line.split_whitespace() {
        if let Some(after) = token.rsplit_once(':')
            && after.1.parse::<u16>().ok() == Some(port)
        {
            return true;
        }
    }
    false
}

fn load_store() -> Option<LoopbackStore> {
    let raw = std::fs::read_to_string(STORE).ok()?;
    serde_json::from_str(&raw).ok()
}

fn persist(port: u16, note: Option<String>) -> Result<(), String> {
    let dir = Path::new("/var/lib/cpn");
    if dir.is_dir() {
        let body = LoopbackStore {
            port,
            note: note.unwrap_or_default(),
        };
        let json = serde_json::to_string_pretty(&body).map_err(|e| e.to_string())?;
        std::fs::write(STORE, format!("{json}\n")).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(STORE, PermissionsExt::from_mode(0o600));
        }
    }
    Ok(())
}

/// True when an HTTP probe of the loopback URL does not look like CPN webmail.
/// Used during heal to detect a foreign app answering on the preferred port.
pub fn loopback_response_looks_foreign(body: &str, status: u16) -> bool {
    if status == 0 {
        return true;
    }
    let lower = body.to_ascii_lowercase();
    if lower.contains("snappymail")
        || lower.contains("tachyon")
        || lower.contains("roundcube")
        || lower.contains("rainloop")
        || lower.contains("rl-started-trigger")
        || lower.contains("cpn webmail")
        || lower.contains("login")
    {
        return false;
    }
    // Bare 404/empty from a non-webmail HTTP service (for example NT-API).
    status == 404 || body.trim().is_empty() || lower.contains("\"error\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_origin_uses_8080() {
        // Without a store file in the test process, helpers fall back to DEFAULT_PORT.
        assert_eq!(DEFAULT_PORT, 8080);
        assert!(CANDIDATE_PORTS.contains(&DEFAULT_PORT));
        assert_eq!(host_header_value(8080), "127.0.0.1:8080");
    }

    #[test]
    fn foreign_response_detection() {
        assert!(loopback_response_looks_foreign("", 404));
        assert!(loopback_response_looks_foreign(
            "{\"error\":\"missing\"}",
            404
        ));
        assert!(!loopback_response_looks_foreign(
            "<html class=\"rl-started-trigger\">Tachyon login",
            200
        ));
        assert!(!loopback_response_looks_foreign("Roundcube Webmail", 200));
    }

    #[test]
    fn line_port_match_is_exact() {
        assert!(line_has_port("LISTEN 0 0 0.0.0.0:8080 0.0.0.0:*", 8080));
        assert!(!line_has_port("LISTEN 0 0 0.0.0.0:80800 0.0.0.0:*", 8080));
        assert!(line_has_port("LISTEN 0 0 127.0.0.1:18081 0.0.0.0:*", 18081));
    }
}
