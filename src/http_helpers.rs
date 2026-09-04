//! Shared HTTP helpers for installer status and auth routes.

use crate::account::account_public_from_disk;
use crate::installer::AppState;
use crate::model::{InstallerStatus, SmtpStatusPublic, TokenQuery};
use crate::smtp_settings::{SmtpTlsMode, smtp_public_from_disk};
use actix_web::HttpRequest;
use std::time::{SystemTime, UNIX_EPOCH};

pub use crate::listen_port::DEFAULT_PORT;

/// Backward-compatible alias for the product default listen port (`2087`).
pub const PORT: u16 = DEFAULT_PORT;
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

const INSTALL_TOKEN_COOKIE: &str = "cpn_install_token";

fn constant_time_eq(a: &str, b: &str) -> bool {
    let a = a.as_bytes();
    let b = b.as_bytes();
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (left, right) in a.iter().zip(b.iter()) {
        diff |= left ^ right;
    }
    diff == 0
}

pub fn token_matches(state: &AppState, token: Option<&str>) -> bool {
    match token {
        Some(value) if !value.is_empty() => constant_time_eq(&state.token, value),
        _ => false,
    }
}

pub fn session_matches(state: &AppState, session: Option<&str>) -> bool {
    match session {
        Some(value) if !value.is_empty() => constant_time_eq(&state.session_id, value),
        _ => false,
    }
}

fn token_from_headers(request: &HttpRequest) -> Option<String> {
    if let Some(auth) = request
        .headers()
        .get(actix_web::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
    {
        let auth = auth.trim();
        if let Some(bearer) = auth
            .strip_prefix("Bearer ")
            .or_else(|| auth.strip_prefix("bearer "))
        {
            let bearer = bearer.trim();
            if !bearer.is_empty() {
                return Some(bearer.to_string());
            }
        }
    }
    if let Some(header) = request
        .headers()
        .get("X-CPN-Token")
        .and_then(|value| value.to_str().ok())
    {
        let header = header.trim();
        if !header.is_empty() {
            return Some(header.to_string());
        }
    }
    None
}

fn session_from_cookie(request: &HttpRequest) -> Option<String> {
    let cookie_header = request
        .headers()
        .get(actix_web::http::header::COOKIE)
        .and_then(|value| value.to_str().ok())?;
    for part in cookie_header.split(';') {
        let part = part.trim();
        if let Some(value) = part.strip_prefix(&format!("{INSTALL_TOKEN_COOKIE}=")) {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

/// Accept token from query, Authorization Bearer, X-CPN-Token, or HttpOnly session cookie (issue #1).
pub fn authorized(state: &AppState, query: &TokenQuery) -> bool {
    token_matches(state, Some(query.token.as_str()))
}

pub fn authorized_request(state: &AppState, query: &TokenQuery, request: &HttpRequest) -> bool {
    if authorized(state, query) {
        return true;
    }
    if token_matches(state, token_from_headers(request).as_deref()) {
        return true;
    }
    session_matches(state, session_from_cookie(request).as_deref())
}

/// When listening on 0.0.0.0, reject cross-site browser POSTs without a matching Origin/Referer.
/// Host allowlist prefers the bound loopback/public hostnames, not only the client-supplied Host header.
pub fn remote_origin_ok(request: &HttpRequest, allow_remote: bool, bind_port: u16) -> bool {
    if !allow_remote {
        return true;
    }
    let method = request.method().as_str();
    if matches!(method, "GET" | "HEAD" | "OPTIONS") {
        return true;
    }
    let origin = request
        .headers()
        .get(actix_web::http::header::ORIGIN)
        .and_then(|value| value.to_str().ok());
    let referer = request
        .headers()
        .get(actix_web::http::header::REFERER)
        .and_then(|value| value.to_str().ok());
    let candidate = origin.or(referer);
    let Some(candidate) = candidate else {
        // Non-browser clients (curl) may omit Origin; allow when Bearer/cookie/query already matched.
        return true;
    };
    origin_matches_allowed(candidate, bind_port, request)
}

fn origin_matches_allowed(candidate: &str, bind_port: u16, request: &HttpRequest) -> bool {
    let host_hdr = request
        .headers()
        .get(actix_web::http::header::HOST)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    let mut allowed = vec![
        format!("127.0.0.1:{bind_port}"),
        format!("localhost:{bind_port}"),
        format!("[::1]:{bind_port}"),
    ];
    if !host_hdr.is_empty() {
        if host_hdr.contains(':') {
            allowed.push(host_hdr.to_string());
        } else {
            allowed.push(format!("{host_hdr}:{bind_port}"));
        }
    }
    allowed
        .iter()
        .any(|host| candidate.contains(host.as_str()))
        || candidate.contains("127.0.0.1")
        || candidate.contains("localhost")
}

/// Origin check for WebSocket upgrades when `--allow-remote` is set (issue #1).
pub fn websocket_origin_ok(request: &HttpRequest, allow_remote: bool, bind_port: u16) -> bool {
    if !allow_remote {
        return true;
    }
    let origin = request
        .headers()
        .get(actix_web::http::header::ORIGIN)
        .and_then(|value| value.to_str().ok());
    let Some(origin) = origin else {
        // Non-browser clients may omit Origin after Authorization already matched.
        return true;
    };
    origin_matches_allowed(origin, bind_port, request)
}

/// Build HttpOnly install-session cookie (value is server-generated session_id).
pub fn install_session_cookie_header(session_id: &str, secure: bool) -> String {
    let secure_flag = if secure { "; Secure" } else { "" };
    format!(
        "{INSTALL_TOKEN_COOKIE}={session_id}; Path=/; HttpOnly; SameSite=Strict; Max-Age=86400{secure_flag}"
    )
}

/// True when a panel bootstrap account exists (memory or disk).
///
/// Used so `/login` stays reachable after install even when the installer is
/// re-opened in `maintenance` phase (upgrade/repair), which must still require
/// a token for the installer SPA at `/`.
pub fn panel_account_ready(status: &InstallerStatus) -> bool {
    status
        .account
        .as_ref()
        .map(|value| value.configured)
        .unwrap_or(false)
        || account_public_from_disk().is_some()
}

pub fn install_finished(status: &InstallerStatus) -> bool {
    // Maintenance keeps the installer SPA available (with token). Panel login
    // uses `panel_account_ready` separately so users are not stuck on the
    // "Installation is not finished yet" token page.
    if status.phase == "maintenance" {
        return false;
    }
    status.phase == "completed" || panel_account_ready(status)
}

pub fn wants_html(request: &HttpRequest) -> bool {
    request
        .headers()
        .get(actix_web::http::header::ACCEPT)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.contains("text/html"))
        .unwrap_or(false)
}

pub fn normalize_language(raw: &str) -> Result<String, String> {
    let value = raw.trim().to_lowercase();
    match value.as_str() {
        "en" | "en-us" | "en-gb" => Ok("en".into()),
        "es" | "es-es" | "es-mx" => Ok("es".into()),
        "nb" | "nb-no" | "no" | "nn" => Ok("nb".into()),
        _ => Err("Idioma no soportado (usa en, es o nb)".into()),
    }
}

pub fn panel_login_url_for(status: &InstallerStatus, token: &str) -> String {
    if let Some(existing) = &status.panel_login_url {
        return existing.clone();
    }
    let host = status
        .environment
        .as_ref()
        .and_then(|env_info| env_info.addresses.first())
        .cloned()
        .unwrap_or_else(|| "127.0.0.1".into());
    let port = status.listen_port;
    let _ = token;
    let base = crate::panel_network::public_base_url(port, Some(&host));
    format!("{base}/login")
}

pub fn smtp_status_public() -> SmtpStatusPublic {
    let value = smtp_public_from_disk();
    let tls_mode = value.tls_mode.map(|mode| match mode {
        SmtpTlsMode::Starttls => "starttls".into(),
        SmtpTlsMode::Tls => "tls".into(),
        SmtpTlsMode::None => "none".into(),
    });
    SmtpStatusPublic {
        configured: value.configured,
        host: value.host,
        port: value.port,
        tls_mode,
        from_address: value.from_address,
    }
}

pub fn enrich_status(mut status: InstallerStatus, token: &str) -> InstallerStatus {
    status.version = VERSION.into();
    if status.listen_port == 0 {
        status.listen_port = status
            .environment
            .as_ref()
            .map(|env_info| env_info.port)
            .unwrap_or(DEFAULT_PORT);
    }
    if let Some(env_info) = status.environment.as_mut() {
        env_info.port = status.listen_port;
    }
    if status.account.is_none() {
        status.account = account_public_from_disk();
    }
    let host_hint = status
        .environment
        .as_ref()
        .and_then(|env_info| env_info.addresses.first())
        .map(String::as_str);
    let network = crate::panel_network::network_public(status.listen_port, host_hint);
    status.panel_hostname = network.panel_hostname.clone();
    status.port_migration = network.port_migration.clone();
    status.public_base_url = Some(network.public_base_url);
    status.panel_login_url = None;
    status
        .panel_login_url
        .replace(panel_login_url_for(&status, token));
    status.smtp = Some(smtp_status_public());
    status
}

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::{install_finished, install_session_cookie_header, origin_matches_allowed, panel_account_ready};
    use crate::model::{AccountPublic, InstallerStatus};
    use actix_web::test::TestRequest;

    fn status_with_phase(phase: &'static str) -> InstallerStatus {
        let mut status = InstallerStatus::default();
        status.phase = phase;
        status
    }

    #[test]
    fn maintenance_without_account_is_not_finished() {
        let status = status_with_phase("maintenance");
        assert!(!install_finished(&status));
        // Disk bootstrap may exist in labs; panel_account_ready is covered below
        // with an in-memory configured account.
    }

    #[test]
    fn configured_account_ready_in_maintenance() {
        let mut status = status_with_phase("maintenance");
        status.account = Some(AccountPublic {
            username: "Admin".into(),
            recovery_email: "admin@example.com".into(),
            configured: true,
        });
        assert!(panel_account_ready(&status));
        assert!(!install_finished(&status));
    }

    #[test]
    fn completed_phase_is_finished() {
        let status = status_with_phase("completed");
        assert!(install_finished(&status));
    }

    #[test]
    fn install_cookie_is_httponly_samesite_strict() {
        let value = install_session_cookie_header("abc123", false);
        assert!(value.contains("HttpOnly"));
        assert!(value.contains("SameSite=Strict"));
        assert!(!value.contains("Secure"));
    }

    #[test]
    fn origin_allowlist_accepts_loopback() {
        let req = TestRequest::default()
            .insert_header((actix_web::http::header::HOST, "evil.example:9999"))
            .to_http_request();
        assert!(origin_matches_allowed(
            "http://127.0.0.1:2087",
            2087,
            &req
        ));
        assert!(!origin_matches_allowed(
            "https://attacker.example",
            2087,
            &req
        ));
    }
}
