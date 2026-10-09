//! Reverse-proxy SOGo (webmail + CalDAV/CardDAV) under the panel port.
//!
//! * `/SOGo` and `/SOGo/...` forward to the loopback `sogod` listener with the
//!   `x-webobjects-*` headers SOGo needs to build absolute URLs behind a proxy.
//! * `/SOGo.woa/WebServerResources/...`, `/SOGo/WebServerResources/...`, and product
//!   `Resources` are served straight from the package tree with real MIME types.
//! * `/.well-known/caldav` and `/.well-known/carddav` redirect to `/SOGo/dav/`.
//!
//! Mailbox users are not panel users, so no panel session is required (same as webmail).

use crate::apps_sogo_config::SOGO_LOOPBACK;
use actix_web::{HttpRequest, HttpResponse, http::StatusCode, web};
use futures_util::StreamExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub const SOGO_MOUNT: &str = "/SOGo";
const RESOURCE_PREFIXES: &[&str] = &["/SOGo.woa/WebServerResources/", "/SOGo/WebServerResources/"];
const MAX_BODY: usize = 64 * 1024 * 1024;

/// True when the panel catch-all should hand this path to the SOGo proxy.
pub fn path_should_proxy_sogo(req_path: &str) -> bool {
    if !crate::apps_sogo::sogo_installed() {
        return false;
    }
    req_path == SOGO_MOUNT
        || req_path.starts_with("/SOGo/")
        || req_path.starts_with("/SOGo.woa/")
        || req_path == "/.well-known/caldav"
        || req_path == "/.well-known/carddav"
}

fn safe_join(base: &str, rel: &str) -> Option<PathBuf> {
    if rel.is_empty() || rel.contains("..") || rel.contains('\0') || rel.starts_with('/') {
        return None;
    }
    let joined = Path::new(base).join(rel);
    let canon = joined.canonicalize().ok()?;
    let base_canon = Path::new(base).canonicalize().ok()?;
    if canon.starts_with(&base_canon) && canon.is_file() {
        Some(canon)
    } else {
        None
    }
}

fn mime_for(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("js") | Some("mjs") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("html") | Some("htm") => "text/html; charset=utf-8",
        Some("json") | Some("map") => "application/json; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("ico") => "image/x-icon",
        Some("webp") => "image/webp",
        Some("woff2") => "font/woff2",
        Some("woff") => "font/woff",
        Some("ttf") => "font/ttf",
        Some("eot") => "application/vnd.ms-fontobject",
        Some("mp3") => "audio/mpeg",
        Some("ogg") => "audio/ogg",
        Some("wav") => "audio/wav",
        Some("txt") => "text/plain; charset=utf-8",
        Some("xml") => "application/xml; charset=utf-8",
        _ => "application/octet-stream",
    }
}

/// Resolve a static SOGo asset on disk for the request path (None = forward to sogod).
fn static_file_for(req_path: &str) -> Option<PathBuf> {
    let root = crate::apps_sogo_repo::web_resources_dir()?;
    for prefix in RESOURCE_PREFIXES {
        if let Some(rest) = req_path.strip_prefix(prefix) {
            return safe_join(root, rest);
        }
    }
    // /SOGo/so/ControlPanel/Products/<Product>/Resources/<file>
    let rest = req_path.strip_prefix("/SOGo/so/ControlPanel/Products/")?;
    let (product, file) = rest.split_once("/Resources/")?;
    if product.is_empty()
        || !product
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return None;
    }
    let bundle = format!(
        "{}/{product}.SOGo/Resources",
        crate::apps_sogo_repo::gnustep_sogo_dir()?
    );
    safe_join(&bundle, file)
}

fn serve_static(path: &Path) -> HttpResponse {
    match std::fs::read(path) {
        Ok(bytes) => HttpResponse::Ok()
            .content_type(mime_for(path))
            .insert_header(("Cache-Control", "public, max-age=604800, immutable"))
            .insert_header(("X-Content-Type-Options", "nosniff"))
            .body(bytes),
        Err(_) => HttpResponse::NotFound().finish(),
    }
}

/// Catch-all handler for SOGo paths.
pub async fn sogo_panel_proxy(req: HttpRequest, payload: web::Payload) -> HttpResponse {
    if !crate::apps_sogo::sogo_installed() {
        return HttpResponse::NotFound()
            .content_type("text/plain; charset=utf-8")
            .body("SOGo is not installed on this host.");
    }
    let path = req.path();
    if path == "/.well-known/caldav" || path == "/.well-known/carddav" {
        return HttpResponse::MovedPermanently()
            .insert_header(("Location", "/SOGo/dav/"))
            .finish();
    }
    if matches!(
        *req.method(),
        actix_web::http::Method::GET | actix_web::http::Method::HEAD
    ) && let Some(file) = static_file_for(path)
    {
        return serve_static(&file);
    }
    static HEAL_ONCE: std::sync::Once = std::sync::Once::new();
    HEAL_ONCE.call_once(|| {
        std::thread::spawn(crate::apps_sogo::heal_sogo_users);
    });
    if !crate::service_detect::port_open(SOGO_LOOPBACK, 400) {
        return HttpResponse::BadGateway()
            .content_type("text/plain; charset=utf-8")
            .body(format!(
                "SOGo is installed but sogod is not answering on {SOGO_LOOPBACK}. Use Start on the SOGo host package (Plugins > Installed > Host) or check: journalctl -u {} -n 40.",
                crate::apps_sogo::sogo_unit()
            ));
    }
    let backend_path = if path == SOGO_MOUNT { "/SOGo/" } else { path };
    let query = req.query_string();
    let target = if query.is_empty() {
        format!("http://{SOGO_LOOPBACK}{backend_path}")
    } else {
        format!("http://{SOGO_LOOPBACK}{backend_path}?{query}")
    };
    let body = match collect_body(payload).await {
        Ok(b) => b,
        Err(err) => {
            return HttpResponse::BadRequest()
                .content_type("text/plain; charset=utf-8")
                .body(format!("Could not read request body: {err}"));
        }
    };
    let (scheme, host) = public_scheme_host(&req);
    match forward(&req, &target, &body, &scheme, &host) {
        Ok(resp) => resp,
        Err(err) => HttpResponse::BadGateway()
            .content_type("text/plain; charset=utf-8")
            .body(format!(
                "SOGo proxy could not reach {SOGO_LOOPBACK} ({err})"
            )),
    }
}

async fn collect_body(mut payload: web::Payload) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    while let Some(chunk) = payload.next().await {
        let chunk = chunk.map_err(|e| e.to_string())?;
        if bytes.len() + chunk.len() > MAX_BODY {
            return Err("Request body too large".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

/// Public scheme + host[:port] as the browser sees the panel (for x-webobjects-server-url).
fn public_scheme_host(req: &HttpRequest) -> (String, String) {
    let info = req.connection_info();
    let scheme = info.scheme().to_string();
    let host = info.host().to_string();
    (scheme, host)
}

fn forward(
    req: &HttpRequest,
    target: &str,
    body: &[u8],
    scheme: &str,
    host: &str,
) -> Result<HttpResponse, String> {
    let method = req.method().as_str().to_ascii_uppercase();
    let mut cmd = Command::new("curl");
    cmd.args([
        "--silent",
        "--show-error",
        "--include",
        "--max-time",
        "180",
        "--path-as-is",
    ]);
    if method == "HEAD" {
        cmd.arg("--head");
    } else {
        cmd.args(["-X", &method]);
    }
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
                | "expect"
        ) || lower.starts_with("x-webobjects-")
        {
            continue;
        }
        if let Ok(v) = value.to_str() {
            cmd.args(["-H", &format!("{}: {}", name.as_str(), v)]);
        }
    }
    let (host_only, port) = split_host_port(host, scheme);
    let remote = req
        .connection_info()
        .realip_remote_addr()
        .map(|s| s.split(':').next().unwrap_or(s).to_string())
        .unwrap_or_else(|| "127.0.0.1".into());
    cmd.args(["-H", &format!("Host: {host}")]);
    cmd.args(["-H", "Accept-Encoding: identity"]);
    cmd.args(["-H", "x-webobjects-server-protocol: HTTP/1.0"]);
    cmd.args(["-H", &format!("x-webobjects-remote-host: {remote}")]);
    cmd.args(["-H", &format!("x-webobjects-server-name: {host_only}")]);
    cmd.args(["-H", &format!("x-webobjects-server-url: {scheme}://{host}")]);
    cmd.args(["-H", &format!("x-webobjects-server-port: {port}")]);
    cmd.args(["-H", &format!("X-Forwarded-Proto: {scheme}")]);
    cmd.args(["-H", &format!("X-Forwarded-For: {remote}")]);
    if !body.is_empty() {
        cmd.args(["--data-binary", "@-"]);
    }
    cmd.arg(target);
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("curl missing or failed to start: {e}"))?;
    if !body.is_empty() {
        use std::io::Write;
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(body)
                .map_err(|e| format!("Could not write proxy body: {e}"))?;
        }
    } else {
        drop(child.stdin.take());
    }
    let output = child
        .wait_with_output()
        .map_err(|e| format!("curl wait failed: {e}"))?;
    if !output.status.success() && output.stdout.is_empty() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(err.trim().chars().take(240).collect());
    }
    parse_response(&output.stdout)
}

fn split_host_port(host: &str, scheme: &str) -> (String, String) {
    if let Some(rest) = host.strip_prefix('[')
        && let Some((v6, tail)) = rest.split_once(']')
    {
        let port = tail.strip_prefix(':').unwrap_or("");
        return (format!("[{v6}]"), default_port(port, scheme));
    }
    match host.rsplit_once(':') {
        Some((h, p)) if p.chars().all(|c| c.is_ascii_digit()) && !p.is_empty() => {
            (h.to_string(), p.to_string())
        }
        _ => (host.to_string(), default_port("", scheme)),
    }
}

fn default_port(port: &str, scheme: &str) -> String {
    if !port.is_empty() {
        port.to_string()
    } else if scheme.eq_ignore_ascii_case("https") {
        "443".into()
    } else {
        "80".into()
    }
}

fn parse_response(raw: &[u8]) -> Result<HttpResponse, String> {
    let (header_block, body) =
        split_headers_body(raw).ok_or_else(|| "Malformed response from sogod".to_string())?;
    let header_text = String::from_utf8_lossy(header_block);
    let mut lines = header_text.lines();
    let status_line = lines
        .next()
        .ok_or_else(|| "Missing HTTP status from sogod".to_string())?;
    let status = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(502);
    let status = StatusCode::from_u16(status).unwrap_or(StatusCode::BAD_GATEWAY);
    let mut builder = HttpResponse::build(status);
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
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
            builder.append_header((name, value));
            continue;
        }
        builder.insert_header((name, value));
    }
    Ok(builder.body(body.to_vec()))
}

fn rewrite_location(value: &str) -> String {
    let loopback = format!("http://{SOGO_LOOPBACK}");
    if let Some(rest) = value.strip_prefix(&loopback) {
        return if rest.is_empty() {
            "/".into()
        } else {
            rest.to_string()
        };
    }
    value.to_string()
}

fn split_headers_body(raw: &[u8]) -> Option<(&[u8], &[u8])> {
    let mut rest = raw;
    loop {
        let (at, len) = find_sep(rest)?;
        let headers = &rest[..at];
        let body = &rest[at + len..];
        let text = String::from_utf8_lossy(headers);
        if text.starts_with("HTTP/1.1 100") || text.contains(" 100 Continue") {
            rest = body;
            continue;
        }
        return Some((headers, body));
    }
}

fn find_sep(raw: &[u8]) -> Option<(usize, usize)> {
    raw.windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|i| (i, 4))
        .or_else(|| raw.windows(2).position(|w| w == b"\n\n").map(|i| (i, 2)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_location_is_made_relative() {
        assert_eq!(
            rewrite_location("http://127.0.0.1:20000/SOGo/so/x"),
            "/SOGo/so/x"
        );
        assert_eq!(
            rewrite_location("http://localhost:2087/SOGo/"),
            "http://localhost:2087/SOGo/"
        );
    }

    #[test]
    fn host_port_split_handles_defaults_and_ipv6() {
        assert_eq!(
            split_host_port("localhost:2087", "http"),
            ("localhost".into(), "2087".into())
        );
        assert_eq!(
            split_host_port("panel.example.com", "https"),
            ("panel.example.com".into(), "443".into())
        );
        assert_eq!(
            split_host_port("[::1]:2087", "http"),
            ("[::1]".into(), "2087".into())
        );
    }

    #[test]
    fn mime_types_cover_sogo_assets() {
        assert_eq!(
            mime_for(Path::new("a.js")),
            "text/javascript; charset=utf-8"
        );
        assert_eq!(mime_for(Path::new("a.css")), "text/css; charset=utf-8");
        assert_eq!(mime_for(Path::new("a.woff2")), "font/woff2");
        assert_eq!(mime_for(Path::new("a.bin")), "application/octet-stream");
    }

    #[test]
    fn traversal_is_rejected() {
        assert!(safe_join("/usr", "../etc/passwd").is_none());
        assert!(safe_join("/usr", "/etc/passwd").is_none());
    }

    #[test]
    fn binary_body_survives_split() {
        let mut raw = b"HTTP/1.1 200 OK\r\nContent-Type: font/woff2\r\n\r\n".to_vec();
        raw.extend_from_slice(&[0xff, 0x00, 0xfe]);
        let (h, b) = split_headers_body(&raw).unwrap();
        assert!(String::from_utf8_lossy(h).contains("200"));
        assert_eq!(b, &[0xff, 0x00, 0xfe]);
    }
}
