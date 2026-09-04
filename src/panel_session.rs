//! Panel session cookies (HMAC-SHA256), shared format with `Panel/src/lib/auth.ts`.

use crate::account::data_dir;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use hmac::{Hmac, Mac};
use rand::Rng;
use sha2::Sha256;
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

pub const SESSION_COOKIE: &str = "cpn_panel_session";
pub const SESSION_TTL_SECONDS: u64 = 60 * 60 * 12;

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
    if std::env::var("CPN_ALLOW_DEV_SESSION").ok().as_deref() == Some("1") || cfg!(test) {
        return "cpn-panel-dev-session".into();
    }
    // Last resort for unit/dev hosts that cannot write the data dir: still unique per process.
    let bytes: [u8; 32] = rand::rng().random();
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
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

pub fn verify_session_token(token: &str, secret: &str) -> Option<String> {
    let decoded = URL_SAFE_NO_PAD.decode(token.as_bytes()).ok()?;
    let decoded = String::from_utf8(decoded).ok()?;
    let mut parts = decoded.splitn(3, '|');
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
    let payload = format!("{username}|{exp}");
    let expected = hmac_hex(secret, &payload);
    if !constant_time_eq(sig, &expected) {
        return None;
    }
    Some(username)
}

pub fn session_cookie_header(token: &str, secure: bool) -> String {
    let secure_flag = if secure { "; Secure" } else { "" };
    format!(
        "{SESSION_COOKIE}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={SESSION_TTL_SECONDS}{secure_flag}"
    )
}

pub fn clear_session_cookie_header(secure: bool) -> String {
    let secure_flag = if secure { "; Secure" } else { "" };
    format!("{SESSION_COOKIE}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0{secure_flag}")
}

pub fn read_session_cookie(cookie_header: Option<&str>) -> Option<String> {
    let cookie_header = cookie_header?;
    for part in cookie_header.split(';') {
        let part = part.trim();
        if let Some(value) = part.strip_prefix(&format!("{SESSION_COOKIE}=")) {
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
        let first = proto.split(',').next().unwrap_or("").trim();
        if first.eq_ignore_ascii_case("https") {
            return true;
        }
        if first.eq_ignore_ascii_case("http") {
            return false;
        }
    }
    connection_scheme.eq_ignore_ascii_case("https")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;

    #[test]
    fn round_trip_session_token() {
        with_test_data_dir(|| {
            let secret = "unit-test-secret";
            let token = create_session_token("Admin", secret);
            assert_eq!(
                verify_session_token(&token, secret).as_deref(),
                Some("Admin")
            );
            assert!(verify_session_token(&token, "other").is_none());
        });
    }

    #[test]
    fn cookie_headers_use_samesite_lax() {
        let set = session_cookie_header("abc", false);
        assert!(set.contains("HttpOnly"));
        assert!(set.contains("SameSite=Lax"));
        assert!(!set.contains("Secure"));
        let secure = session_cookie_header("abc", true);
        assert!(secure.contains("Secure"));
        let clear = clear_session_cookie_header(false);
        assert!(clear.contains("Max-Age=0"));
    }
}
