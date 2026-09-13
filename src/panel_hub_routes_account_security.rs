//! Forced password-change and mandatory 2FA enrollment routes.

use crate::account::{hash_password, new_password_salt, password_meets_policy, write_account_file};
use crate::account_mfa::{begin_totp_enroll, confirm_totp_enroll, load_pending_secret};
use crate::account_mgmt::find_account;
use crate::account_security::{
    change_password_gate_main, enroll_mfa_gate_main, must_change_password, needs_mfa_enrollment,
    post_login_security_path,
};
use crate::installer::AppState;
use crate::panel_hub_http::{
    html_ok, login_redirect, redirect, require_panel_user, urlencoding_simple,
};
use crate::panel_pages::panel_shell;
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
    let (mut boot, path) = find_account(username)?;
    password_meets_policy(password, &boot.password_policy)?;
    let salt = new_password_salt();
    boot.password_salt = salt.clone();
    boot.password_hash = hash_password(password, &salt);
    boot.must_change_password = false;
    write_account_file(&path, &boot)
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
        return redirect("/dashboard");
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
        Ok(backup_codes) => gate_shell(
            &user,
            "Enable 2FA",
            &enroll_mfa_gate_main(
                Some("Two-factor authentication is enabled."),
                None,
                None,
                None,
                Some(&backup_codes),
            ),
        ),
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
