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

impl DenyReason {
    /// Short hint safe to return to the webmail hook (never a secret). Only reasons that are
    /// reached after the password was verified (or that reveal nothing about accounts) are
    /// exposed, so the form can say what is missing instead of a generic failure.
    pub fn client_hint(self) -> Option<&'static str> {
        match self {
            DenyReason::MfaRequired
            | DenyReason::MfaFailed
            | DenyReason::MfaEnrollRequired
            | DenyReason::PasskeyOnly
            | DenyReason::MustChangePassword
            | DenyReason::RateLimited => Some(self.code()),
            _ => None,
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

pub(crate) fn names_equal(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

/// True when the stored account is the panel owner and not a reseller / ACL / jailed login.
pub(crate) fn owner_role_ok(boot: &PanelBootstrap) -> bool {
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
pub(crate) fn request_is_local_bridge_call(http: &HttpRequest, cfg: &BridgeConfig) -> bool {
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

pub(crate) fn json_reply(status: actix_web::http::StatusCode, ok: bool) -> HttpResponse {
    json_reply_hint(status, ok, None)
}

fn json_reply_hint(
    status: actix_web::http::StatusCode,
    ok: bool,
    reason: Option<&str>,
) -> HttpResponse {
    let body = match reason {
        Some(r) => serde_json::json!({ "ok": ok, "reason": r }),
        None => serde_json::json!({ "ok": ok }),
    };
    HttpResponse::build(status)
        .content_type("application/json; charset=utf-8")
        .insert_header(("Cache-Control", "no-store"))
        .body(body.to_string())
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
            json_reply_hint(
                actix_web::http::StatusCode::UNAUTHORIZED,
                false,
                reason.client_hint(),
            )
        }
    }
}

#[cfg(test)]
mod tests;
