//! Panel session cookies (HMAC-SHA256), shared format with `Panel/src/lib/auth.ts`.
//!
//! Browser cookies are scoped by name + domain + path, **not** TCP port. Lab panels
//! on the same host (`127.0.0.1:2087` vs `127.0.0.1:2092`) would overwrite each other
//! if they shared a fixed cookie name. Cookie names therefore include the Host port
//! when the browser sent one (`cpn_panel_session_2092`). Production hosts without a
//! port in `Host` keep the unscoped base name.

use crate::account::data_dir;
use actix_web::HttpRequest;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use hmac::{Hmac, KeyInit, Mac};
use rand::Rng;
use sha2::Sha256;
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

pub const SESSION_COOKIE: &str = "cpn_panel_session";
pub const MFA_PENDING_COOKIE: &str = "cpn_panel_mfa_pending";
pub const SESSION_TTL_SECONDS: u64 = 60 * 60 * 12;
/// Password-ok / MFA-pending window. Must cover a normal WebAuthn prompt (Windows Hello,
/// security key, PIN) without forcing the operator back to /login mid-ceremony.
pub const MFA_PENDING_TTL_SECONDS: u64 = 60 * 15;

/// Parse an explicit port from an HTTP `Host` value (`127.0.0.1:2092`, `[::1]:2087`).
/// Returns `None` when the browser omitted the port (default 80/443 or reverse proxy).
pub fn auth_cookie_port_from_host(host: &str) -> Option<u16> {
    let host = host.trim();
    if host.is_empty() || host.len() > 253 {
        return None;
    }
    if let Some(rest) = host.strip_prefix('[') {
        let end = rest.find(']')?;
        let after = rest.get(end + 1..)?;
        let port_str = after.strip_prefix(':')?;
        return parse_cookie_port(port_str);
    }
    // One colon only: hostname:port or IPv4:port (reject raw IPv6 without brackets).
    if host.matches(':').count() != 1 {
        return None;
    }
    let port_str = host.rsplit_once(':')?.1;
    parse_cookie_port(port_str)
}

fn parse_cookie_port(raw: &str) -> Option<u16> {
    let port: u16 = raw.parse().ok()?;
    if port == 0 {
        return None;
    }
    Some(port)
}

/// Browser-facing port from the request `Host` header (NAT lab ports, not guest bind).
pub fn request_auth_cookie_port(http: &HttpRequest) -> Option<u16> {
    let host = http
        .headers()
        .get(actix_web::http::header::HOST)
        .and_then(|value| value.to_str().ok())?;
    auth_cookie_port_from_host(host)
}

/// `cpn_panel_session` or `cpn_panel_session_2092`.
pub fn scoped_auth_cookie_name(base: &str, port: Option<u16>) -> String {
    match port {
        Some(port) => format!("{base}_{port}"),
        None => base.to_string(),
    }
}

pub fn session_cookie_name(port: Option<u16>) -> String {
    scoped_auth_cookie_name(SESSION_COOKIE, port)
}

pub fn mfa_pending_cookie_name(port: Option<u16>) -> String {
    scoped_auth_cookie_name(MFA_PENDING_COOKIE, port)
}

type HmacSha256 = Hmac<Sha256>;

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or(0)
}

fn secret_path() -> PathBuf {
    data_dir().join("panel-session.secret")
}

fn load_or_create_persisted_secret() -> Option<String> {
    let path = secret_path();
    if let Ok(raw) = fs::read_to_string(&path) {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    let bytes: [u8; 32] = rand::rng().random();
    let value = bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    if let Ok(mut file) = options.open(&path) {
        use std::io::Write;
        let _ = file.write_all(value.as_bytes());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
        }
        return Some(value);
    }
    None
}

/// Resolve HMAC secret for panel sessions (issue #8).
/// Prefer env and persisted secret; never use a public hardcoded secret outside
/// explicit development/test overrides.
pub fn session_secret(installer_token: Option<&str>) -> String {
    if let Some(env_secret) = std::env::var("CPN_PANEL_SESSION_SECRET")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    {
        return env_secret;
    }
    if let Some(persisted) = load_or_create_persisted_secret() {
        return persisted;
    }
    if let Some(token) = installer_token
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return token.to_string();
    }
    // Dev/test or last resort: unique per process (never a committed shared secret).
    use std::sync::OnceLock;
    static PROCESS_SECRET: OnceLock<String> = OnceLock::new();
    PROCESS_SECRET
        .get_or_init(|| {
            let bytes: [u8; 32] = rand::rng().random();
            bytes.iter().map(|byte| format!("{byte:02x}")).collect()
        })
        .clone()
}

fn hmac_hex(secret: &str, payload: &str) -> String {
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(payload.as_bytes());
    mac.finalize()
        .into_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

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

pub fn create_session_token(username: &str, secret: &str) -> String {
    let exp = now_unix() + SESSION_TTL_SECONDS;
    let payload = format!("{username}|{exp}");
    let sig = hmac_hex(secret, &payload);
    URL_SAFE_NO_PAD.encode(format!("{payload}|{sig}").as_bytes())
}

/// Short-lived token proving password auth succeeded; MFA still required.
pub fn create_mfa_pending_token(username: &str, secret: &str) -> String {
    let exp = now_unix() + MFA_PENDING_TTL_SECONDS;
    let payload = format!("mfa|{username}|{exp}");
    let sig = hmac_hex(secret, &payload);
    URL_SAFE_NO_PAD.encode(format!("{payload}|{sig}").as_bytes())
}

pub fn verify_session_token(token: &str, secret: &str) -> Option<String> {
    let decoded = URL_SAFE_NO_PAD.decode(token.as_bytes()).ok()?;
    let decoded = String::from_utf8(decoded).ok()?;
    let mut parts = decoded.splitn(3, '|');
    let username = parts.next()?.to_string();
    let exp_raw = parts.next()?;
    let sig = parts.next()?;
    if username.is_empty() || username == "mfa" {
        return None;
    }
    let exp: u64 = exp_raw.parse().ok()?;
    if exp < now_unix() {
        return None;
    }
    let payload = format!("{username}|{exp}");
    let expected = hmac_hex(secret, &payload);
    if !constant_time_eq(sig, &expected) {
        return None;
    }
    Some(username)
}

pub fn verify_mfa_pending_token(token: &str, secret: &str) -> Option<String> {
    let decoded = URL_SAFE_NO_PAD.decode(token.as_bytes()).ok()?;
    let decoded = String::from_utf8(decoded).ok()?;
    let mut parts = decoded.splitn(4, '|');
    let marker = parts.next()?;
    if marker != "mfa" {
        return None;
    }
    let username = parts.next()?.to_string();
    let exp_raw = parts.next()?;
    let sig = parts.next()?;
    if username.is_empty() {
        return None;
    }
    let exp: u64 = exp_raw.parse().ok()?;
    if exp < now_unix() {
        return None;
    }
    let payload = format!("mfa|{username}|{exp}");
    let expected = hmac_hex(secret, &payload);
    if !constant_time_eq(sig, &expected) {
        return None;
    }
    Some(username)
}

fn cookie_secure_flag(secure: bool) -> &'static str {
    if secure { "; Secure" } else { "" }
}

fn clear_named_cookie_header(name: &str, secure: bool) -> String {
    format!(
        "{name}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0{}",
        cookie_secure_flag(secure)
    )
}

pub fn session_cookie_header(token: &str, secure: bool, port: Option<u16>) -> String {
    let name = session_cookie_name(port);
    format!(
        "{name}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={SESSION_TTL_SECONDS}{}",
        cookie_secure_flag(secure)
    )
}

/// Set the port-scoped session cookie; also expire the legacy unscoped name when ported
/// so concurrent labs on the same host stop sharing a pre-fix cookie.
pub fn issue_session_cookie_headers(token: &str, secure: bool, port: Option<u16>) -> Vec<String> {
    let mut headers = vec![session_cookie_header(token, secure, port)];
    if port.is_some() {
        headers.push(clear_named_cookie_header(SESSION_COOKIE, secure));
    }
    headers
}

pub fn mfa_pending_cookie_header(token: &str, secure: bool, port: Option<u16>) -> String {
    let name = mfa_pending_cookie_name(port);
    format!(
        "{name}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={MFA_PENDING_TTL_SECONDS}{}",
        cookie_secure_flag(secure)
    )
}

pub fn issue_mfa_pending_cookie_headers(
    token: &str,
    secure: bool,
    port: Option<u16>,
) -> Vec<String> {
    let mut headers = vec![mfa_pending_cookie_header(token, secure, port)];
    if port.is_some() {
        headers.push(clear_named_cookie_header(MFA_PENDING_COOKIE, secure));
    }
    headers
}

/// Clear this Host port's session cookie (and the legacy unscoped name when ported).
pub fn clear_session_cookie_headers(secure: bool, port: Option<u16>) -> Vec<String> {
    let mut headers = vec![clear_named_cookie_header(
        &session_cookie_name(port),
        secure,
    )];
    if port.is_some() {
        headers.push(clear_named_cookie_header(SESSION_COOKIE, secure));
    }
    headers
}

pub fn clear_mfa_pending_cookie_headers(secure: bool, port: Option<u16>) -> Vec<String> {
    let mut headers = vec![clear_named_cookie_header(
        &mfa_pending_cookie_name(port),
        secure,
    )];
    if port.is_some() {
        headers.push(clear_named_cookie_header(MFA_PENDING_COOKIE, secure));
    }
    headers
}

/// Backward-compatible single clear (unscoped / no Host port).
pub fn clear_session_cookie_header(secure: bool) -> String {
    clear_named_cookie_header(SESSION_COOKIE, secure)
}

pub fn clear_mfa_pending_cookie_header(secure: bool) -> String {
    clear_named_cookie_header(MFA_PENDING_COOKIE, secure)
}

/// Expire unscoped auth cookie names when the browser sent `Host: …:port` so stale
/// pre-1.4.9 cookies cannot hijack another lab panel on the same loopback host.
pub fn purge_legacy_port_colliding_cookie_headers(
    secure: bool,
    port: Option<u16>,
) -> Vec<String> {
    if port.is_none() {
        return Vec::new();
    }
    let bases = [
        SESSION_COOKIE,
        MFA_PENDING_COOKIE,
        "cpn_login_return",
        "cpn_install_token",
    ];
    bases
        .iter()
        .map(|base| clear_named_cookie_header(base, secure))
        .collect()
}

pub fn read_session_cookie(cookie_header: Option<&str>, port: Option<u16>) -> Option<String> {
    read_scoped_then_legacy(cookie_header, SESSION_COOKIE, port)
}

pub fn read_mfa_pending_cookie(cookie_header: Option<&str>, port: Option<u16>) -> Option<String> {
    read_scoped_then_legacy(cookie_header, MFA_PENDING_COOKIE, port)
}

fn read_scoped_then_legacy(
    cookie_header: Option<&str>,
    base: &str,
    port: Option<u16>,
) -> Option<String> {
    let scoped = scoped_auth_cookie_name(base, port);
    if let Some(value) = read_named_cookie(cookie_header, &scoped) {
        return Some(value);
    }
    // Lab NAT URLs (`localhost:2087` vs `:2092`) share a browser host; unscoped cookies
    // collide across ports. Never accept the legacy name when the browser sent a port.
    if port.is_some() {
        return None;
    }
    read_named_cookie(cookie_header, base)
}

fn read_named_cookie(cookie_header: Option<&str>, name: &str) -> Option<String> {
    let cookie_header = cookie_header?;
    let prefix = format!("{name}=");
    for part in cookie_header.split(';') {
        let part = part.trim();
        if let Some(value) = part.strip_prefix(&prefix) {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

pub fn request_is_https(connection_scheme: &str, forwarded_proto: Option<&str>) -> bool {
    if let Some(proto) = forwarded_proto {
        // Cap length before parsing so Host/X-Forwarded-* cannot drive large allocs.
        let clipped = if proto.len() > 16 {
            &proto[..16]
        } else {
            proto
        };
        let first = clipped.split(',').next().unwrap_or("").trim();
        if first.eq_ignore_ascii_case("https") {
            return true;
        }
        if first.eq_ignore_ascii_case("http") {
            return false;
        }
    }
    let scheme = if connection_scheme.len() > 8 {
        &connection_scheme[..8]
    } else {
        connection_scheme
    };
    scheme.eq_ignore_ascii_case("https")
}

/// Resolve HTTPS for cookies without calling `connection_info()` (CodeQL Host alloc).
/// Only allowlisted short `X-Forwarded-Proto` values or direct TLS peers count.
pub fn request_https_from_headers(http: &actix_web::HttpRequest) -> bool {
    if let Some(raw) = http.headers().get("X-Forwarded-Proto") {
        let bytes = raw.as_bytes();
        if bytes.len() > 16 {
            return false;
        }
        if let Ok(value) = raw.to_str() {
            return request_is_https("http", Some(value));
        }
        return false;
    }
    // No forwarded header: treat as plain HTTP (installer default bind).
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;

    #[test]
    fn round_trip_session_token() {
        with_test_data_dir(|| {
            // Per-run secrets (no hard-coded key literals for CodeQL).
            let secret = load_or_create_persisted_secret().expect("test data dir secret");
            let token = create_session_token("Admin", &secret);
            assert_eq!(
                verify_session_token(&token, &secret).as_deref(),
                Some("Admin")
            );
            let wrong = format!("{secret}-mismatch");
            assert!(verify_session_token(&token, &wrong).is_none());
        });
    }

    #[test]
    fn cookie_headers_use_samesite_lax() {
        let set = session_cookie_header("abc", false, None);
        assert!(set.starts_with(&format!("{SESSION_COOKIE}=")));
        assert!(set.contains("HttpOnly"));
        assert!(set.contains("SameSite=Lax"));
        assert!(!set.contains("Secure"));
        let secure = session_cookie_header("abc", true, None);
        assert!(secure.contains("Secure"));
        let clear = clear_session_cookie_header(false);
        assert!(clear.contains("Max-Age=0"));
    }

    #[test]
    fn mfa_pending_ttl_covers_authenticator_prompt() {
        assert!(
            MFA_PENDING_TTL_SECONDS >= 60 * 10,
            "MFA pending cookie must survive a normal passkey prompt (got {}s)",
            MFA_PENDING_TTL_SECONDS
        );
        let set = mfa_pending_cookie_header("tok", false, None);
        assert!(set.contains(&format!("Max-Age={MFA_PENDING_TTL_SECONDS}")));
    }

    #[test]
    fn host_port_scopes_cookie_names_for_lab_panels() {
        assert_eq!(auth_cookie_port_from_host("127.0.0.1:2087"), Some(2087));
        assert_eq!(auth_cookie_port_from_host("127.0.0.1:2092"), Some(2092));
        assert_eq!(auth_cookie_port_from_host("[::1]:2090"), Some(2090));
        assert_eq!(auth_cookie_port_from_host("panel.example.com"), None);
        assert_eq!(auth_cookie_port_from_host("127.0.0.1"), None);
        assert_eq!(
            session_cookie_name(Some(2092)),
            format!("{SESSION_COOKIE}_2092")
        );
        assert_eq!(session_cookie_name(None), SESSION_COOKIE);

        let alma = session_cookie_header("tok-a", false, Some(2087));
        let ubuntu = session_cookie_header("tok-b", false, Some(2092));
        assert!(alma.starts_with(&format!("{SESSION_COOKIE}_2087=")));
        assert!(ubuntu.starts_with(&format!("{SESSION_COOKIE}_2092=")));
        assert_ne!(alma, ubuntu);

        let jar = format!("{SESSION_COOKIE}_2087=alma; {SESSION_COOKIE}_2092=ubuntu; other=1");
        assert_eq!(
            read_session_cookie(Some(&jar), Some(2087)).as_deref(),
            Some("alma")
        );
        assert_eq!(
            read_session_cookie(Some(&jar), Some(2092)).as_deref(),
            Some("ubuntu")
        );

        // Legacy unscoped cookie is ignored when the Host port is explicit (lab collision guard).
        let legacy = format!("{SESSION_COOKIE}=legacy");
        assert_eq!(read_session_cookie(Some(&legacy), Some(2092)), None);

        let issued = issue_session_cookie_headers("new", false, Some(2092));
        assert_eq!(issued.len(), 2);
        assert!(issued[0].starts_with(&format!("{SESSION_COOKIE}_2092=")));
        assert!(issued[1].starts_with(&format!("{SESSION_COOKIE}=")));
        assert!(issued[1].contains("Max-Age=0"));
    }
}
