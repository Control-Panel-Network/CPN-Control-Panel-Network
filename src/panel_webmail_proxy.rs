//! Reverse-proxy webmail from the panel mount path to loopback PHP-FPM HTTP (:8080).

use crate::panel_feature_gate::webmail_installed;
use crate::panel_session::request_https_from_headers;
use crate::panel_webmail::{backend_path_for_webmail_proxy, load_webmail_config, webmail_ready};
use actix_web::{HttpRequest, HttpResponse, http::Method, http::StatusCode, web};
use futures_util::StreamExt;
use std::process::{Command, Stdio};

const BACKEND: &str = "http://127.0.0.1:8080";

/// Catch-all proxy for the configured webmail public path (and nested assets).
pub async fn webmail_panel_proxy(req: HttpRequest, payload: web::Payload) -> HttpResponse {
    if !webmail_ready() || !webmail_installed() {
        return HttpResponse::NotFound()
            .content_type("text/plain; charset=utf-8")
            .body("Webmail is not installed on this host.");
    }
    // Best-effort: keep loopback docroot + PATH_INFO in sync with the preferred client.
    // Run off the request path: SELinux/sieve heal can block for tens of seconds and
    // would otherwise stall every /tachyon|/snappymail proxy hit (and panel workers)
    // behind Once::call_once, which shows up as browser ERR_CONNECTION_RESET / timeout.
    static HEAL_ONCE: std::sync::Once = std::sync::Once::new();
    HEAL_ONCE.call_once(|| {
        std::thread::spawn(|| {
            let _ = crate::install_webmail_runtime::heal_webmail_loopback_config();
        });
    });
    let path = req.path();
    let Some(backend_path) = backend_path_for_webmail_proxy(path) else {
        return HttpResponse::NotFound().finish();
    };
    // Normalize bare mount to index.php so SnappyMail serves the login UI.
    let backend_path = if backend_path == "/" {
        "/index.php".to_string()
    } else {
        backend_path
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
                "Webmail proxy could not reach {BACKEND} ({err}). Is PHP-FPM webmail listening on 127.0.0.1:8080?"
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

fn client_scheme(req: &HttpRequest) -> &'static str {
    if request_https_from_headers(req) || req.app_config().secure() {
        "https"
    } else {
        "http"
    }
}

fn short_header<'a>(req: &'a HttpRequest, name: &str) -> Option<&'a str> {
    let value = req.headers().get(name)?.to_str().ok()?;
    if value.is_empty() || value.len() > 253 {
        return None;
    }
    if value.bytes().any(|b| {
        !(b.is_ascii_alphanumeric()
            || matches!(b, b'.' | b'-' | b':' | b'[' | b']' | b'_' | b',' | b' '))
    }) {
        return None;
    }
    Some(value)
}

fn forward_http(
    method: &str,
    target: &str,
    req: &HttpRequest,
    body: &[u8],
) -> Result<HttpResponse, String> {
    // Use curl for portable proxying without adding an HTTP client crate.
    // Enough for SnappyMail/Roundcube HTML, assets, and form posts in lab/production.
    let mut cmd = Command::new("curl");
    // Admin Json package/core checks must not hang the panel worker when the
    // upstream SnappyMail repository is unreachable (browser aborts ~30s).
    let max_time = if target.contains("admin/Json") || target.contains("?admin") {
        "35"
    } else {
        "120"
    };
    cmd.args([
        "--silent",
        "--show-error",
        "--include",
        "--max-time",
        max_time,
        "-X",
        method,
        "--path-as-is",
    ]);
    // Hop-by-hop and host headers are rebuilt for the loopback backend.
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
                | "x-forwarded-proto"
                | "x-forwarded-host"
                | "x-forwarded-for"
                | "x-forwarded-prefix"
        ) {
            continue;
        }
        if let Ok(v) = value.to_str() {
            cmd.args(["-H", &format!("{}: {}", name.as_str(), v)]);
        }
    }
    let scheme = client_scheme(req);
    let host = short_header(req, "X-Forwarded-Host")
        .or_else(|| short_header(req, "Host"))
        .unwrap_or("127.0.0.1");
    let mount = load_webmail_config()
        .public_path
        .trim_end_matches('/')
        .to_string();
    // Always tell Tachyon/SnappyMail the browser scheme. Sending https on plain
    // http://IP:2087 marks smtoken Secure and browsers drop it → HTTP Token mismatch.
    cmd.args(["-H", "Host: 127.0.0.1:8080"]);
    cmd.args(["-H", "Accept-Encoding: identity"]);
    cmd.args(["-H", &format!("X-Forwarded-Proto: {scheme}")]);
    cmd.args(["-H", &format!("X-Forwarded-Host: {host}")]);
    if !mount.is_empty() {
        cmd.args(["-H", &format!("X-Forwarded-Prefix: {mount}")]);
    }
    if let Some(peer) = req.peer_addr() {
        cmd.args(["-H", &format!("X-Forwarded-For: {}", peer.ip())]);
    }
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
    parse_curl_include(&output.stdout, &mount, scheme == "https")
}

fn parse_curl_include(raw: &[u8], mount: &str, client_https: bool) -> Result<HttpResponse, String> {
    // Binary-safe: never decode the body as UTF-8 (WOFF/JS would corrupt).
    let (header_block, body) = split_headers_body_bytes(raw)
        .ok_or_else(|| "Malformed proxy response from webmail backend".to_string())?;
    let header_text = String::from_utf8_lossy(header_block);
    let mut lines = header_text.lines();
    let status_line = lines
        .next()
        .ok_or_else(|| "Missing HTTP status from webmail backend".to_string())?;
    let status = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(502);
    let status = StatusCode::from_u16(status).unwrap_or(StatusCode::BAD_GATEWAY);
    let mut builder = HttpResponse::build(status);
    let mut content_type = String::new();
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
            if lower == "content-type" {
                content_type = value.to_ascii_lowercase();
            }
            if lower == "location" {
                builder.insert_header((name, rewrite_location(value, mount)));
                continue;
            }
            // Multiple Set-Cookie headers must not collapse via insert_header.
            if lower == "set-cookie" {
                builder.append_header((name, rewrite_set_cookie(value, mount, client_https)));
                continue;
            }
            builder.insert_header((name, value));
        }
    }
    // Expire legacy Path=/ smtoken cookies from CyberPanel SnappyMail so they cannot
    // collide with mount-scoped tokens (HTTP Token mismatch on login).
    if !mount.is_empty() {
        for clear in clear_legacy_root_webmail_cookies(client_https) {
            builder.append_header(("Set-Cookie", clear));
        }
    }
    let body = rewrite_json_web_path(body, &content_type, mount);
    Ok(builder.body(body))
}

fn rewrite_set_cookie(value: &str, mount: &str, client_https: bool) -> String {
    let mount = if mount.is_empty() { "/" } else { mount };
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
            parts.push(format!("Path={mount}"));
            saw_path = true;
        } else if lower == "secure" {
            // Drop backend Secure; re-add only when the browser used HTTPS.
            continue;
        } else if lower.starts_with("samesite=") {
            parts.push("SameSite=Lax".into());
            saw_samesite = true;
        } else {
            parts.push(trimmed.to_string());
        }
    }
    if !saw_path {
        parts.push(format!("Path={mount}"));
    }
    if !saw_samesite {
        parts.push("SameSite=Lax".into());
    }
    if client_https {
        parts.push("Secure".into());
    }
    parts.join("; ")
}

fn clear_legacy_root_webmail_cookies(client_https: bool) -> Vec<String> {
    let secure = if client_https { "; Secure" } else { "" };
    ["smtoken", "smsession"]
        .iter()
        .map(|name| {
            format!("{name}=; Path=/; Max-Age=0; HttpOnly; SameSite=Lax{secure}")
        })
        .collect()
}

/// Tachyon AppData reports webPath `/` because the loopback docroot is at `/`.
/// Rewrite to the panel mount so the SPA posts Json to `/tachyon/?/Json/` not `/?/Json/`.
fn rewrite_json_web_path(body: &[u8], content_type: &str, mount: &str) -> Vec<u8> {
    if mount.is_empty() || mount == "/" {
        return body.to_vec();
    }
    if !content_type.contains("json") {
        return body.to_vec();
    }
    let Ok(text) = std::str::from_utf8(body) else {
        return body.to_vec();
    };
    if !text.contains("\"webPath\"") {
        return body.to_vec();
    }
    let mount_slash = format!("{}/", mount.trim_end_matches('/'));
    // PHP json_encode often escapes slashes as \/
    let mount_escaped = mount_slash.replace('/', "\\/");
    let updated = text
        .replace("\"webPath\":\"/\"", &format!("\"webPath\":\"{mount_slash}\""))
        .replace("\"webPath\": \"/\"", &format!("\"webPath\": \"{mount_slash}\""))
        .replace("\"webPath\":\"\\/\"", &format!("\"webPath\":\"{mount_escaped}\""))
        .replace("\"webPath\": \"\\/\"", &format!("\"webPath\": \"{mount_escaped}\""));
    updated.into_bytes()
}

fn split_headers_body_bytes(raw: &[u8]) -> Option<(&[u8], &[u8])> {
    // curl --include may chain 100 Continue; take the last header block.
    let mut rest = raw;
    loop {
        let sep = find_header_sep(rest)?;
        let headers = &rest[..sep.0];
        let body = &rest[sep.1..];
        let header_text = String::from_utf8_lossy(headers);
        if header_text.contains(" 100 ") || header_text.starts_with("HTTP/1.1 100") {
            rest = body;
            continue;
        }
        return Some((headers, body));
    }
}

fn find_header_sep(raw: &[u8]) -> Option<(usize, usize)> {
    if let Some(idx) = find_subslice(raw, b"\r\n\r\n") {
        return Some((idx, idx + 4));
    }
    if let Some(idx) = find_subslice(raw, b"\n\n") {
        return Some((idx, idx + 2));
    }
    None
}

fn find_subslice(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

fn rewrite_location(value: &str, mount: &str) -> String {
    if value.starts_with("http://127.0.0.1:8080") {
        return value.replacen("http://127.0.0.1:8080", mount, 1);
    }
    if value.starts_with('/') && !value.starts_with(mount) {
        return format!("{mount}{value}");
    }
    value.to_string()
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
    fn location_rewrite_prefixes_mount() {
        assert_eq!(
            rewrite_location("/index.php", "/snappymail"),
            "/snappymail/index.php"
        );
        assert_eq!(rewrite_location("http://127.0.0.1:8080/a", "/wm"), "/wm/a");
    }

    #[test]
    fn allowed_methods_cover_forms() {
        assert!(allowed_proxy_method(&Method::POST));
        assert!(!allowed_proxy_method(&Method::OPTIONS));
    }

    #[test]
    fn binary_body_survives_header_split() {
        let mut raw = b"HTTP/1.1 200 OK\r\nContent-Type: font/woff2\r\n\r\n".to_vec();
        raw.extend_from_slice(&[0xff, 0x00, 0xfe, 0x01]);
        let (headers, body) = split_headers_body_bytes(&raw).expect("split");
        assert!(String::from_utf8_lossy(headers).contains("200"));
        assert_eq!(body, &[0xff, 0x00, 0xfe, 0x01]);
    }

    #[test]
    fn cookie_rewrite_scopes_path_and_strips_secure_on_http() {
        let rewritten = rewrite_set_cookie(
            "smtoken=abc; path=/; secure; HttpOnly; SameSite=Strict",
            "/tachyon",
            false,
        );
        assert!(rewritten.contains("Path=/tachyon"));
        assert!(rewritten.contains("SameSite=Lax"));
        assert!(!rewritten.to_ascii_lowercase().contains("secure"));
        let https = rewrite_set_cookie("smtoken=abc; path=/", "/tachyon", true);
        assert!(https.to_ascii_lowercase().contains("secure"));
        assert!(https.contains("Path=/tachyon"));
    }

    #[test]
    fn json_webpath_rewritten_for_mount() {
        let body = br#"{"System":{"webPath":"/","webVersionPath":"/tachyon/v/4.2.3/"}}"#;
        let out = rewrite_json_web_path(body, "application/json", "/tachyon");
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("\"webPath\":\"/tachyon/\""));
        assert!(text.contains("\"webVersionPath\":\"/tachyon/v/4.2.3/\""));
    }

    #[test]
    fn json_webpath_rewritten_when_php_escapes_slashes() {
        let body = br#"{"System":{"webPath":"\/","webVersionPath":"\/tachyon\/v\/4.2.3\/"}}"#;
        let out = rewrite_json_web_path(body, "application/json; charset=utf-8", "/tachyon");
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("\"webPath\":\"\\/tachyon\\/\"") || text.contains("\"webPath\":\"/tachyon/\""));
        assert!(text.contains("webVersionPath"));
    }
}
