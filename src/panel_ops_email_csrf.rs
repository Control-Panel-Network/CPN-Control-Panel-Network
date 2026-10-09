//! CSRF tokens for Email hub tools (HMAC, hour-bucketed).

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

pub fn email_csrf_token(username: &str) -> String {
    email_csrf_token_with_secret(&session_secret(None), username)
}

pub fn verify_email_csrf(username: &str, token: &str) -> bool {
    verify_email_csrf_with_secret(&session_secret(None), username, token)
}

/// Token for an explicit secret (tests and callers that already resolved the
/// session secret once; avoids re-reading the data dir on every call).
pub fn email_csrf_token_with_secret(secret: &str, username: &str) -> String {
    let hour = now_unix() / 3600;
    let payload = format!("email-tools|{username}|{hour}");
    format!("{hour}.{}", hmac_hex(secret, &payload))
}

/// Verify against an explicit secret (see `email_csrf_token_with_secret`).
pub fn verify_email_csrf_with_secret(secret: &str, username: &str, token: &str) -> bool {
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
    let payload = format!("email-tools|{username}|{hour}");
    let expected = hmac_hex(secret, &payload);
    expected == sig
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csrf_roundtrip() {
        // Explicit secret: `session_secret(None)` resolves the data dir on each call, and
        // other tests swap `CPN_DATA_DIR` concurrently (`with_test_data_dir`). On hosts where
        // the default data dir is writable (Windows dev builds) the two resolutions could
        // return different persisted secrets and make this test flaky.
        let secret = format!("test-secret-{}", std::process::id());
        let t = email_csrf_token_with_secret(&secret, "Admin");
        assert!(verify_email_csrf_with_secret(&secret, "Admin", &t));
        assert!(!verify_email_csrf_with_secret(&secret, "other", &t));
        assert!(!verify_email_csrf_with_secret(
            "another-secret",
            "Admin",
            &t
        ));
    }

    #[test]
    fn csrf_rejects_malformed_tokens() {
        let secret = "s";
        assert!(!verify_email_csrf_with_secret(secret, "Admin", ""));
        assert!(!verify_email_csrf_with_secret(secret, "Admin", "no-dot"));
        assert!(!verify_email_csrf_with_secret(secret, "Admin", "abc.def"));
        // Hour bucket far in the past is rejected even with a valid signature shape.
        assert!(!verify_email_csrf_with_secret(secret, "Admin", "1.00"));
    }
}
