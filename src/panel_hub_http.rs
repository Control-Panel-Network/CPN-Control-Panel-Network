//! Shared HTTP helpers for hub feature routes.

use crate::auth_api::panel_user_from_request;
use crate::installer::AppState;
use actix_web::{HttpRequest, HttpResponse};

pub fn require_panel_user(state: &AppState, http: &HttpRequest) -> Option<String> {
    panel_user_from_request(state, http)
}

pub fn html_ok(body: String) -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(body)
}

pub fn login_redirect() -> HttpResponse {
    HttpResponse::SeeOther()
        .append_header(("Location", "/login"))
        .finish()
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
