//! Forgot-password and reset-password HTTP handlers.

use crate::account::password_meets_policy;
use crate::account_mgmt::{find_account, reset_account_password};
use crate::account_password_reset::{
    check_and_record_forgot_rate, consume_reset_token, create_reset_token, find_account_for_reset,
    invalidate_tokens_for_user, peek_reset_token,
};
use crate::auth_pages::{
    forgot_password_ack_html, forgot_password_html, reset_password_html,
    reset_password_invalid_html,
};
use crate::http_helpers::{enrich_status, panel_login_url_for};
use crate::installer::AppState;
use crate::mail_outbound::{build_password_reset_email, send_mail};
use crate::model::InstallerStatus;
use crate::panel_session::{
    clear_mfa_pending_cookie_header, create_session_token, session_cookie_header, session_secret,
};
use actix_web::{HttpRequest, HttpResponse, post, web};
use std::sync::Arc;

#[derive(Debug, serde::Deserialize)]
struct ForgotPasswordForm {
    /// Preferred single field: username or email.
    #[serde(default)]
    account: String,
    /// Legacy fields kept for older clients.
    #[serde(default)]
    username: String,
    #[serde(default)]
    email: String,
}

fn client_key_from_request(http: &HttpRequest) -> Option<String> {
    // Use the peer socket address only (not X-Forwarded-For) so rate-limit keys
    // are not derived from attacker-controlled header text (CodeQL allocation/log).
    http.peer_addr().map(|addr| addr.ip().to_string())
}

fn panel_base_url_for(status: &InstallerStatus) -> String {
    if let Some(base) = status
        .public_base_url
        .as_ref()
        .filter(|v| !v.trim().is_empty())
    {
        return base.trim_end_matches('/').to_string();
    }
    let host_hint = status
        .environment
        .as_ref()
        .and_then(|env_info| env_info.addresses.first())
        .map(String::as_str);
    crate::panel_network::public_base_url(status.listen_port, host_hint)
}

fn forgot_ack_response() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(forgot_password_ack_html())
}

fn request_secure(http: &HttpRequest) -> bool {
    crate::panel_session::request_https_from_headers(http)
}

#[actix_web::route("/forgot-password", method = "GET", method = "HEAD")]
pub async fn forgot_password_page() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(forgot_password_html())
}

#[post("/forgot-password")]
pub async fn forgot_password_submit(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<ForgotPasswordForm>,
) -> HttpResponse {
    // Always return the same ack page: no account enumeration.
    let identifier = {
        let account = form.account.trim();
        if !account.is_empty() {
            account.to_string()
        } else if !form.username.trim().is_empty() {
            form.username.trim().to_string()
        } else {
            form.email.trim().to_string()
        }
    };

    let client_key = client_key_from_request(&http);
    // Record rate limit even for unknown accounts so probes cannot spray mail.
    if check_and_record_forgot_rate(&identifier, client_key.as_deref()).is_err() {
        return forgot_ack_response();
    }

    if let Some((username, recovery_email)) = find_account_for_reset(&identifier) {
        let status = state
            .status
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        let status = enrich_status(status, &state.token);
        let base = panel_base_url_for(&status);
        let login_url = panel_login_url_for(&status, &state.token);
        if let Ok(raw_token) = create_reset_token(&username) {
            let reset_url = format!("{base}/reset-password?token={raw_token}");
            let mut message = build_password_reset_email(&reset_url, &login_url);
            message.to = recovery_email;
            let _ = send_mail(&message);
        }
    }

    forgot_ack_response()
}

#[derive(Debug, serde::Deserialize)]
struct ResetPasswordQuery {
    #[serde(default)]
    token: String,
}

#[derive(Debug, serde::Deserialize)]
struct ResetPasswordForm {
    #[serde(default)]
    token: String,
    #[serde(default)]
    password: String,
    #[serde(default)]
    password_confirm: String,
}

#[actix_web::route("/reset-password", method = "GET", method = "HEAD")]
pub async fn reset_password_page(query: web::Query<ResetPasswordQuery>) -> HttpResponse {
    let token = query.token.trim();
    if token.is_empty() || peek_reset_token(token).is_none() {
        return HttpResponse::BadRequest()
            .content_type("text/html; charset=utf-8")
            .body(reset_password_invalid_html(
                "This reset link is missing, invalid, or already used.",
            ));
    }
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(reset_password_html(token, None))
}

#[post("/reset-password")]
pub async fn reset_password_submit(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<ResetPasswordForm>,
) -> HttpResponse {
    let token = form.token.trim().to_string();
    if token.is_empty() || peek_reset_token(&token).is_none() {
        return HttpResponse::BadRequest()
            .content_type("text/html; charset=utf-8")
            .body(reset_password_invalid_html(
                "This reset link is missing, invalid, or already used.",
            ));
    }

    let password = form.password.as_str();
    let confirm = form.password_confirm.as_str();
    if password != confirm {
        return HttpResponse::BadRequest()
            .content_type("text/html; charset=utf-8")
            .body(reset_password_html(
                &token,
                Some("New password and confirmation do not match."),
            ));
    }

    let username = match peek_reset_token(&token) {
        Some(user) => user,
        None => {
            return HttpResponse::BadRequest()
                .content_type("text/html; charset=utf-8")
                .body(reset_password_invalid_html(
                    "This reset link is missing, invalid, or already used.",
                ));
        }
    };

    // Validate against the account policy before consuming the token.
    let Ok((boot, _)) = find_account(&username) else {
        return HttpResponse::BadRequest()
            .content_type("text/html; charset=utf-8")
            .body(reset_password_invalid_html(
                "This reset link is missing, invalid, or already used.",
            ));
    };
    if let Err(error) = password_meets_policy(password, &boot.password_policy) {
        return HttpResponse::BadRequest()
            .content_type("text/html; charset=utf-8")
            .body(reset_password_html(&token, Some(&error)));
    }

    let username = match consume_reset_token(&token) {
        Ok(user) => user,
        Err(error) => {
            return HttpResponse::BadRequest()
                .content_type("text/html; charset=utf-8")
                .body(reset_password_invalid_html(&error));
        }
    };

    if reset_account_password(&username, Some(password), false).is_err() {
        // Token already consumed; ask for a new link rather than leaking details.
        return HttpResponse::BadRequest()
            .content_type("text/html; charset=utf-8")
            .body(reset_password_invalid_html(
                "Could not update the password. Request a new reset link.",
            ));
    }

    invalidate_tokens_for_user(&username);

    let secret = session_secret(Some(&state.token));
    let secure = request_secure(&http);
    let session = create_session_token(&username, &secret);
    HttpResponse::SeeOther()
        .append_header(("Location", "/dashboard"))
        .append_header(("Set-Cookie", session_cookie_header(&session, secure)))
        .append_header(("Set-Cookie", clear_mfa_pending_cookie_header(secure)))
        .finish()
}
