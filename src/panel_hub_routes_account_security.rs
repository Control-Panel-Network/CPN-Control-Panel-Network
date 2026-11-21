//! Forced password-change and mandatory 2FA enrollment routes.

use crate::account::{hash_password, new_password_salt, password_meets_policy, write_account_file};
use crate::account_mfa::{
    begin_totp_enroll, confirm_totp_enroll, load_pending_secret, store_once_backup_codes,
};
use crate::account_mfa_enroll_token::{consume_mfa_enroll_token, peek_mfa_enroll_token};
use crate::account_mgmt::find_account;
use crate::account_security::{
    change_password_gate_main, enroll_mfa_gate_main, must_change_password, needs_mfa_enrollment,
    post_login_security_path,
};
use crate::auth_pages::{mfa_enroll_cli_confirm_html, mfa_enroll_cli_invalid_html};
use crate::installer::AppState;
use crate::panel_hub_http::{
    html_ok, login_redirect, redirect, require_panel_user, urlencoding_simple,
};
use crate::panel_pages::panel_shell;
use crate::panel_session::{
    clear_mfa_pending_cookie_headers, create_session_token, issue_session_cookie_headers,
    request_auth_cookie_port, session_secret,
};
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use std::sync::Arc;

fn gate_shell(user: &str, title: &str, main: &str) -> HttpResponse {
    html_ok(panel_shell(user, "account-security", title, main))
}

#[get("/account/security/change-password")]
pub async fn account_security_change_password_get(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !must_change_password(&user) {
        if let Some(path) = post_login_security_path(&user) {
            return redirect(path);
        }
        return redirect("/dashboard");
    }
    gate_shell(
        &user,
        "Change password",
        &change_password_gate_main(
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    )
}

#[derive(Debug, serde::Deserialize)]
pub struct ForcedPasswordForm {
    #[serde(default)]
    password: String,
    #[serde(default)]
    password_confirm: String,
}

#[post("/account/security/change-password")]
pub async fn account_security_change_password_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<ForcedPasswordForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !must_change_password(&user) {
        return redirect("/dashboard");
    }
    let password = form.password.trim();
    let confirm = form.password_confirm.trim();
    if password.is_empty() {
        return gate_shell(
            &user,
            "Change password",
            &change_password_gate_main(None, Some("Enter a new password.")),
        );
    }
    if password != confirm {
        return gate_shell(
            &user,
            "Change password",
            &change_password_gate_main(None, Some("Passwords do not match.")),
        );
    }
    match set_forced_password(&user, password) {
        Ok(()) => {
            if let Some(path) = post_login_security_path(&user) {
                return redirect(path);
            }
            redirect("/dashboard")
        }
        Err(error) => gate_shell(
            &user,
            "Change password",
            &change_password_gate_main(None, Some(&error)),
        ),
    }
}

fn set_forced_password(username: &str, password: &str) -> Result<(), String> {
    use crate::account::default_password_policy;
    let (mut boot, path) = find_account(username)?;
    let policy = default_password_policy();
    password_meets_policy(password, &policy)?;
    let salt = new_password_salt();
    boot.password_salt = salt.clone();
    boot.password_hash = hash_password(password, &salt);
    boot.password_policy = policy;
    boot.must_change_password = false;
    write_account_file(&path, &boot)?;
    let _ = crate::install_snappymail_prefs::sync_snappymail_admin_password(password);
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
pub struct MfaEnrollCliQuery {
    #[serde(default)]
    token: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct MfaEnrollCliForm {
    #[serde(default)]
    token: String,
}

fn request_secure(http: &HttpRequest) -> bool {
    crate::panel_session::request_https_from_headers(http)
}

fn enroll_cli_invalid(message: &str) -> HttpResponse {
    HttpResponse::BadRequest()
        .content_type("text/html; charset=utf-8")
        .body(mfa_enroll_cli_invalid_html(message))
}

fn enroll_cli_session_redirect(
    http: &HttpRequest,
    state: &AppState,
    username: &str,
) -> HttpResponse {
    let secret = session_secret(Some(&state.token));
    let secure = request_secure(http);
    let port = request_auth_cookie_port(http);
    let session = create_session_token(username, &secret);
    let location =
        post_login_security_path(username).unwrap_or("/account/users/modify?tab=security");
    let mut builder = HttpResponse::SeeOther();
    builder.append_header(("Location", location));
    for cookie in issue_session_cookie_headers(&session, secure, port) {
        builder.append_header(("Set-Cookie", cookie));
    }
    for cookie in clear_mfa_pending_cookie_headers(secure, port) {
        builder.append_header(("Set-Cookie", cookie));
    }
    for cookie in crate::login_next::clear_login_return_cookie_headers(secure, port) {
        builder.append_header(("Set-Cookie", cookie));
    }
    builder.finish()
}

/// Operator-issued link (GET/HEAD): peek only and show a confirm page.
/// Token is not consumed here so chat/email/antivirus HEAD or GET previews cannot burn it.
#[actix_web::route("/account/security/enroll-cli", method = "GET", method = "HEAD")]
pub async fn account_security_enroll_cli(
    _http: HttpRequest,
    query: web::Query<MfaEnrollCliQuery>,
) -> HttpResponse {
    let token = query.token.trim();
    let Some(username) = peek_mfa_enroll_token(token) else {
        return enroll_cli_invalid(
            "This enroll link is missing, invalid, expired, or already used.",
        );
    };
    if find_account(&username).is_err() {
        return enroll_cli_invalid("This enroll link is invalid or the account no longer exists.");
    }
    if crate::account_lifecycle::account_is_disabled(&username) {
        return enroll_cli_invalid(
            "This account is disabled. Ask an operator to enable it first.",
        );
    }
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(mfa_enroll_cli_confirm_html(token, &username))
}

/// Confirm step: consume the single-use token and open the MFA / Security UI.
#[post("/account/security/enroll-cli")]
pub async fn account_security_enroll_cli_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<MfaEnrollCliForm>,
) -> HttpResponse {
    let token = form.token.trim();
    if token.is_empty() || peek_mfa_enroll_token(token).is_none() {
        return enroll_cli_invalid(
            "This enroll link is missing, invalid, expired, or already used.",
        );
    }
    let username = match consume_mfa_enroll_token(token) {
        Ok(user) => user,
        Err(message) => return enroll_cli_invalid(&message),
    };
    if find_account(&username).is_err() {
        return enroll_cli_invalid("This enroll link is invalid or the account no longer exists.");
    }
    if crate::account_lifecycle::account_is_disabled(&username) {
        return enroll_cli_invalid(
            "This account is disabled. Ask an operator to enable it first.",
        );
    }
    enroll_cli_session_redirect(&http, state.get_ref(), &username)
}

#[get("/account/security/enroll-2fa")]
pub async fn account_security_enroll_2fa_get(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if must_change_password(&user) {
        return redirect("/account/security/change-password");
    }
    if !needs_mfa_enrollment(&user) {
        return redirect("/account/users/modify?tab=security");
    }
    let (secret, qr) = match load_pending_secret(&user) {
        Ok(secret) => {
            let uri = crate::account_totp::otpauth_uri("CPN Panel", &user, &secret);
            let qr = crate::account_totp::otpauth_qr_svg(&uri).ok();
            (Some(secret), qr)
        }
        Err(_) => (None, None),
    };
    gate_shell(
        &user,
        "Enable 2FA",
        &enroll_mfa_gate_main(
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
            secret.as_deref(),
            qr.as_deref(),
            None,
        ),
    )
}

/// GET on the POST-only begin path (browser refresh) must not fall through to the
/// installer SPA catch-all.
#[get("/account/security/enroll-2fa/begin")]
pub async fn account_security_enroll_2fa_begin_get() -> HttpResponse {
    redirect("/account/security/enroll-2fa")
}

#[post("/account/security/enroll-2fa/begin")]
pub async fn account_security_enroll_2fa_begin(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if must_change_password(&user) {
        return redirect("/account/security/change-password");
    }
    match begin_totp_enroll(&user) {
        // PRG: keep the address bar on the GET enroll page so refresh stays on MFA UI.
        Ok((_secret, _uri, _svg)) => redirect("/account/security/enroll-2fa"),
        Err(error) => redirect(&format!(
            "/account/security/enroll-2fa?error={}",
            urlencoding_simple(&error)
        )),
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct EnrollCodeForm {
    #[serde(default)]
    code: String,
}

#[post("/account/security/enroll-2fa/confirm")]
pub async fn account_security_enroll_2fa_confirm(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<EnrollCodeForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if must_change_password(&user) {
        return redirect("/account/security/change-password");
    }
    match confirm_totp_enroll(&user, &form.code) {
        Ok(backup_codes) => {
            let _ = store_once_backup_codes(&user, &backup_codes);
            redirect(&format!(
                "/account/users/modify?tab=security&notice={}",
                urlencoding_simple(
                    "Two-factor authentication is enabled. Store your backup codes."
                )
            ))
        }
        Err(error) => {
            let (secret, qr) = match load_pending_secret(&user) {
                Ok(secret) => {
                    let uri = crate::account_totp::otpauth_uri("CPN Panel", &user, &secret);
                    let qr = crate::account_totp::otpauth_qr_svg(&uri).ok();
                    (Some(secret), qr)
                }
                Err(_) => (None, None),
            };
            gate_shell(
                &user,
                "Enable 2FA",
                &enroll_mfa_gate_main(None, Some(&error), secret.as_deref(), qr.as_deref(), None),
            )
        }
    }
}
