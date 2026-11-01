//! CSRF tokens for Sub-users POSTs (HMAC, hour-bucketed).

use crate::panel_session::session_secret;
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use std::time::{SystemTime, UNIX_EPOCH};

type HmacSha256 = Hmac<Sha256>;

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

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn subusers_csrf_token(username: &str) -> String {
    subusers_csrf_token_with_secret(&session_secret(None), username)
}

pub fn verify_subusers_csrf(username: &str, token: &str) -> bool {
    verify_subusers_csrf_with_secret(&session_secret(None), username, token)
}

pub fn subusers_csrf_token_with_secret(secret: &str, username: &str) -> String {
    let hour = now_unix() / 3600;
    let payload = format!("sub-users|{username}|{hour}");
    format!("{hour}.{}", hmac_hex(secret, &payload))
}

pub fn verify_subusers_csrf_with_secret(secret: &str, username: &str, token: &str) -> bool {
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
    let payload = format!("sub-users|{username}|{hour}");
    let expected = hmac_hex(secret, &payload);
    expected == sig
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csrf_roundtrip() {
        let secret = format!("subusers-secret-{}", std::process::id());
        let t = subusers_csrf_token_with_secret(&secret, "cpnowner");
        assert!(verify_subusers_csrf_with_secret(&secret, "cpnowner", &t));
        assert!(!verify_subusers_csrf_with_secret(&secret, "other", &t));
    }
}
