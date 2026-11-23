//! Tachyon / SnappyMail-family `/?admin` sign-in bridge to the CPN panel owner account.
//!
//! The webmail admin form posts to a tiny PHP hook (installed by
//! `tachyon_admin_bridge_install`) which calls this loopback-only endpoint. Only the
//! panel owner (bootstrap admin) may authenticate. Normal panel users, resellers, and
//! ACL (sub-user) accounts are always denied. Mailbox IMAP login for regular webmail is
//! not touched: mail users are never webmail admins.
//!
//! Password verification reuses the panel hash store (PBKDF2). When the owner has TOTP
//! enrolled, the same authenticator or backup code is required (shared MFA rate limits).
//! Passkey-only owners are denied here (the webmail admin form has no passkey step);
//! enroll TOTP in CPN Security to use webmail admin. Secrets are never logged.

use crate::account::{MAX_PASSWORD_CHARS, PanelBootstrap, load_bootstrap, verify_password};
use crate::account_mfa::{
    check_rate_limit, clear_rate_limit, register_rate_failure, totp_enabled_for,
    verify_mfa_challenge,
};
use crate::account_passkeys::has_passkeys;
use crate::panel_audit_types::{AuditCategory, AuditSeverity};
use actix_web::{HttpRequest, HttpResponse, post, web};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Header that carries the shared bridge token (never a panel session cookie).
pub const BRIDGE_TOKEN_HEADER: &str = "x-cpn-bridge-token";
/// Default location of the bridge config (root-owned, group `cpn-webmail`, mode 0640).
pub const DEFAULT_CONFIG_PATH: &str = "/var/lib/cpn-webmail/admin-bridge.json";
/// Env override for tests and non-standard layouts.
pub const CONFIG_PATH_ENV: &str = "CPN_TACHYON_BRIDGE_CONFIG";
const MAX_USERNAME_CHARS: usize = 128;
const MAX_TOTP_CHARS: usize = 32;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BridgeConfig {
    pub version: u32,
    /// Loopback panel base URL (for example `http://127.0.0.1:2086`).
    pub url: String,
    /// Shared secret for the loopback call (random, 64 hex chars).
    pub token: String,
}

/// Why a webmail admin sign-in was refused (audit only; HTTP replies stay generic).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DenyReason {
    Invalid,
    RateLimited,
    NotOwner,
    RoleNotAllowed,
    Disabled,
    MustChangePassword,
    BadPassword,
    MfaRequired,
    MfaEnrollRequired,
    PasskeyOnly,
    MfaFailed,
}

impl DenyReason {
    pub fn code(self) -> &'static str {
        match self {
            DenyReason::Invalid => "invalid_input",
            DenyReason::RateLimited => "rate_limited",
            DenyReason::NotOwner => "not_panel_owner",
            DenyReason::RoleNotAllowed => "role_not_allowed",
            DenyReason::Disabled => "account_disabled",
            DenyReason::MustChangePassword => "must_change_password",
            DenyReason::BadPassword => "bad_credentials",
            DenyReason::MfaRequired => "mfa_required",
            DenyReason::MfaEnrollRequired => "mfa_enroll_required",
            DenyReason::PasskeyOnly => "passkey_only_owner",
            DenyReason::MfaFailed => "mfa_failed",
        }
    }
}

pub fn config_path() -> PathBuf {
    std::env::var(CONFIG_PATH_ENV)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_CONFIG_PATH))
}

pub fn load_config() -> Option<BridgeConfig> {
    let raw = std::fs::read_to_string(config_path()).ok()?;
    let cfg: BridgeConfig = serde_json::from_str(&raw).ok()?;
    (cfg.version >= 1 && cfg.token.len() >= 32 && !cfg.url.trim().is_empty()).then_some(cfg)
}

fn names_equal(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

/// True when the stored account is the panel owner and not a reseller / ACL / jailed login.
fn owner_role_ok(boot: &PanelBootstrap) -> bool {
    let role = boot.role.trim().to_ascii_lowercase();
    matches!(role.as_str(), "" | "user" | "admin" | "owner")
        && boot.parent_reseller.trim().is_empty()
        && boot.parent_owner.trim().is_empty()
}

/// Verify webmail admin credentials against the CPN panel owner account.
///
/// Allowed: panel owner only (bootstrap admin). Denied: users, resellers, ACL accounts.
pub fn verify_owner_admin_login(
    username: &str,
    password: &str,
    totp: &str,
) -> Result<(), DenyReason> {
    let username = username.trim();
    if username.is_empty()
        || password.is_empty()
        || username.chars().count() > MAX_USERNAME_CHARS
        || password.chars().count() > MAX_PASSWORD_CHARS
        || totp.chars().count() > MAX_TOTP_CHARS
        || username.chars().any(|c| c.is_control())
    {
        return Err(DenyReason::Invalid);
    }
    // Separate namespace so spraying webmail admin cannot lock the real panel login.
    let rl_key = format!("tachyon-admin:{}", username.to_ascii_lowercase());
    check_rate_limit(&rl_key).map_err(|_| DenyReason::RateLimited)?;

    let Some(boot) = load_bootstrap() else {
        register_rate_failure(&rl_key);
        return Err(DenyReason::NotOwner);
    };
    // Owner only: every non-bootstrap account (user, reseller, ACL sub-user) stops here.
    if !names_equal(&boot.username, username) {
        register_rate_failure(&rl_key);
        return Err(DenyReason::NotOwner);
    }
    if !owner_role_ok(&boot) {
        register_rate_failure(&rl_key);
        return Err(DenyReason::RoleNotAllowed);
    }
    if boot.disabled {
        return Err(DenyReason::Disabled);
    }
    if !verify_password(password, &boot.password_salt, &boot.password_hash) {
        register_rate_failure(&rl_key);
        return Err(DenyReason::BadPassword);
    }
    if boot.must_change_password {
        return Err(DenyReason::MustChangePassword);
    }

    let owner = boot.username.as_str();
    if totp_enabled_for(owner) {
        if totp.trim().is_empty() {
            return Err(DenyReason::MfaRequired);
        }
        match verify_mfa_challenge(owner, totp) {
            Ok(true) => {}
            Ok(false) => {
                register_rate_failure(&rl_key);
                return Err(DenyReason::MfaFailed);
            }
            Err(_) => return Err(DenyReason::MfaFailed),
        }
    } else if has_passkeys(owner) {
        return Err(DenyReason::PasskeyOnly);
    } else if boot.totp_required {
        // CPN would send this owner to MFA enrollment before full panel use.
        return Err(DenyReason::MfaEnrollRequired);
    }
    clear_rate_limit(&rl_key);
    Ok(())
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

/// Request must come straight from the local PHP hook (not proxied from the internet).
fn request_is_local_bridge_call(http: &HttpRequest, cfg: &BridgeConfig) -> bool {
    let Some(peer) = http.peer_addr() else {
        return false;
    };
    if !peer.ip().is_loopback() {
        return false;
    }
    // A reverse proxy in front of the panel adds these; the PHP hook never does.
    let h = http.headers();
    if h.contains_key("x-forwarded-for")
        || h.contains_key("x-real-ip")
        || h.contains_key("forwarded")
        || h.contains_key("x-forwarded-host")
    {
        return false;
    }
    let presented = h
        .get(BRIDGE_TOKEN_HEADER)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    !presented.is_empty() && constant_time_eq(presented, &cfg.token)
}

#[derive(Debug, Deserialize)]
pub struct VerifyBody {
    #[serde(default)]
    username: String,
    #[serde(default)]
    password: String,
    #[serde(default)]
    totp: String,
}

fn json_reply(status: actix_web::http::StatusCode, ok: bool) -> HttpResponse {
    HttpResponse::build(status)
        .content_type("application/json; charset=utf-8")
        .insert_header(("Cache-Control", "no-store"))
        .body(serde_json::json!({ "ok": ok }).to_string())
}

/// Loopback endpoint used by the webmail admin PHP hook.
#[post("/api/internal/tachyon-admin/verify")]
pub async fn tachyon_admin_verify(http: HttpRequest, body: web::Json<VerifyBody>) -> HttpResponse {
    let Some(cfg) = load_config() else {
        return json_reply(actix_web::http::StatusCode::NOT_FOUND, false);
    };
    if !request_is_local_bridge_call(&http, &cfg) {
        return json_reply(actix_web::http::StatusCode::FORBIDDEN, false);
    }
    let VerifyBody {
        username,
        password,
        totp,
    } = body.into_inner();
    let actor = if username.trim().is_empty() || username.chars().count() > MAX_USERNAME_CHARS {
        "anonymous".to_string()
    } else {
        username.trim().to_string()
    };
    // PBKDF2 is CPU heavy: keep it off the Actix worker threads.
    let outcome = web::block(move || verify_owner_admin_login(&username, &password, &totp)).await;
    let verdict = match outcome {
        Ok(v) => v,
        Err(_) => Err(DenyReason::Invalid),
    };
    match verdict {
        Ok(()) => {
            crate::panel_audit_http::audit_from_request(
                &http,
                &actor,
                "auth.webmail_admin.login",
                "tachyon-admin",
                true,
                "webmail admin sign-in via panel owner",
                AuditCategory::Auth,
                AuditSeverity::Notice,
            );
            json_reply(actix_web::http::StatusCode::OK, true)
        }
        Err(reason) => {
            crate::panel_audit_http::audit_from_request(
                &http,
                &actor,
                "auth.webmail_admin.fail",
                "tachyon-admin",
                false,
                reason.code(),
                AuditCategory::Auth,
                AuditSeverity::Warning,
            );
            json_reply(actix_web::http::StatusCode::UNAUTHORIZED, false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::{
        accounts_dir, default_password_policy, new_password_salt, setup_account,
        with_test_data_dir, write_account_file,
    };

    fn pw(parts: &[&str]) -> String {
        parts.concat()
    }

    fn owner_pw() -> String {
        pw(&["Owner", "Passw0rd", "xyz"])
    }

    fn make_owner() {
        setup_account(
            "ntowner",
            Some(&owner_pw()),
            false,
            "owner@example.com",
            default_password_policy(),
            "en",
        )
        .unwrap();
        // Setup marks MFA required; tests below enroll TOTP or clear the flag explicitly.
    }

    fn set_flags(f: impl FnOnce(&mut PanelBootstrap)) {
        let mut boot = load_bootstrap().unwrap();
        f(&mut boot);
        crate::account::persist_bootstrap(&boot).unwrap();
    }

    fn add_extra(name: &str, role: &str, parent_owner: &str, parent_reseller: &str) {
        let password = pw(&["Extra", "Passw0rd", "abc"]);
        let salt = new_password_salt();
        let boot = PanelBootstrap {
            schema_version: 1,
            username: name.into(),
            recovery_email: "x@example.com".into(),
            password_hash: crate::account::hash_password(&password, &salt),
            password_salt: salt,
            password_policy: default_password_policy(),
            language: "en".into(),
            created_at_unix: 1,
            must_change_password: false,
            totp_required: false,
            disabled: false,
            role: role.into(),
            parent_reseller: parent_reseller.into(),
            parent_owner: parent_owner.into(),
        };
        let path = accounts_dir().join(format!("{name}.json"));
        write_account_file(&path, &boot).unwrap();
    }

    #[test]
    fn owner_with_password_and_no_required_mfa_is_allowed() {
        with_test_data_dir(|| {
            make_owner();
            set_flags(|b| b.totp_required = false);
            assert_eq!(verify_owner_admin_login("ntowner", &owner_pw(), ""), Ok(()));
            assert_eq!(verify_owner_admin_login("NTOwner", &owner_pw(), ""), Ok(()));
        });
    }

    #[test]
    fn wrong_password_is_denied() {
        with_test_data_dir(|| {
            make_owner();
            set_flags(|b| b.totp_required = false);
            let bad = pw(&["Wrong", "Passw0rd", "123"]);
            assert_eq!(
                verify_owner_admin_login("ntowner", &bad, ""),
                Err(DenyReason::BadPassword)
            );
        });
    }

    #[test]
    fn normal_user_reseller_and_acl_accounts_are_denied() {
        with_test_data_dir(|| {
            make_owner();
            set_flags(|b| b.totp_required = false);
            add_extra("plainuser", "", "", "");
            add_extra("resell", "reseller", "", "");
            add_extra("aclsub", "subuser", "ntowner", "");
            add_extra("jailed", "", "", "resell");
            let extra = pw(&["Extra", "Passw0rd", "abc"]);
            for name in ["plainuser", "resell", "aclsub", "jailed"] {
                assert_eq!(
                    verify_owner_admin_login(name, &extra, ""),
                    Err(DenyReason::NotOwner),
                    "{name} must not reach webmail admin"
                );
            }
            // Owner password must not unlock an extra username either.
            assert_eq!(
                verify_owner_admin_login("aclsub", &owner_pw(), ""),
                Err(DenyReason::NotOwner)
            );
        });
    }

    #[test]
    fn owner_role_guard_rejects_reseller_and_acl_flags() {
        with_test_data_dir(|| {
            make_owner();
            set_flags(|b| {
                b.totp_required = false;
                b.role = "reseller".into();
            });
            assert_eq!(
                verify_owner_admin_login("ntowner", &owner_pw(), ""),
                Err(DenyReason::RoleNotAllowed)
            );
            set_flags(|b| {
                b.role = String::new();
                b.parent_owner = "someone".into();
            });
            assert_eq!(
                verify_owner_admin_login("ntowner", &owner_pw(), ""),
                Err(DenyReason::RoleNotAllowed)
            );
        });
    }

    #[test]
    fn disabled_or_must_change_owner_is_denied() {
        with_test_data_dir(|| {
            make_owner();
            set_flags(|b| {
                b.totp_required = false;
                b.disabled = true;
            });
            assert_eq!(
                verify_owner_admin_login("ntowner", &owner_pw(), ""),
                Err(DenyReason::Disabled)
            );
            set_flags(|b| {
                b.disabled = false;
                b.must_change_password = true;
            });
            assert_eq!(
                verify_owner_admin_login("ntowner", &owner_pw(), ""),
                Err(DenyReason::MustChangePassword)
            );
        });
    }

    #[test]
    fn owner_without_enrolled_mfa_is_denied_when_cpn_requires_it() {
        with_test_data_dir(|| {
            make_owner();
            set_flags(|b| b.totp_required = true);
            assert_eq!(
                verify_owner_admin_login("ntowner", &owner_pw(), ""),
                Err(DenyReason::MfaEnrollRequired)
            );
        });
    }

    #[test]
    fn owner_with_totp_needs_the_same_code_as_the_panel() {
        with_test_data_dir(|| {
            use crate::account_mfa::{begin_totp_enroll, confirm_totp_enroll};
            use crate::account_totp::{decode_totp_secret, totp_code_at};
            make_owner();
            let (secret_b32, _uri, _svg) = begin_totp_enroll("ntowner").unwrap();
            let secret = decode_totp_secret(&secret_b32).unwrap();
            let code = format!("{:06}", totp_code_at(&secret, crate::account::now_unix()));
            confirm_totp_enroll("ntowner", &code).unwrap();

            assert_eq!(
                verify_owner_admin_login("ntowner", &owner_pw(), ""),
                Err(DenyReason::MfaRequired)
            );
            assert_eq!(
                verify_owner_admin_login("ntowner", &owner_pw(), "000000"),
                Err(DenyReason::MfaFailed)
            );
            let ok = format!("{:06}", totp_code_at(&secret, crate::account::now_unix()));
            assert_eq!(
                verify_owner_admin_login("ntowner", &owner_pw(), &ok),
                Ok(())
            );
        });
    }

    #[test]
    fn repeated_failures_lock_the_bridge_without_locking_panel_login() {
        with_test_data_dir(|| {
            make_owner();
            set_flags(|b| b.totp_required = false);
            let bad = pw(&["Wrong", "Passw0rd", "123"]);
            for _ in 0..40 {
                let _ = verify_owner_admin_login("ntowner", &bad, "");
            }
            assert_eq!(
                verify_owner_admin_login("ntowner", &owner_pw(), ""),
                Err(DenyReason::RateLimited)
            );
            // Panel MFA/login rate namespace is separate.
            assert!(check_rate_limit("ntowner").is_ok());
        });
    }

    #[test]
    fn invalid_input_is_rejected_early() {
        with_test_data_dir(|| {
            assert_eq!(
                verify_owner_admin_login("", "x", ""),
                Err(DenyReason::Invalid)
            );
            assert_eq!(
                verify_owner_admin_login("ntowner", "", ""),
                Err(DenyReason::Invalid)
            );
            let long = "a".repeat(MAX_PASSWORD_CHARS + 1);
            assert_eq!(
                verify_owner_admin_login("ntowner", &long, ""),
                Err(DenyReason::Invalid)
            );
        });
    }

    #[test]
    fn deny_codes_are_stable_and_secret_free() {
        for r in [
            DenyReason::Invalid,
            DenyReason::RateLimited,
            DenyReason::NotOwner,
            DenyReason::RoleNotAllowed,
            DenyReason::Disabled,
            DenyReason::MustChangePassword,
            DenyReason::BadPassword,
            DenyReason::MfaRequired,
            DenyReason::MfaEnrollRequired,
            DenyReason::PasskeyOnly,
            DenyReason::MfaFailed,
        ] {
            assert!(!r.code().is_empty());
            assert!(!r.code().contains(' '));
        }
    }

    #[test]
    fn constant_time_eq_works() {
        assert!(constant_time_eq("abc", "abc"));
        assert!(!constant_time_eq("abc", "abd"));
        assert!(!constant_time_eq("abc", "abcd"));
    }
}
