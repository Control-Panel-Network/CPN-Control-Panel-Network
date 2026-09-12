//! Reverse-proxy phpMyAdmin from the panel mount `/phpmyadmin` to loopback :8081.
//!
//! Host browsers (VirtualBox NAT labs) cannot reach a guest-only `127.0.0.1:8081`
//! bind. Serving under the panel port keeps Open + auto-login on one URL.

use crate::apps_phpmyadmin::phpmyadmin_health_url;
use crate::panel_feature_gate::phpmyadmin_installed;
use crate::service_detect::port_open;
use actix_web::{HttpRequest, HttpResponse, http::Method, http::StatusCode, web};
use futures_util::StreamExt;
use std::process::{Command, Stdio};

pub const PMA_MOUNT: &str = "/phpmyadmin";
const BACKEND: &str = "http://127.0.0.1:8081";

/// True when catch-all should reverse-proxy phpMyAdmin under the panel port.
pub fn path_should_proxy_phpmyadmin(req_path: &str) -> bool {
    req_path == PMA_MOUNT
        || req_path == format!("{PMA_MOUNT}/")
        || req_path.starts_with(&format!("{PMA_MOUNT}/"))
}

fn backend_path(req_path: &str) -> Option<String> {
    if !path_should_proxy_phpmyadmin(req_path) {
        return None;
    }
    let rest = req_path.strip_prefix(PMA_MOUNT).unwrap_or("");
    if rest.is_empty() || rest == "/" {
        return Some("/index.php".into());
    }
    Some(rest.to_string())
}

/// Catch-all proxy for `/phpmyadmin` and nested assets (requires panel session).
pub async fn phpmyadmin_panel_proxy(req: HttpRequest, payload: web::Payload) -> HttpResponse {
    if !phpmyadmin_installed() {
        return HttpResponse::NotFound()
            .content_type("text/plain; charset=utf-8")
            .body("phpMyAdmin is not installed on this host.");
    }
    if !port_open("127.0.0.1:8081", 200) {
        let _ = crate::apps_phpmyadmin_sso::ensure_ols_phpmyadmin_listener();
    }
    if !port_open("127.0.0.1:8081", 500) {
        return HttpResponse::BadGateway()
            .content_type("text/plain; charset=utf-8")
            .body(format!(
                "phpMyAdmin loopback listener is not ready ({BACKEND}). Open auto-login wires OpenLiteSpeed :8081 when needed."
            ));
    }
    let Some(backend_path) = backend_path(req.path()) else {
        return HttpResponse::NotFound().finish();
    };
    let query = req.query_string();
    let target = if query.is_empty() {
        format!("{BACKEND}{backend_path}")
    } else {
        format!("{BACKEND}{backend_path}?{query}")
    };
    let method = req.method().as_str();
    let body_bytes = match collect_body(payload).await {
        Ok(b) => b,
        Err(err) => {
            return HttpResponse::BadRequest()
                .content_type("text/plain; charset=utf-8")
                .body(format!("Could not read request body: {err}"));
        }
    };
    match forward_http(method, &target, &req, &body_bytes) {
        Ok(resp) => resp,
        Err(err) => HttpResponse::BadGateway()
            .content_type("text/plain; charset=utf-8")
            .body(format!(
                "phpMyAdmin proxy could not reach {} ({err}). Health URL: {}.",
                BACKEND,
                phpmyadmin_health_url()
            )),
    }
}

async fn collect_body(mut payload: web::Payload) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    while let Some(chunk) = payload.next().await {
        let chunk = chunk.map_err(|e| e.to_string())?;
        if bytes.len() + chunk.len() > 16 * 1024 * 1024 {
            return Err("Request body too large".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn forward_http(
    method: &str,
    target: &str,
    req: &HttpRequest,
    body: &[u8],
) -> Result<HttpResponse, String> {
    let mut cmd = Command::new("curl");
    cmd.args([
        "--silent",
        "--show-error",
        "--include",
        "--max-time",
        "120",
        "-X",
        method,
        "--path-as-is",
    ]);
    for (name, value) in req.headers() {
        let lower = name.as_str().to_ascii_lowercase();
        if matches!(
            lower.as_str(),
            "host"
                | "content-length"
                | "connection"
                | "transfer-encoding"
                | "keep-alive"
                | "proxy-authenticate"
                | "proxy-authorization"
                | "te"
                | "trailers"
                | "upgrade"
                | "accept-encoding"
        ) {
            continue;
        }
        if let Ok(v) = value.to_str() {
            cmd.args(["-H", &format!("{}: {}", name.as_str(), v)]);
        }
    }
    cmd.args(["-H", "Host: 127.0.0.1:8081"]);
    cmd.args(["-H", "Accept-Encoding: identity"]);
    if !body.is_empty()
        && matches!(
            method.to_ascii_uppercase().as_str(),
            "POST" | "PUT" | "PATCH"
        )
    {
        cmd.args(["--data-binary", "@-"]);
    }
    cmd.arg(target);
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("curl missing or failed to start: {e}"))?;
    if !body.is_empty()
        && matches!(
            method.to_ascii_uppercase().as_str(),
            "POST" | "PUT" | "PATCH"
        )
    {
        use std::io::Write;
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(body)
                .map_err(|e| format!("Could not write proxy body: {e}"))?;
        }
    }
    let output = child
        .wait_with_output()
        .map_err(|e| format!("curl wait failed: {e}"))?;
    if !output.status.success() && output.stdout.is_empty() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(err.trim().chars().take(240).collect());
    }
    parse_curl_include(&output.stdout)
}

fn parse_curl_include(raw: &[u8]) -> Result<HttpResponse, String> {
    let text = String::from_utf8_lossy(raw);
    let (header_block, body) = split_headers_body(&text)
        .ok_or_else(|| "Malformed proxy response from phpMyAdmin backend".to_string())?;
    let mut lines = header_block.lines();
    let status_line = lines
        .next()
        .ok_or_else(|| "Missing HTTP status from phpMyAdmin backend".to_string())?;
    let status = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(502);
    let status = StatusCode::from_u16(status).unwrap_or(StatusCode::BAD_GATEWAY);
    let mut builder = HttpResponse::build(status);
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            let name = name.trim();
            let value = value.trim();
            let lower = name.to_ascii_lowercase();
            if matches!(
                lower.as_str(),
                "transfer-encoding" | "content-length" | "connection" | "content-encoding"
            ) {
                continue;
            }
            if lower == "location" {
                builder.insert_header((name, rewrite_location(value)));
                continue;
            }
            if lower == "set-cookie" {
                builder.insert_header((name, rewrite_set_cookie(value)));
                continue;
            }
            builder.insert_header((name, value));
        }
    }
    Ok(builder.body(body.to_string()))
}

fn split_headers_body(text: &str) -> Option<(&str, &str)> {
    let mut rest = text;
    loop {
        if let Some(idx) = rest.find("\r\n\r\n") {
            let headers = &rest[..idx];
            let body = &rest[idx + 4..];
            if headers.contains(" 100 ") || headers.starts_with("HTTP/1.1 100") {
                rest = body;
                continue;
            }
            return Some((headers, body));
        }
        if let Some(idx) = rest.find("\n\n") {
            let headers = &rest[..idx];
            let body = &rest[idx + 2..];
            if headers.contains(" 100 ") {
                rest = body;
                continue;
            }
            return Some((headers, body));
        }
        return None;
    }
}

fn rewrite_location(value: &str) -> String {
    if value.starts_with("http://127.0.0.1:8081") {
        return value.replacen("http://127.0.0.1:8081", PMA_MOUNT, 1);
    }
    if value.starts_with('/') && !value.starts_with(PMA_MOUNT) {
        return format!("{PMA_MOUNT}{value}");
    }
    value.to_string()
}

fn rewrite_set_cookie(value: &str) -> String {
    // Keep cookies scoped to the panel mount so they do not leak across panel routes.
    if value.to_ascii_lowercase().contains("path=") {
        let mut parts = Vec::new();
        for part in value.split(';') {
            let trimmed = part.trim();
            if trimmed.to_ascii_lowercase().starts_with("path=") {
                parts.push(format!("Path={PMA_MOUNT}"));
            } else if !trimmed.is_empty() {
                parts.push(trimmed.to_string());
            }
        }
        return parts.join("; ");
    }
    format!("{value}; Path={PMA_MOUNT}")
}

pub fn allowed_proxy_method(method: &Method) -> bool {
    matches!(
        *method,
        Method::GET | Method::HEAD | Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mount_paths_match() {
        assert!(path_should_proxy_phpmyadmin("/phpmyadmin"));
        assert!(path_should_proxy_phpmyadmin("/phpmyadmin/"));
        assert!(path_should_proxy_phpmyadmin("/phpmyadmin/index.php"));
        assert!(!path_should_proxy_phpmyadmin("/databases/phpmyadmin"));
        assert!(!path_should_proxy_phpmyadmin("/phpmyadminx"));
    }

    #[test]
    fn backend_paths_strip_mount() {
        assert_eq!(backend_path("/phpmyadmin").as_deref(), Some("/index.php"));
        assert_eq!(
            backend_path("/phpmyadmin/cpn-signon.php").as_deref(),
            Some("/cpn-signon.php")
        );
        assert_eq!(
            backend_path("/phpmyadmin/themes/pmahomme/img/logo_left.png").as_deref(),
            Some("/themes/pmahomme/img/logo_left.png")
        );
    }

    #[test]
    fn location_and_cookie_rewrite() {
        assert_eq!(rewrite_location("/index.php"), "/phpmyadmin/index.php");
        assert_eq!(
            rewrite_location("http://127.0.0.1:8081/cpn-signon.php"),
            "/phpmyadmin/cpn-signon.php"
        );
        assert!(
            rewrite_set_cookie("phpMyAdmin=abc; path=/; HttpOnly").contains("Path=/phpmyadmin")
        );
    }
}
