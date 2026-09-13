//! Safe post-login return paths (`?next=` / short-lived return cookie).
//!
//! Open-redirect rules: only relative panel paths on the same host. Absolute
//! external URLs, scheme-relative URLs, and auth endpoints are rejected.
//! SnappyMail / OLS WebAdmin are out of scope (separate apps; panel session only).

use actix_web::http::header;
use actix_web::{HttpRequest, HttpResponse};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};

pub const LOGIN_RETURN_COOKIE: &str = "cpn_login_return";
pub const LOGIN_RETURN_TTL_SECONDS: u64 = 10 * 60;
const MAX_NEXT_LEN: usize = 2048;

fn percent_encode(value: &str) -> String {
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

/// Accept only same-origin relative paths (path + optional query).
pub fn sanitize_login_next(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.len() > MAX_NEXT_LEN {
        return None;
    }
    if !trimmed.starts_with('/') || trimmed.starts_with("//") || trimmed.starts_with("/\\") {
        return None;
    }
    if trimmed.contains('\\')
        || trimmed.contains("://")
        || trimmed.bytes().any(|b| b < 0x20 || b == 0x7f)
    {
        return None;
    }
    // Strip fragment; Location headers must not carry attacker-controlled fragments into loops.
    let without_hash = trimmed.split('#').next().unwrap_or(trimmed);
    if without_hash.is_empty() || !without_hash.starts_with('/') {
        return None;
    }
    let path_only = without_hash.split('?').next().unwrap_or(without_hash);
    if is_blocked_auth_path(path_only) {
        return None;
    }
    Some(without_hash.to_string())
}

fn is_blocked_auth_path(path: &str) -> bool {
    const BLOCKED: &[&str] = &[
        "/login",
        "/logout",
        "/api/logout",
        "/forgot-password",
        "/reset-password",
    ];
    for blocked in BLOCKED {
        if path == *blocked || path.starts_with(&format!("{blocked}/")) {
            return true;
        }
    }
    false
}

pub fn post_login_location(next: Option<&str>) -> String {
    next.and_then(sanitize_login_next)
        .unwrap_or_else(|| "/dashboard".to_string())
}

pub fn login_location(next: Option<&str>) -> String {
    match next.and_then(sanitize_login_next) {
        Some(n) => format!("/login?next={}", percent_encode(&n)),
        None => "/login".to_string(),
    }
}

pub fn mfa_location(next: Option<&str>) -> String {
    match next.and_then(sanitize_login_next) {
        Some(n) => format!("/login/2fa?next={}", percent_encode(&n)),
        None => "/login/2fa".to_string(),
    }
}

/// Path + query of the current request, if safe as a post-login return.
pub fn request_return_path(http: &HttpRequest) -> Option<String> {
    let path = http.uri().path();
    if path.is_empty() {
        return None;
    }
    let full = match http.uri().query() {
        Some(q) if !q.is_empty() => format!("{path}?{q}"),
        _ => path.to_string(),
    };
    sanitize_login_next(&full)
}

/// Same-host Referer path+query (used after explicit Log out).
pub fn referer_return_path(http: &HttpRequest) -> Option<String> {
    let referer = http
        .headers()
        .get(header::REFERER)
        .and_then(|v| v.to_str().ok())?;
    if referer.starts_with('/') {
        return sanitize_login_next(referer);
    }
    let after_scheme = referer.split("://").nth(1)?;
    let (host_and_port, path_and_query) = match after_scheme.split_once('/') {
        Some((h, rest)) => (h, format!("/{rest}")),
        None => (after_scheme, "/".to_string()),
    };
    let ref_host = host_and_port.split(':').next()?.trim();
    if ref_host.is_empty() {
        return None;
    }
    let req_host = http
        .headers()
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("")
        .trim();
    if req_host.is_empty() || !ref_host.eq_ignore_ascii_case(req_host) {
        return None;
    }
    sanitize_login_next(&path_and_query)
}

pub fn first_safe_next(candidates: &[Option<&str>]) -> Option<String> {
    for raw in candidates.iter().flatten() {
        if let Some(safe) = sanitize_login_next(raw) {
            return Some(safe);
        }
    }
    None
}

/// Safe post-passkey-register return paths (profile / security / dashboard only).
///
/// Rejects external hosts (via [`sanitize_login_next`]) and panel paths outside
/// account profile, account security, and `/dashboard`.
pub fn sanitize_passkey_register_next(raw: &str) -> Option<String> {
    let safe = sanitize_login_next(raw)?;
    let path = safe.split('?').next().unwrap_or(safe.as_str());
    if path == "/dashboard" {
        return Some(safe);
    }
    if path == "/account/users/profile"
        || path == "/account/users/modify"
        || path.starts_with("/account/users/profile/")
        || path.starts_with("/account/users/modify/")
    {
        return Some(safe);
    }
    if path == "/account/security" || path.starts_with("/account/security/") {
        return Some(safe);
    }
    None
}

/// Prefer a sanitized client `next`; otherwise `/dashboard` (MFA unlock default).
pub fn passkey_register_location(next: Option<&str>) -> String {
    next.and_then(sanitize_passkey_register_next)
        .unwrap_or_else(|| "/dashboard".to_string())
}

pub fn login_redirect(http: &HttpRequest) -> HttpResponse {
    let next = request_return_path(http);
    let mut builder = HttpResponse::SeeOther();
    if let Some(ref path) = next
        && let Some(cookie) =
            login_return_cookie_header(path, crate::panel_session::request_https_from_headers(http))
    {
        builder.append_header(("Set-Cookie", cookie));
    }
    builder
        .append_header(("Location", login_location(next.as_deref())))
        .finish()
}

pub fn login_return_cookie_header(next: &str, secure: bool) -> Option<String> {
    let safe = sanitize_login_next(next)?;
    let encoded = URL_SAFE_NO_PAD.encode(safe.as_bytes());
    let secure_flag = if secure { "; Secure" } else { "" };
    Some(format!(
        "{LOGIN_RETURN_COOKIE}={encoded}; Path=/; HttpOnly; SameSite=Lax; Max-Age={LOGIN_RETURN_TTL_SECONDS}{secure_flag}"
    ))
}

pub fn clear_login_return_cookie_header(secure: bool) -> String {
    let secure_flag = if secure { "; Secure" } else { "" };
    format!("{LOGIN_RETURN_COOKIE}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0{secure_flag}")
}

pub fn read_login_return_cookie(cookie_header: Option<&str>) -> Option<String> {
    let cookie_header = cookie_header?;
    for part in cookie_header.split(';') {
        let part = part.trim();
        if let Some(value) = part.strip_prefix(&format!("{LOGIN_RETURN_COOKIE}=")) {
            let value = value.trim();
            if value.is_empty() {
                continue;
            }
            let decoded = URL_SAFE_NO_PAD.decode(value.as_bytes()).ok()?;
            let path = String::from_utf8(decoded).ok()?;
            return sanitize_login_next(&path);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_relative_panel_paths() {
        assert_eq!(
            sanitize_login_next("/websites").as_deref(),
            Some("/websites")
        );
        assert_eq!(
            sanitize_login_next("/websites/manage?domain=example.com").as_deref(),
            Some("/websites/manage?domain=example.com")
        );
        assert_eq!(
            sanitize_login_next("/server/openlitespeed").as_deref(),
            Some("/server/openlitespeed")
        );
    }

    #[test]
    fn rejects_open_redirects_and_auth_loops() {
        assert!(sanitize_login_next("https://evil.example/").is_none());
        assert!(sanitize_login_next("//evil.example").is_none());
        assert!(sanitize_login_next("/\\evil").is_none());
        assert!(sanitize_login_next("javascript:alert(1)").is_none());
        assert!(sanitize_login_next("/login").is_none());
        assert!(sanitize_login_next("/login?x=1").is_none());
        assert!(sanitize_login_next("/logout").is_none());
        assert!(sanitize_login_next("/api/logout").is_none());
        assert!(sanitize_login_next("").is_none());
        assert!(sanitize_login_next("websites").is_none());
    }

    #[test]
    fn passkey_register_next_allowlist() {
        assert_eq!(
            sanitize_passkey_register_next("/account/users/modify?notice=Passkey+registered")
                .as_deref(),
            Some("/account/users/modify?notice=Passkey+registered")
        );
        assert_eq!(
            sanitize_passkey_register_next("/account/users/profile").as_deref(),
            Some("/account/users/profile")
        );
        assert_eq!(
            sanitize_passkey_register_next("/account/security/enroll-2fa").as_deref(),
            Some("/account/security/enroll-2fa")
        );
        assert_eq!(
            sanitize_passkey_register_next("/dashboard").as_deref(),
            Some("/dashboard")
        );
        assert!(sanitize_passkey_register_next("/websites").is_none());
        assert!(sanitize_passkey_register_next("https://evil.example/").is_none());
        assert!(sanitize_passkey_register_next("//evil").is_none());
        assert_eq!(passkey_register_location(None), "/dashboard");
        assert_eq!(
            passkey_register_location(Some("/account/users/modify")),
            "/account/users/modify"
        );
        assert_eq!(
            passkey_register_location(Some("/packages")),
            "/dashboard"
        );
    }

    #[test]
    fn post_login_falls_back_to_dashboard() {
        assert_eq!(post_login_location(None), "/dashboard");
        assert_eq!(post_login_location(Some("//evil")), "/dashboard");
        assert_eq!(post_login_location(Some("/packages")), "/packages");
    }

    #[test]
    fn login_location_encodes_next() {
        let loc = login_location(Some("/websites?x=1"));
        assert!(loc.starts_with("/login?next="));
        assert!(loc.contains("%2Fwebsites"));
    }
}
