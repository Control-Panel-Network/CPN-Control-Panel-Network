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
use crate::panel_session::{
    auth_cookie_port_from_host, read_session_cookie, session_secret, verify_session_token,
};
use crate::tachyon_admin_bridge::{
    DenyReason, json_reply, load_config, names_equal, owner_role_ok, request_is_local_bridge_call,
};
use actix_web::{HttpRequest, HttpResponse, post, web};
use serde::Deserialize;
use std::sync::Arc;

const MAX_COOKIE_CHARS: usize = 8192;
const MAX_HOST_CHARS: usize = 255;

/// Decide whether a forwarded cookie jar is a valid panel session of the panel owner.
///
/// `Err(DenyReason::Invalid)` means "no usable panel session" (anonymous visitor).
pub fn owner_from_session_cookie(
    cookie_header: &str,
    host: &str,
    secret: &str,
) -> Result<String, DenyReason> {
    if cookie_header.is_empty()
        || cookie_header.chars().count() > MAX_COOKIE_CHARS
        || host.chars().count() > MAX_HOST_CHARS
        || cookie_header.chars().any(|c| c.is_control())
    {
        return Err(DenyReason::Invalid);
    }
    let port = auth_cookie_port_from_host(host);
    let token = read_session_cookie(Some(cookie_header), port).ok_or(DenyReason::Invalid)?;
    let username = verify_session_token(&token, secret).ok_or(DenyReason::Invalid)?;
    let boot = load_bootstrap().ok_or(DenyReason::NotOwner)?;
    owner_session_ok(&boot, &username)?;
    Ok(boot.username)
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
    #[serde(default)]
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
    let SessionBody { cookie, host } = body.into_inner();
    let secret = session_secret(Some(&state.token));
    let verdict = web::block(move || owner_from_session_cookie(&cookie, &host, &secret)).await;
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
