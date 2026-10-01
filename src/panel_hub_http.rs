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

/// Longest a hub page may spend rendering before the browser gets a clean 503 instead of a
/// hung connection that Actix would eventually answer with `408 Request Timeout`.
pub const HUB_RENDER_BUDGET: std::time::Duration = std::time::Duration::from_secs(20);

/// Render a hub page on the blocking pool so slow host probes (package database, container
/// engine, firewall daemon) cannot occupy an Actix worker, and bound the wait.
pub async fn html_blocking<F>(render: F) -> HttpResponse
where
    F: FnOnce() -> String + Send + 'static,
{
    match tokio::time::timeout(HUB_RENDER_BUDGET, tokio::task::spawn_blocking(render)).await {
        Ok(Ok(body)) => html_ok(body),
        Ok(Err(_)) => HttpResponse::InternalServerError()
            .content_type("text/html; charset=utf-8")
            .body("<!DOCTYPE html><title>Error</title><p>This page could not be rendered. Reload to try again.</p>"),
        Err(_) => HttpResponse::ServiceUnavailable()
            .insert_header(("Retry-After", "5"))
            .content_type("text/html; charset=utf-8")
            .body("<!DOCTYPE html><title>Busy</title><p>The panel is still gathering host status. Reload in a few seconds.</p>"),
    }
}

/// In-page 403 for panel-owner-only pages. Replaces a redirect to the hub so the visitor sees
/// why access was refused and which account is signed in, instead of a page that reloads.
pub fn owner_only_html(username: &str, active: &str, title: &str) -> HttpResponse {
    let who = username
        .chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | '@'))
        .collect::<String>();
    let body = format!(
        r#"<div class="dashboard-heading"><div><p class="eyebrow">CPN PANEL</p><h1>{title}</h1></div></div>
<article class="section-card">
  <p class="panel-notice error">This page is only available to the panel owner account.</p>
  <p class="muted">You are signed in as <strong>{who}</strong>. Sign out and sign in with the owner account (the first account created at install) to change this setting.</p>
  <p><a class="btn-primary" href="/settings">Back to Settings overview</a> <a class="btn-secondary" href="/dashboard">Dashboard</a></p>
</article>"#,
        title = title.replace('&', "&amp;").replace('<', "&lt;"),
        who = who,
    );
    HttpResponse::Forbidden()
        .content_type("text/html; charset=utf-8")
        .body(crate::panel_pages::panel_shell(
            username, active, title, &body,
        ))
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
