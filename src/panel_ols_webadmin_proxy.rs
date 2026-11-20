//! Reverse-proxy OpenLiteSpeed WebAdmin from `/ols` to the loopback admin listener (:7080).
//!
//! Browsers cannot use `https://127.0.0.1:7080` when WebAdmin binds on the server only.
//! The mount stays on the panel origin and requires a panel session (same pattern as phpMyAdmin).

use crate::litespeed_stack::{WEBADMIN_PANEL_MOUNT, any_litespeed_installed, webadmin_backend_url};
use crate::panel_session::request_https_from_headers;
use actix_web::{HttpRequest, HttpResponse, http::Method, http::StatusCode, web};
use futures_util::StreamExt;
use std::process::{Command, Stdio};

const CURL_MAX_TIME: &str = "120";

pub fn path_should_proxy_ols_webadmin(req_path: &str) -> bool {
    req_path == WEBADMIN_PANEL_MOUNT
        || req_path == format!("{WEBADMIN_PANEL_MOUNT}/")
        || req_path.starts_with(&format!("{WEBADMIN_PANEL_MOUNT}/"))
}

fn backend_path(req_path: &str) -> Option<String> {
    if !path_should_proxy_ols_webadmin(req_path) {
        return None;
    }
    let rest = req_path.strip_prefix(WEBADMIN_PANEL_MOUNT).unwrap_or("");
    if rest.is_empty() || rest == "/" {
        return Some("/".into());
    }
    Some(rest.to_string())
}

fn backend_base() -> String {
    webadmin_backend_url().trim_end_matches('/').to_string()
}

fn backend_host_header(base: &str) -> String {
    base.strip_prefix("https://")
        .or_else(|| base.strip_prefix("http://"))
        .unwrap_or("127.0.0.1:7080")
        .to_string()
}

/// Catch-all proxy for `/ols` and nested WebAdmin assets (requires panel session).
pub async fn ols_webadmin_panel_proxy(req: HttpRequest, payload: web::Payload) -> HttpResponse {
    if !any_litespeed_installed() {
        return HttpResponse::NotFound()
            .content_type("text/plain; charset=utf-8")
            .body("LiteSpeed WebAdmin is not available (no LiteSpeed install detected).");
    }
    // Relative WebAdmin links (`view/...`, `login.php`) need the trailing slash on the mount.
    if req.path() == WEBADMIN_PANEL_MOUNT && matches!(*req.method(), Method::GET | Method::HEAD) {
        let query = req.query_string();
        let location = if query.is_empty() {
            format!("{WEBADMIN_PANEL_MOUNT}/")
        } else {
            format!("{WEBADMIN_PANEL_MOUNT}/?{query}")
        };
        return HttpResponse::Found()
            .insert_header(("Location", location))
            .finish();
    }
    let Some(backend_rel) = backend_path(req.path()) else {
        return HttpResponse::NotFound().finish();
    };
    let base = backend_base();
    let query = req.query_string();
    let target = if query.is_empty() {
        format!("{base}{backend_rel}")
    } else {
        format!("{base}{backend_rel}?{query}")
    };
    let method = req.method().as_str().to_string();
    let body_bytes = match collect_body(payload).await {
        Ok(b) => b,
        Err(err) => {
            return HttpResponse::BadGateway()
                .content_type("text/plain; charset=utf-8")
                .body(format!("Could not read request body: {err}"));
        }
    };
    let headers = proxy_request_headers(&req, &base);
    match forward_http_blocking(&method, &target, &headers, &body_bytes, &base).await {
        Ok(resp) => resp,
        Err(err) => HttpResponse::BadGateway()
            .content_type("text/plain; charset=utf-8")
            .body(format!(
                "LiteSpeed WebAdmin proxy could not reach {base} ({err}). Is WebAdmin enabled and listening on the admin port?"
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

/// Request headers that must never reach WebAdmin. Client-address headers are dropped so the
/// WebAdmin IP ACL sees the loopback peer (127.0.0.1) and never a per-operator home IP.
/// Panel auth stays in front of this proxy (`require_panel_user`).
fn is_dropped_request_header(lower: &str) -> bool {
    matches!(
        lower,
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
            | "forwarded"
            | "x-forwarded-for"
            | "x-forwarded-host"
            | "x-forwarded-proto"
            | "x-forwarded-port"
            | "x-forwarded-server"
            | "x-real-ip"
            | "x-client-ip"
            | "x-cluster-client-ip"
            | "client-ip"
            | "true-client-ip"
            | "cf-connecting-ip"
            | "cf-connecting-ipv6"
            | "fastly-client-ip"
            | "x-original-forwarded-for"
            | "via"
    )
}

/// Remove CPN panel auth cookies so the panel session is never handed to WebAdmin.
fn strip_panel_cookies(cookie_header: &str) -> Option<String> {
    let kept: Vec<&str> = cookie_header
        .split(';')
        .map(str::trim)
        .filter(|pair| !pair.is_empty())
        .filter(|pair| {
            let name = pair.split('=').next().unwrap_or("").trim();
            !name.starts_with("cpn_panel_")
        })
        .collect();
    if kept.is_empty() {
        None
    } else {
        Some(kept.join("; "))
    }
}

/// `Origin`/`Referer` pointing at the public panel host are mapped to the WebAdmin origin so
/// WebAdmin same-origin checks pass behind the `/ols` mount.
fn rewrite_origin_header(value: &str, backend_base: &str) -> String {
    let _ = value;
    backend_base.trim_end_matches('/').to_string()
}

fn rewrite_referer_header(value: &str, backend_base: &str) -> String {
    let base = backend_base.trim_end_matches('/');
    let after_scheme = value
        .strip_prefix("https://")
        .or_else(|| value.strip_prefix("http://"));
    if let Some(rest) = after_scheme
        && let Some(slash) = rest.find('/')
    {
        let path = &rest[slash..];
        let mount_slash = format!("{WEBADMIN_PANEL_MOUNT}/");
        if let Some(inner) = path.strip_prefix(&mount_slash) {
            return format!("{base}/{inner}");
        }
        if path == WEBADMIN_PANEL_MOUNT {
            return format!("{base}/");
        }
    }
    format!("{base}/")
}

fn proxy_request_headers_from(
    pairs: &[(String, String)],
    backend_base: &str,
) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (name, value) in pairs {
        let lower = name.to_ascii_lowercase();
        if is_dropped_request_header(&lower) {
            continue;
        }
        match lower.as_str() {
            "cookie" => {
                if let Some(c) = strip_panel_cookies(value) {
                    out.push((name.clone(), c));
                }
            }
            "origin" => out.push((name.clone(), rewrite_origin_header(value, backend_base))),
            "referer" => out.push((name.clone(), rewrite_referer_header(value, backend_base))),
            _ => out.push((name.clone(), value.clone())),
        }
    }
    out
}

fn proxy_request_headers(req: &HttpRequest, backend_base: &str) -> Vec<(String, String)> {
    let pairs: Vec<(String, String)> = req
        .headers()
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|v| (name.as_str().to_string(), v.to_string()))
        })
        .collect();
    proxy_request_headers_from(&pairs, backend_base)
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
    backend_base: &str,
) -> Result<HttpResponse, String> {
    let method = method.to_string();
    let target = target.to_string();
    let headers = headers.to_vec();
    let body = body.to_vec();
    let backend_base = backend_base.to_string();
    let proxied = tokio::task::spawn_blocking(move || {
        forward_http_with_headers(&method, &target, &headers, &body, &backend_base)
    })
    .await
    .map_err(|_| "WebAdmin proxy task failed".to_string())??;
    Ok(proxied_to_response(proxied))
}

fn proxied_to_response(proxied: ProxiedHttp) -> HttpResponse {
    let status = StatusCode::from_u16(proxied.status).unwrap_or(StatusCode::BAD_GATEWAY);
    let mut builder = HttpResponse::build(status);
    // append_header keeps repeated headers (several Set-Cookie lines) instead of overwriting.
    for (name, value) in proxied.headers {
        builder.append_header((name, value));
    }
    builder.body(proxied.body)
}

fn forward_http_with_headers(
    method: &str,
    target: &str,
    headers: &[(String, String)],
    body: &[u8],
    backend_base: &str,
) -> Result<ProxiedHttp, String> {
    let host = backend_host_header(backend_base);
    let mut cmd = Command::new("curl");
    cmd.args([
        "--silent",
        "--show-error",
        "--include",
        "--insecure",
        "--noproxy",
        "*",
        "--max-time",
        CURL_MAX_TIME,
        "-X",
        method,
        "--path-as-is",
    ]);
    for (name, value) in headers {
        cmd.args(["-H", &format!("{name}: {value}")]);
    }
    cmd.args(["-H", &format!("Host: {host}")]);
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
    parse_curl_include(&output.stdout, backend_base)
}

fn parse_curl_include(raw: &[u8], backend_base: &str) -> Result<ProxiedHttp, String> {
    let (header_block, body) = split_headers_body_bytes(raw)
        .ok_or_else(|| "Malformed proxy response from WebAdmin backend".to_string())?;
    let header_text = String::from_utf8_lossy(header_block);
    let mut lines = header_text.lines();
    let status_line = lines
        .next()
        .ok_or_else(|| "Missing HTTP status from WebAdmin backend".to_string())?;
    let status = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(502);
    let mut out_headers = Vec::new();
    let mut content_type = None;
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
            if lower == "content-type" {
                content_type = Some(value.to_string());
            }
            let value = if lower == "location" {
                rewrite_location(value, backend_base)
            } else if lower == "set-cookie" {
                rewrite_set_cookie(value)
            } else {
                value.to_string()
            };
            out_headers.push((name, value));
        }
    }
    let body = maybe_rewrite_body(body, content_type.as_deref(), backend_base);
    Ok(ProxiedHttp {
        status,
        headers: out_headers,
        body,
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

fn is_under_mount(path: &str) -> bool {
    path == WEBADMIN_PANEL_MOUNT || path.starts_with(&format!("{WEBADMIN_PANEL_MOUNT}/"))
}

/// Every `Location` coming from WebAdmin is relative to the WebAdmin root, so a root-relative
/// value such as `/login.php` (or anything that merely starts with `/login`) must be moved under
/// the `/ols` mount. Panel routes like `/login` are never valid WebAdmin redirect targets.
fn rewrite_location(value: &str, backend_base: &str) -> String {
    let base = backend_base.trim_end_matches('/');
    if !base.is_empty() && value.starts_with(base) {
        let rest = value.strip_prefix(base).unwrap_or("");
        return mount_path(rest);
    }
    for loopback in [
        "https://127.0.0.1:7080",
        "http://127.0.0.1:7080",
        "https://localhost:7080",
        "http://localhost:7080",
        "https://[::1]:7080",
        "http://[::1]:7080",
    ] {
        if let Some(rest) = value.strip_prefix(loopback) {
            return mount_path(rest);
        }
    }
    if value.starts_with('/') && !value.starts_with("//") {
        return mount_path(value);
    }
    value.to_string()
}

/// Prefix a WebAdmin root-relative path with the panel mount (idempotent).
fn mount_path(rest: &str) -> String {
    if rest.is_empty() {
        return format!("{WEBADMIN_PANEL_MOUNT}/");
    }
    if is_under_mount(rest) {
        return rest.to_string();
    }
    if rest.starts_with('/') {
        return format!("{WEBADMIN_PANEL_MOUNT}{rest}");
    }
    // `?query` or `#frag` after a bare origin
    format!("{WEBADMIN_PANEL_MOUNT}/{rest}")
}

fn rewrite_set_cookie(value: &str) -> String {
    let mut parts = Vec::new();
    let mut saw_path = false;
    for part in value.split(';') {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }
        let lower = trimmed.to_ascii_lowercase();
        if lower.starts_with("path=") {
            parts.push(format!("Path={WEBADMIN_PANEL_MOUNT}"));
            saw_path = true;
        } else if lower.starts_with("domain=") {
            continue;
        } else {
            parts.push(trimmed.to_string());
        }
    }
    if !saw_path {
        parts.push(format!("Path={WEBADMIN_PANEL_MOUNT}"));
    }
    parts.join("; ")
}

fn maybe_rewrite_body(body: &[u8], content_type: Option<&str>, backend_base: &str) -> Vec<u8> {
    let ct = content_type.unwrap_or("").to_ascii_lowercase();
    let rewriteable = ct.contains("text/html")
        || ct.contains("text/css")
        || ct.contains("javascript")
        || ct.contains("application/json");
    if !rewriteable {
        return body.to_vec();
    }
    let text = String::from_utf8_lossy(body);
    let base = backend_base.trim_end_matches('/');
    let mut out = text.into_owned();
    if !base.is_empty() {
        out = out.replace(base, WEBADMIN_PANEL_MOUNT);
    }
    out = out.replace("https://127.0.0.1:7080", WEBADMIN_PANEL_MOUNT);
    out = out.replace("http://127.0.0.1:7080", WEBADMIN_PANEL_MOUNT);
    if ct.contains("text/html") {
        out = prefix_html_root_urls(&out);
    } else if ct.contains("text/css") || ct.contains("javascript") {
        out = prefix_asset_root_urls(&out);
    }
    out.into_bytes()
}

/// Top-level entries that exist in the WebAdmin document root (`admin/html`).
/// Only these are rewritten, so regex literals like `"/g` and server file paths stay intact.
const WEBADMIN_ROOT_ENTRIES: &[&str] = &[
    "res/",
    "view/",
    "lib/",
    "login.php",
    "index.php",
    "favicon.ico",
];

fn replace_all_prefixed(text: &str, lead: &str, entries: &[&str]) -> String {
    let mut out = text.to_string();
    for entry in entries {
        let from = format!("{lead}/{entry}");
        let to = format!("{lead}{WEBADMIN_PANEL_MOUNT}/{entry}");
        out = out.replace(&from, &to);
    }
    out
}

/// HTML: only URL-bearing attributes and `location`/`href` assignments (not form values).
fn prefix_html_root_urls(text: &str) -> String {
    let mut out = text.to_string();
    for attr in [
        "href=",
        "src=",
        "action=",
        "formaction=",
        "data-src=",
        "data-href=",
        "data-url=",
        ".href=",
        ".href =",
        "location=",
        "location =",
    ] {
        for quote in ['"', '\''] {
            let lead = format!("{attr}{quote}");
            out = replace_all_prefixed(&out, &lead, WEBADMIN_ROOT_ENTRIES);
        }
        // `attr = "..."` with spaces around `=` in inline scripts
        for quote in ['"', '\''] {
            let lead = format!("{attr} {quote}");
            out = replace_all_prefixed(&out, &lead, WEBADMIN_ROOT_ENTRIES);
        }
    }
    // Inline <style> blocks and style="" attributes.
    for lead in ["url(", "url(\"", "url('"] {
        out = replace_all_prefixed(&out, lead, WEBADMIN_ROOT_ENTRIES);
    }
    out
}

/// CSS/JS assets: quoted strings, template literals and `url(...)` references.
fn prefix_asset_root_urls(text: &str) -> String {
    let mut out = text.to_string();
    for lead in ["\"", "'", "`", "(", "url(\"", "url('", "url( "] {
        out = replace_all_prefixed(&out, lead, WEBADMIN_ROOT_ENTRIES);
    }
    out
}

pub fn allowed_proxy_method(method: &Method) -> bool {
    matches!(
        *method,
        Method::GET | Method::HEAD | Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    )
}

/// Panel mount path operators should open in the browser (not loopback :7080).
pub fn webadmin_browser_url() -> String {
    let base = crate::panel_network::public_base_url(
        crate::panel_network::preferred_listen_port_or_default(),
        None,
    );
    format!("{}{}", base.trim_end_matches('/'), WEBADMIN_PANEL_MOUNT)
}

pub fn request_uses_https(req: &HttpRequest) -> bool {
    request_https_from_headers(req) || req.connection_info().scheme() == "https"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::litespeed_stack::WEBADMIN_PANEL_MOUNT;

    #[test]
    fn mount_paths_match() {
        assert!(path_should_proxy_ols_webadmin("/ols"));
        assert!(path_should_proxy_ols_webadmin("/ols/"));
        assert!(path_should_proxy_ols_webadmin("/ols/login.php"));
        assert!(!path_should_proxy_ols_webadmin("/server/openlitespeed"));
    }

    #[test]
    fn location_rewrite_maps_backend() {
        assert_eq!(
            rewrite_location("https://127.0.0.1:7080/login.php", "https://127.0.0.1:7080"),
            format!("{WEBADMIN_PANEL_MOUNT}/login.php")
        );
        assert_eq!(
            rewrite_location("/login.php", "https://127.0.0.1:7080"),
            "/ols/login.php"
        );
        assert_eq!(
            rewrite_location("/login.php?timedout=1", "https://127.0.0.1:7080"),
            "/ols/login.php?timedout=1"
        );
        assert_eq!(
            rewrite_location("/ols/login.php", "https://127.0.0.1:7080"),
            "/ols/login.php"
        );
        assert_eq!(
            rewrite_location("login.php", "https://127.0.0.1:7080"),
            "login.php"
        );
        assert_eq!(
            rewrite_location("https://example.com/x", "https://127.0.0.1:7080"),
            "https://example.com/x"
        );
    }

    #[test]
    fn html_root_urls_prefixed() {
        let html = r#"<link href="/res/css/a.css?v=1"><script src='/res/js/a.js'></script><img src="/res/img/x.svg"><form action="login.php"><input value="/lib/keep">"#;
        let out = maybe_rewrite_body(html.as_bytes(), Some("text/html"), "https://127.0.0.1:7080");
        let out = String::from_utf8(out).unwrap();
        assert!(out.contains(r#"href="/ols/res/css/a.css?v=1""#));
        assert!(out.contains("src='/ols/res/js/a.js'"));
        assert!(out.contains(r#"src="/ols/res/img/x.svg""#));
        assert!(out.contains(r#"action="login.php""#));
        assert!(out.contains(r#"value="/lib/keep""#));
        let again = maybe_rewrite_body(out.as_bytes(), Some("text/html"), "https://127.0.0.1:7080");
        assert_eq!(String::from_utf8(again).unwrap(), out);
    }

    #[test]
    fn js_and_css_root_urls_prefixed() {
        let js =
            br#"a.replace(/"/g,"");location.href="/login.php?timedout=1";load("/res/css/x.css");"#;
        let out = String::from_utf8(maybe_rewrite_body(
            js,
            Some("text/javascript"),
            "https://127.0.0.1:7080",
        ))
        .unwrap();
        assert!(out.contains(r#"/"/g"#));
        assert!(out.contains(r#""/ols/login.php?timedout=1""#));
        assert!(out.contains(r#""/ols/res/css/x.css""#));
        let css = b"body{background:url(/res/img/a.png)} @import \"/res/css/b.css\";";
        let out = String::from_utf8(maybe_rewrite_body(
            css,
            Some("text/css"),
            "https://127.0.0.1:7080",
        ))
        .unwrap();
        assert!(out.contains("url(/ols/res/img/a.png)"));
        assert!(out.contains("\"/ols/res/css/b.css\""));
    }

    #[test]
    fn request_headers_drop_client_ip_and_panel_cookies() {
        let pairs = vec![
            ("X-Forwarded-For".to_string(), "51.174.191.240".to_string()),
            ("X-Real-IP".to_string(), "51.174.191.240".to_string()),
            ("Forwarded".to_string(), "for=51.174.191.240".to_string()),
            ("Host".to_string(), "panel.newstargeted.com".to_string()),
            (
                "Cookie".to_string(),
                "cpn_panel_session=secret; LSUI1=abc; cpn_panel_session_2087=x".to_string(),
            ),
            (
                "Origin".to_string(),
                "https://panel.newstargeted.com".to_string(),
            ),
            (
                "Referer".to_string(),
                "https://panel.newstargeted.com/ols/index.php?view=x".to_string(),
            ),
            ("Accept".to_string(), "text/html".to_string()),
        ];
        let out = proxy_request_headers_from(&pairs, "https://127.0.0.1:7080");
        let get = |n: &str| {
            out.iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(n))
                .map(|(_, v)| v.clone())
        };
        assert!(get("x-forwarded-for").is_none());
        assert!(get("x-real-ip").is_none());
        assert!(get("forwarded").is_none());
        assert!(get("host").is_none());
        assert_eq!(get("cookie").as_deref(), Some("LSUI1=abc"));
        assert_eq!(get("origin").as_deref(), Some("https://127.0.0.1:7080"));
        assert_eq!(
            get("referer").as_deref(),
            Some("https://127.0.0.1:7080/index.php?view=x")
        );
        assert_eq!(get("accept").as_deref(), Some("text/html"));
    }

    #[test]
    fn cookie_path_scoped_to_mount() {
        let rewritten = rewrite_set_cookie("LSWSWEBUI=abc; path=/; HttpOnly");
        assert!(rewritten.contains("Path=/ols"));
        assert!(!rewritten.contains("domain="));
    }
}
