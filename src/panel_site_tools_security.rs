//! CSRF and rate limits for site terminal, git, and clone/staging tools.

use crate::panel_session::session_secret;
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

type HmacSha256 = Hmac<Sha256>;

const RATE_WINDOW_SECS: u64 = 60;
const RATE_MAX_OPS: u32 = 60;
const TERM_RATE_MAX: u32 = 8;

static RATE: Mutex<Option<HashMap<String, (u64, u32)>>> = Mutex::new(None);
static TERM_RATE: Mutex<Option<HashMap<String, (u64, u32)>>> = Mutex::new(None);

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn hmac_hex(secret: &str, payload: &str) -> String {
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(payload.as_bytes());
    mac.finalize()
        .into_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn verify_hmac_hex(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.bytes().zip(b.bytes()) {
        diff |= x ^ y;
    }
    diff == 0
}

fn mint_csrf(purpose: &str, username: &str, domain: &str) -> String {
    let secret = session_secret(None);
    let hour = now_unix() / 3600;
    let payload = format!("{purpose}|{username}|{domain}|{hour}");
    format!("{hour}.{}", hmac_hex(&secret, &payload))
}

fn check_csrf(purpose: &str, username: &str, domain: &str, token: &str) -> bool {
    let secret = session_secret(None);
    let Some((hour_s, sig)) = token.split_once('.') else {
        return false;
    };
    let Ok(hour) = hour_s.parse::<u64>() else {
        return false;
    };
    let current = now_unix() / 3600;
    if hour + 2 < current || hour > current + 1 {
        return false;
    }
    let payload = format!("{purpose}|{username}|{domain}|{hour}");
    let expected = hmac_hex(&secret, &payload);
    verify_hmac_hex(&expected, sig)
}

pub fn site_tools_csrf_token(username: &str, domain: &str) -> String {
    mint_csrf("site-tools", username, domain)
}

pub fn verify_site_tools_csrf(username: &str, domain: &str, token: &str) -> bool {
    check_csrf("site-tools", username, domain, token)
}

pub fn terminal_csrf_token(username: &str, domain: &str) -> String {
    mint_csrf("site-term", username, domain)
}

pub fn verify_terminal_csrf(username: &str, domain: &str, token: &str) -> bool {
    check_csrf("site-term", username, domain, token)
}

fn bump_rate(
    map_slot: &Mutex<Option<HashMap<String, (u64, u32)>>>,
    key: &str,
    max: u32,
    busy: &str,
    limited: &str,
) -> Result<(), String> {
    let now = now_unix();
    let mut guard = map_slot.lock().map_err(|_| busy.to_string())?;
    let map = guard.get_or_insert_with(HashMap::new);
    let entry = map.entry(key.to_string()).or_insert((now, 0));
    if now.saturating_sub(entry.0) >= RATE_WINDOW_SECS {
        *entry = (now, 0);
    }
    if entry.1 >= max {
        return Err(limited.into());
    }
    entry.1 += 1;
    Ok(())
}

pub fn check_tools_rate_limit(username: &str) -> Result<(), String> {
    bump_rate(
        &RATE,
        username,
        RATE_MAX_OPS,
        "Rate limiter busy",
        "Too many site tool operations; wait a minute and try again",
    )
}

pub fn check_terminal_rate_limit(username: &str) -> Result<(), String> {
    bump_rate(
        &TERM_RATE,
        username,
        TERM_RATE_MAX,
        "Terminal rate limiter busy",
        "Too many terminal sessions; wait a minute and try again",
    )
}

pub fn same_origin_ok(http: &actix_web::HttpRequest) -> bool {
    let Some(origin) = http
        .headers()
        .get("origin")
        .or_else(|| http.headers().get("referer"))
        .and_then(|v| v.to_str().ok())
    else {
        return true;
    };
    let host = http.connection_info().host().to_string();
    origin.contains(&host)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;

    #[test]
    fn csrf_roundtrip() {
        with_test_data_dir(|| {
            let t = site_tools_csrf_token("Admin", "example.com");
            assert!(verify_site_tools_csrf("Admin", "example.com", &t));
            assert!(!verify_site_tools_csrf("Other", "example.com", &t));
            assert!(!verify_site_tools_csrf("Admin", "other.com", &t));
            let term = terminal_csrf_token("Admin", "example.com");
            assert!(verify_terminal_csrf("Admin", "example.com", &term));
            assert!(!verify_site_tools_csrf("Admin", "example.com", &term));
        });
    }
}
