//! Single sign-on for the webmail admin page from a CPN panel owner session.
//!
//! The webmail PHP hook forwards only the `cpn_panel_session*` cookie(s) of the browser
//! request (same host as the panel) to this loopback-only endpoint. The panel verifies
//! the signed session exactly as for any panel request and admits the webmail admin only
//! when that session belongs to the panel owner (bootstrap admin). Normal users,
//! resellers, and ACL accounts are refused even with a valid panel session. No password
//! or authenticator code is involved, and nothing secret is logged.

use crate::account::{PanelBootstrap, load_bootstrap};
use crate::installer::AppState;
use crate::panel_audit_types::{AuditCategory, AuditSeverity};
use crate::panel_session::{SESSION_COOKIE, session_secret, verify_session_token};
use crate::tachyon_admin_bridge::{
    DenyReason, json_reply, load_config, names_equal, owner_role_ok, request_is_local_bridge_call,
};
use actix_web::{HttpRequest, HttpResponse, post, web};
use serde::Deserialize;
use std::sync::Arc;

const MAX_COOKIE_CHARS: usize = 8192;

/// Panel session cookie names the webmail hook may forward: `cpn_panel_session` or the
/// port-scoped lab form `cpn_panel_session_<port>`.
fn is_session_cookie_name(name: &str) -> bool {
    match name.strip_prefix(SESSION_COOKIE) {
        Some("") => true,
        Some(rest) => rest.strip_prefix('_').is_some_and(|p| {
            !p.is_empty() && p.len() <= 5 && p.bytes().all(|b| b.is_ascii_digit())
        }),
        None => false,
    }
}

/// Decide whether a forwarded cookie jar holds a valid panel session of the panel owner.
///
/// The webmail request arrives on the internal webmail listener, so its `Host` says nothing
/// about the panel port the browser used. Instead of guessing the cookie name from the host,
/// every forwarded panel session cookie is verified (each is HMAC signed) and the first one
/// that belongs to the owner wins.
///
/// `Err(DenyReason::Invalid)` means "no usable panel session" (anonymous visitor).
pub fn owner_from_session_cookie(cookie_header: &str, secret: &str) -> Result<String, DenyReason> {
    if cookie_header.is_empty()
        || cookie_header.chars().count() > MAX_COOKIE_CHARS
        || cookie_header.chars().any(|c| c.is_control())
    {
        return Err(DenyReason::Invalid);
    }
    let mut verdict = Err(DenyReason::Invalid);
    for part in cookie_header.split(';') {
        let Some((name, value)) = part.trim().split_once('=') else {
            continue;
        };
        let value = value.trim();
        if value.is_empty() || !is_session_cookie_name(name.trim()) {
            continue;
        }
        let Some(username) = verify_session_token(value, secret) else {
            continue;
        };
        let Some(boot) = load_bootstrap() else {
            verdict = Err(DenyReason::NotOwner);
            continue;
        };
        match owner_session_ok(&boot, &username) {
            Ok(()) => return Ok(boot.username),
            Err(reason) => verdict = Err(reason),
        }
    }
    verdict
}
/// Owner checks that apply once the session token itself is valid.
pub fn owner_session_ok(boot: &PanelBootstrap, session_user: &str) -> Result<(), DenyReason> {
    if !names_equal(&boot.username, session_user) {
        return Err(DenyReason::NotOwner);
    }
    if !owner_role_ok(boot) {
        return Err(DenyReason::RoleNotAllowed);
    }
    if boot.disabled || crate::account_lifecycle::account_is_disabled(&boot.username) {
        return Err(DenyReason::Disabled);
    }
    if boot.must_change_password {
        return Err(DenyReason::MustChangePassword);
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct SessionBody {
    #[serde(default)]
    cookie: String,
    /// Ignored (older hooks sent it); kept so their requests still deserialize.
    #[serde(default)]
    #[allow(dead_code)]
    host: String,
}

/// Loopback endpoint used by the webmail admin PHP hook for session reuse.
#[post("/api/internal/tachyon-admin/session")]
pub async fn tachyon_admin_session(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<SessionBody>,
) -> HttpResponse {
    let Some(cfg) = load_config() else {
        return json_reply(actix_web::http::StatusCode::NOT_FOUND, false);
    };
    if !request_is_local_bridge_call(&http, &cfg) {
        return json_reply(actix_web::http::StatusCode::FORBIDDEN, false);
    }
    let SessionBody { cookie, .. } = body.into_inner();
    let secret = session_secret(Some(&state.token));
    let verdict = web::block(move || owner_from_session_cookie(&cookie, &secret)).await;
    match verdict {
        Ok(Ok(owner)) => {
            crate::panel_audit_http::audit_from_request(
                &http,
                &owner,
                "auth.webmail_admin.sso",
                "tachyon-admin",
                true,
                "webmail admin sign-in via panel owner session",
                AuditCategory::Auth,
                AuditSeverity::Notice,
            );
            json_reply(actix_web::http::StatusCode::OK, true)
        }
        // Anonymous visitors (no session) are normal: no audit noise.
        Ok(Err(DenyReason::Invalid)) | Err(_) => {
            json_reply(actix_web::http::StatusCode::UNAUTHORIZED, false)
        }
        Ok(Err(reason)) => {
            crate::panel_audit_http::audit_from_request(
                &http,
                "panel-session",
                "auth.webmail_admin.sso_denied",
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
mod tests;
