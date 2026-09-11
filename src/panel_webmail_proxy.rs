//! Reverse-proxy webmail from the panel mount path to loopback PHP-FPM HTTP (:8080).

use crate::panel_feature_gate::webmail_installed;
use crate::panel_webmail::{load_webmail_config, strip_webmail_mount, webmail_ready};
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
    let path = req.path();
    let Some(backend_path) = strip_webmail_mount(path) else {
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

fn forward_http(
    method: &str,
    target: &str,
    req: &HttpRequest,
    body: &[u8],
) -> Result<HttpResponse, String> {
    // Use curl for portable proxying without adding an HTTP client crate.
    // Enough for SnappyMail/Roundcube HTML, assets, and form posts in lab/production.
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
        ) {
            continue;
        }
        if let Ok(v) = value.to_str() {
            cmd.args(["-H", &format!("{}: {}", name.as_str(), v)]);
        }
    }
    cmd.args(["-H", "Host: 127.0.0.1:8080"]);
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
    cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
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
        .ok_or_else(|| "Malformed proxy response from webmail backend".to_string())?;
    let mut lines = header_block.lines();
    let status_line = lines
        .next()
        .ok_or_else(|| "Missing HTTP status from webmail backend".to_string())?;
    let status = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(502);
    let status =
        StatusCode::from_u16(status).unwrap_or(StatusCode::BAD_GATEWAY);
    let mut builder = HttpResponse::build(status);
    let mount = load_webmail_config().public_path.trim_end_matches('/').to_string();
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
                builder.insert_header((name, rewrite_location(value, &mount)));
                continue;
            }
            builder.insert_header((name, value));
        }
    }
    Ok(builder.body(body.to_string()))
}

fn split_headers_body(text: &str) -> Option<(&str, &str)> {
    // curl --include may chain 100 Continue; take the last header block.
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
        assert_eq!(
            rewrite_location("http://127.0.0.1:8080/a", "/wm"),
            "/wm/a"
        );
    }

    #[test]
    fn allowed_methods_cover_forms() {
        assert!(allowed_proxy_method(&Method::POST));
        assert!(!allowed_proxy_method(&Method::OPTIONS));
    }
}
