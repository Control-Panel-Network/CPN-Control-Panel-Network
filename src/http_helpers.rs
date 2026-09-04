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
/// Fixed cookie value length (matches generated session_id / install token size).
const INSTALL_COOKIE_VALUE_LEN: usize = 28;

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
    let host_hdr = request
        .headers()
        .get(actix_web::http::header::HOST)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    let expected_port = if host_hdr.contains(':') {
        host_hdr.to_string()
    } else {
        format!("{host_hdr}:{bind_port}")
    };
    candidate.contains(&expected_port)
        || candidate.contains("127.0.0.1")
        || candidate.contains("localhost")
}

/// Prebuilt Set-Cookie HeaderValues (constructed at process start, never from request input).
#[derive(Clone)]
pub struct InstallSessionCookies {
    pub http: actix_web::http::header::HeaderValue,
    pub https: actix_web::http::header::HeaderValue,
}

/// Cookie for the server-generated install session (not the URL token).
pub fn install_token_cookie_header(session_id: &str, secure: bool) -> Option<String> {
    let session_id = session_id.as_bytes();
    if session_id.len() != INSTALL_COOKIE_VALUE_LEN {
        return None;
    }
    if !session_id.iter().all(|b| b.is_ascii_alphanumeric()) {
        return None;
    }
    // Fixed-capacity buffer avoids unbounded allocation from request data.
    let mut value = String::with_capacity(INSTALL_COOKIE_VALUE_LEN);
    for &b in session_id {
        value.push(b as char);
    }
    let secure_flag = if secure { "; Secure" } else { "" };
    Some(format!(
        "{INSTALL_TOKEN_COOKIE}={value}; Path=/; HttpOnly; SameSite=Strict{secure_flag}; Max-Age=86400"
    ))
}

pub fn build_install_session_cookies(session_id: &str) -> Option<InstallSessionCookies> {
    let http = install_token_cookie_header(session_id, false)?;
    let https = install_token_cookie_header(session_id, true)?;
    Some(InstallSessionCookies {
        http: actix_web::http::header::HeaderValue::try_from(http).ok()?,
        https: actix_web::http::header::HeaderValue::try_from(https).ok()?,
    })
}

pub fn install_finished(status: &InstallerStatus) -> bool {
    if status.phase == "maintenance" {
        return false;
    }
    status.phase == "completed"
        || status
            .account
            .as_ref()
            .map(|value| value.configured)
            .unwrap_or(false)
        || account_public_from_disk().is_some()
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
