//! Shared HTTP helpers for hub feature routes.

use crate::auth_api::panel_user_from_request;
use crate::installer::AppState;
use actix_web::{HttpRequest, HttpResponse};

pub use crate::login_next::login_redirect;

const FLASH_COOKIE: &str = "cpn_panel_flash";

pub fn require_panel_user(state: &AppState, http: &HttpRequest) -> Option<String> {
    panel_user_from_request(state, http)
}

pub fn html_ok(body: String) -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(body)
}

pub fn urlencoding_simple(value: &str) -> String {
    let mut out = String::with_capacity(value.len() * 3);
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn urldecoding_simple(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = String::with_capacity(value.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                let hex = |c: u8| -> Option<u8> {
                    match c {
                        b'0'..=b'9' => Some(c - b'0'),
                        b'a'..=b'f' => Some(c - b'a' + 10),
                        b'A'..=b'F' => Some(c - b'A' + 10),
                        _ => None,
                    }
                };
                if let (Some(hi), Some(lo)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                    out.push((hi << 4 | lo) as char);
                    i += 3;
                } else {
                    out.push('%');
                    i += 1;
                }
            }
            c => {
                out.push(c as char);
                i += 1;
            }
        }
    }
    out
}

pub fn redirect(path: &str) -> HttpResponse {
    HttpResponse::SeeOther()
        .append_header(("Location", path.to_string()))
        .finish()
}

pub fn redirect_notice(base: &str, notice: Option<&str>, error: Option<&str>) -> HttpResponse {
    let mut loc = base.to_string();
    let mut first = !base.contains('?');
    if let Some(n) = notice {
        loc.push(if first { '?' } else { '&' });
        first = false;
        loc.push_str("notice=");
        loc.push_str(&urlencoding_simple(n));
    }
    if let Some(e) = error {
        loc.push(if first { '?' } else { '&' });
        loc.push_str("error=");
        loc.push_str(&urlencoding_simple(e));
    }
    redirect(&loc)
}

/// PRG redirect that stores the notice/error in an HttpOnly flash cookie (clean URL).
pub fn redirect_flash(path: &str, notice: Option<&str>, error: Option<&str>) -> HttpResponse {
    let payload = if let Some(n) = notice {
        format!("n.{}", urlencoding_simple(n))
    } else if let Some(e) = error {
        format!("e.{}", urlencoding_simple(e))
    } else {
        String::new()
    };
    let mut builder = HttpResponse::SeeOther();
    builder.append_header(("Location", path.to_string()));
    if !payload.is_empty() {
        builder.append_header((
            "Set-Cookie",
            format!("{FLASH_COOKIE}={payload}; Path=/; Max-Age=120; HttpOnly; SameSite=Lax"),
        ));
    }
    builder.finish()
}

fn cookie_has_flash(http: &HttpRequest) -> bool {
    http.headers()
        .get("cookie")
        .and_then(|v| v.to_str().ok())
        .map(|c| {
            c.split(';')
                .any(|p| p.trim().starts_with(&format!("{FLASH_COOKIE}=")))
        })
        .unwrap_or(false)
}

/// Parse flash cookie (preferred) with optional query-string fallback.
pub fn flash_messages(
    http: &HttpRequest,
    query_notice: Option<&str>,
    query_error: Option<&str>,
) -> (Option<String>, Option<String>) {
    let mut notice = query_notice.map(str::to_string);
    let mut error = query_error.map(str::to_string);
    if let Some(cookie_header) = http.headers().get("cookie").and_then(|v| v.to_str().ok()) {
        for part in cookie_header.split(';') {
            let part = part.trim();
            let Some((name, value)) = part.split_once('=') else {
                continue;
            };
            if name != FLASH_COOKIE {
                continue;
            }
            if let Some(rest) = value.strip_prefix("n.") {
                notice = Some(urldecoding_simple(rest));
            } else if let Some(rest) = value.strip_prefix("e.") {
                error = Some(urldecoding_simple(rest));
            }
        }
    }
    (notice, error)
}

/// HTML 200 that clears a consumed flash cookie when present.
pub fn html_ok_pop_flash(http: &HttpRequest, body: String) -> HttpResponse {
    let mut builder = HttpResponse::Ok();
    builder.content_type("text/html; charset=utf-8");
    if cookie_has_flash(http) {
        builder.append_header((
            "Set-Cookie",
            format!("{FLASH_COOKIE}=; Path=/; Max-Age=0; HttpOnly; SameSite=Lax"),
        ));
    }
    builder.body(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flash_roundtrip_encoding() {
        let raw = "Password updated for `a@b.c`. Local mailbox ready.";
        let enc = urlencoding_simple(raw);
        assert_eq!(urldecoding_simple(&enc), raw);
    }
}
