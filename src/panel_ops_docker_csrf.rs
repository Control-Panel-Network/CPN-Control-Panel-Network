//! CSRF tokens for Docker container POST actions (HMAC, hour-bucketed).

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

fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.bytes().zip(b.bytes()) {
        diff |= x ^ y;
    }
    diff == 0
}

pub fn docker_csrf_token(username: &str) -> String {
    docker_csrf_token_with_secret(&session_secret(None), username)
}

pub fn verify_docker_csrf(username: &str, token: &str) -> bool {
    verify_docker_csrf_with_secret(&session_secret(None), username, token)
}

pub fn docker_csrf_token_with_secret(secret: &str, username: &str) -> String {
    let hour = now_unix() / 3600;
    let payload = format!("docker|{username}|{hour}");
    format!("{hour}.{}", hmac_hex(secret, &payload))
}

pub fn verify_docker_csrf_with_secret(secret: &str, username: &str, token: &str) -> bool {
    let token = token.trim();
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
    let payload = format!("docker|{username}|{hour}");
    let expected = hmac_hex(secret, &payload);
    constant_time_eq(&expected, sig)
}

/// Hidden CSRF input for Docker forms.
pub fn docker_csrf_hidden(username: &str) -> String {
    format!(
        r#"<input type="hidden" name="csrf" value="{}">"#,
        html_escape_attr(&docker_csrf_token(username))
    )
}

fn html_escape_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Admin/owner may manage any container; account owners only their own label.
/// Host and Unassigned containers stay admin-only.
pub fn user_can_manage_container(username: &str, is_admin: bool, container_owner: &str) -> bool {
    if is_admin {
        return true;
    }
    let owner = container_owner.trim();
    if owner.is_empty()
        || owner.eq_ignore_ascii_case("Host")
        || owner.eq_ignore_ascii_case("Unassigned")
        || owner.eq_ignore_ascii_case("CPN")
    {
        return false;
    }
    owner.eq_ignore_ascii_case(username.trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csrf_roundtrip() {
        let secret = format!("docker-csrf-{}", std::process::id());
        let t = docker_csrf_token_with_secret(&secret, "Admin");
        assert!(verify_docker_csrf_with_secret(&secret, "Admin", &t));
        assert!(!verify_docker_csrf_with_secret(&secret, "other", &t));
        let other = format!("{secret}-mismatch");
        assert!(!verify_docker_csrf_with_secret(&other, "Admin", &t));
    }

    #[test]
    fn csrf_rejects_malformed() {
        let secret = format!("docker-bad-{}", std::process::id());
        assert!(!verify_docker_csrf_with_secret(&secret, "Admin", ""));
        assert!(!verify_docker_csrf_with_secret(&secret, "Admin", "no-dot"));
        assert!(!verify_docker_csrf_with_secret(&secret, "Admin", "1.00"));
    }

    #[test]
    fn manage_acl_owner_and_admin() {
        assert!(user_can_manage_container("alice", true, "bob"));
        assert!(user_can_manage_container("alice", false, "alice"));
        assert!(user_can_manage_container("Alice", false, "alice"));
        assert!(!user_can_manage_container("alice", false, "bob"));
        assert!(!user_can_manage_container("alice", false, "Host"));
        assert!(!user_can_manage_container("alice", false, "Unassigned"));
    }
}
