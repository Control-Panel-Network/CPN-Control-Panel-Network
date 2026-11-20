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
    let heal = tokio::task::spawn_blocking(|| {
        if crate::apps_phpmyadmin_fast::runtime_dirs_need_heal() {
            let _ = crate::apps_phpmyadmin::ensure_phpmyadmin_runtime_dirs();
        }
        if !crate::apps_phpmyadmin_fast::pma_sock_ready() {
            let _ = crate::apps_phpmyadmin::ensure_fpm_socket_for_ols();
        }
        if !crate::apps_phpmyadmin_fast::pma_listener_ready() {
            let _ = crate::apps_phpmyadmin_sso::ensure_ols_phpmyadmin_listener();
        }
        port_open("127.0.0.1:8081", 200)
    })
    .await
    .unwrap_or(false);
    if !crate::apps_phpmyadmin_fast::pma_listener_ready() && !heal {
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
    let method = req.method().as_str().to_string();
    let body_bytes = match collect_body(payload).await {
        Ok(b) => b,
        Err(err) => {
            return HttpResponse::BadRequest()
                .content_type("text/plain; charset=utf-8")
                .body(format!("Could not read request body: {err}"));
        }
    };
    let headers = proxy_request_headers(&req);
    match forward_http_blocking(&method, &target, &headers, &body_bytes).await {
        Ok(resp) if resp.status() == StatusCode::SERVICE_UNAVAILABLE => {
            // LiteSpeed 503 when php-fpm sock is down; heal FPM then retry once.
            let _ = tokio::task::spawn_blocking(|| {
                let _ = crate::apps_phpmyadmin::ensure_fpm_socket_for_ols();
                let _ = crate::apps_phpmyadmin_sso::ensure_ols_phpmyadmin_listener();
            })
            .await;
            match forward_http_blocking(&method, &target, &headers, &body_bytes).await {
                Ok(retry) => retry,
                Err(_) => resp,
            }
        }
        Ok(resp) => resp,
        Err(err) => {
            let _ = tokio::task::spawn_blocking(|| {
                let _ = crate::apps_phpmyadmin::ensure_fpm_socket_for_ols();
                let _ = crate::apps_phpmyadmin_sso::ensure_ols_phpmyadmin_listener();
            })
            .await;
            match forward_http_blocking(&method, &target, &headers, &body_bytes).await {
                Ok(resp) => resp,
                Err(err2) => HttpResponse::BadGateway()
                    .content_type("text/plain; charset=utf-8")
                    .body(format!(
                        "phpMyAdmin proxy could not reach {} ({err}; retry: {err2}). Health URL: {}.",
                        BACKEND,
                        phpmyadmin_health_url()
                    )),
            }
        }
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

fn proxy_max_time(target: &str) -> &'static str {
    if target.contains("ajax_request=1")
        || target.contains("/navigation")
        || target.contains("/config/")
    {
        "35"
    } else {
        "120"
    }
}

fn proxy_request_headers(req: &HttpRequest) -> Vec<(String, String)> {
    let mut out = Vec::new();
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
            out.push((name.as_str().to_string(), v.to_string()));
        }
    }
    out
}

#[derive(Clone)]
struct ProxiedHttp {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

async fn forward_http_blocking(
    method: &str,
    target: &str,
    headers: &[(String, String)],
    body: &[u8],
) -> Result<HttpResponse, String> {
    let method = method.to_string();
    let target = target.to_string();
    let headers = headers.to_vec();
    let body = body.to_vec();
    let proxied = tokio::task::spawn_blocking(move || {
        forward_http_with_headers(&method, &target, &headers, &body)
    })
    .await
    .map_err(|_| "phpMyAdmin proxy task failed".to_string())??;
    Ok(proxied_to_response(proxied))
}

fn proxied_to_response(proxied: ProxiedHttp) -> HttpResponse {
    let status = StatusCode::from_u16(proxied.status).unwrap_or(StatusCode::BAD_GATEWAY);
    let mut builder = HttpResponse::build(status);
    for (name, value) in proxied.headers {
        builder.insert_header((name, value));
    }
    builder.body(proxied.body)
}

fn forward_http_with_headers(
    method: &str,
    target: &str,
    headers: &[(String, String)],
    body: &[u8],
) -> Result<ProxiedHttp, String> {
    let mut cmd = Command::new("curl");
    cmd.args([
        "--silent",
        "--show-error",
        "--include",
        "--max-time",
        proxy_max_time(target),
        "-X",
        method,
        "--path-as-is",
    ]);
    for (name, value) in headers {
        cmd.args(["-H", &format!("{name}: {value}")]);
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

fn parse_curl_include(raw: &[u8]) -> Result<ProxiedHttp, String> {
    // Keep the body as raw bytes. UTF-8 decoding corrupts PNG/GIF/JS themes
    // (0x89 PNG signature becomes U+FFFD) and breaks the phpMyAdmin UI.
    let (header_block, body) = split_headers_body_bytes(raw)
        .ok_or_else(|| "Malformed proxy response from phpMyAdmin backend".to_string())?;
    let header_text = String::from_utf8_lossy(header_block);
    let mut lines = header_text.lines();
    let status_line = lines
        .next()
        .ok_or_else(|| "Missing HTTP status from phpMyAdmin backend".to_string())?;
    let status = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(502);
    let mut out_headers = Vec::new();
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            let name = name.trim().to_string();
            let value = value.trim();
            let lower = name.to_ascii_lowercase();
            if matches!(
                lower.as_str(),
                "transfer-encoding" | "content-length" | "connection" | "content-encoding"
            ) {
                continue;
            }
            let value = if lower == "location" {
                rewrite_location(value)
            } else if lower == "set-cookie" {
                rewrite_set_cookie(value)
            } else {
                value.to_string()
            };
            out_headers.push((name, value));
        }
    }
    Ok(ProxiedHttp {
        status,
        headers: out_headers,
        body: body.to_vec(),
    })
}

fn find_header_body_sep(raw: &[u8]) -> Option<(usize, usize)> {
    raw.windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|i| (i, 4))
        .or_else(|| raw.windows(2).position(|w| w == b"\n\n").map(|i| (i, 2)))
}

fn split_headers_body_bytes(raw: &[u8]) -> Option<(&[u8], &[u8])> {
    let mut rest = raw;
    loop {
        let (sep_at, sep_len) = find_header_body_sep(rest)?;
        let headers = &rest[..sep_at];
        let body = &rest[sep_at + sep_len..];
        let header_text = String::from_utf8_lossy(headers);
        if header_text.contains(" 100 ") || header_text.starts_with("HTTP/1.1 100") {
            rest = body;
            continue;
        }
        return Some((headers, body));
    }
}

fn rewrite_location(value: &str) -> String {
    if value.starts_with("http://127.0.0.1:8081") {
        return value.replacen("http://127.0.0.1:8081", PMA_MOUNT, 1);
    }
    // Keep panel routes absolute. SignonURL remints via /databases/phpmyadmin/open
    // after php-fpm restarts; never prefix those with /phpmyadmin.
    if is_panel_absolute_location(value) {
        return value.to_string();
    }
    if value.starts_with('/') && !value.starts_with(PMA_MOUNT) {
        return format!("{PMA_MOUNT}{value}");
    }
    value.to_string()
}

fn is_panel_absolute_location(value: &str) -> bool {
    const PREFIXES: &[&str] = &[
        "/databases/",
        "/login",
        "/logout",
        "/dashboard",
        "/server/",
        "/account/",
        "/websites/",
        "/email/",
        "/ftp/",
        "/plugins/",
        "/wordpress/",
        "/backups/",
        "/packages",
        "/settings",
        "/security",
        "/api/",
    ];
    PREFIXES.iter().any(|p| value == *p || value.starts_with(p))
}

fn rewrite_set_cookie(value: &str) -> String {
    // Scope cookies to the mount and match CPN panel session TTL (12h).
    // Proxy access already requires a live cpn_panel_session; cookie Max-Age alone
    // must not outlive the panel session.
    let ttl = crate::panel_session::SESSION_TTL_SECONDS;
    let mut parts = Vec::new();
    let mut saw_path = false;
    let mut saw_samesite = false;
    for part in value.split(';') {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }
        let lower = trimmed.to_ascii_lowercase();
        if lower.starts_with("path=") {
            parts.push(format!("Path={PMA_MOUNT}"));
            saw_path = true;
        } else if lower.starts_with("max-age=") || lower.starts_with("expires=") {
            // Drop backend expiry; apply panel-aligned Max-Age below.
            continue;
        } else if lower.starts_with("samesite=") {
            parts.push("SameSite=Lax".into());
            saw_samesite = true;
        } else {
            parts.push(trimmed.to_string());
        }
    }
    if !saw_path {
        parts.push(format!("Path={PMA_MOUNT}"));
    }
    parts.push(format!("Max-Age={ttl}"));
    if !saw_samesite {
        parts.push("SameSite=Lax".into());
    }
    parts.join("; ")
}

/// Cookie names phpMyAdmin / CPN sign-on commonly set under `/phpmyadmin`.
pub const PMA_SESSION_COOKIE_NAMES: &[&str] = &[
    "phpMyAdmin",
    "phpMyAdmin_https",
    "pma_lang",
    "pma_collation_connection",
    "pmaUser-1",
    "pmaPass-1",
    "CPNPmaSignon",
];

/// Clear phpMyAdmin cookies on CPN logout so a prior PMA session cannot linger.
pub fn clear_phpmyadmin_cookie_headers(secure: bool) -> Vec<String> {
    let secure_flag = if secure { "; Secure" } else { "" };
    PMA_SESSION_COOKIE_NAMES
        .iter()
        .map(|name| {
            format!("{name}=; Path={PMA_MOUNT}; HttpOnly; SameSite=Lax; Max-Age=0{secure_flag}")
        })
        .collect()
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
        assert_eq!(
            rewrite_location("/databases/phpmyadmin/open"),
            "/databases/phpmyadmin/open"
        );
        assert_eq!(rewrite_location("/login"), "/login");
        assert!(
            rewrite_set_cookie("phpMyAdmin=abc; path=/; HttpOnly").contains("Path=/phpmyadmin")
        );
        let aligned = rewrite_set_cookie("phpMyAdmin=abc; path=/; Max-Age=999999; HttpOnly");
        assert!(aligned.contains(&format!(
            "Max-Age={}",
            crate::panel_session::SESSION_TTL_SECONDS
        )));
        assert!(!aligned.contains("999999"));
        let cleared = clear_phpmyadmin_cookie_headers(false);
        assert!(
            cleared
                .iter()
                .any(|c| c.starts_with("phpMyAdmin=") && c.contains("Max-Age=0"))
        );
        assert!(cleared.iter().any(|c| c.starts_with("CPNPmaSignon=")));
    }

    #[test]
    fn binary_body_survives_header_split() {
        let png = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR";
        let mut raw = b"HTTP/1.1 200 OK\r\nContent-Type: image/png\r\n\r\n".to_vec();
        raw.extend_from_slice(png);
        let (headers, body) = split_headers_body_bytes(&raw).expect("split");
        assert!(String::from_utf8_lossy(headers).contains("200 OK"));
        assert_eq!(&body[..8], b"\x89PNG\r\n\x1a\n");
        let parsed = parse_curl_include(&raw).expect("parse");
        assert_eq!(parsed.status, StatusCode::OK.as_u16());
        let resp = proxied_to_response(parsed);
        assert_eq!(resp.status(), StatusCode::OK);
    }
}
