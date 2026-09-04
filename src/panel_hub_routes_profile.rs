//! Self-service profile routes and Modify User (self-edit + admin tools).

use crate::account::verify_password;
use crate::account_mfa::{
    begin_totp_enroll, confirm_totp_enroll, disable_totp, load_pending_secret,
};
use crate::account_mgmt::{
    change_own_password, find_account, rename_own_account, update_own_profile,
};
use crate::account_totp::otpauth_qr_svg;
use crate::installer::AppState;
use crate::panel_hub_http::{html_ok, login_redirect, redirect_notice, require_panel_user};
use crate::panel_hub_pages_profile::{users_modify_page, users_profile_page};
use crate::panel_pages::panel_shell;
use crate::panel_session::{create_session_token, session_cookie_header, session_secret};
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use std::sync::Arc;

fn parse_flag(raw: &str) -> bool {
    matches!(
        raw.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn request_secure(http: &HttpRequest) -> bool {
    crate::panel_session::request_https_from_headers(http)
}

fn modify_html(
    user: &str,
    notice: Option<&str>,
    error: Option<&str>,
    enroll_secret: Option<&str>,
    enroll_qr_svg: Option<&str>,
    backup_codes: Option<&[String]>,
    generated_password: Option<&str>,
) -> HttpResponse {
    html_ok(panel_shell(
        user,
        "users",
        "Modify User",
        &users_modify_page(
            user,
            notice,
            error,
            enroll_secret,
            enroll_qr_svg,
            backup_codes,
            generated_password,
        ),
    ))
}

#[derive(Debug, serde::Deserialize)]
pub struct ProfileDetailsForm {
    #[serde(default)]
    username: String,
    #[serde(default)]
    recovery_email: String,
    #[serde(default)]
    language: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct ProfilePasswordForm {
    #[serde(default)]
    current_password: String,
    #[serde(default)]
    password: String,
    #[serde(default)]
    generate: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct ProfileTotpCodeForm {
    #[serde(default)]
    code: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct ProfileTotpDisableForm {
    #[serde(default)]
    current_password: String,
    #[serde(default)]
    code: String,
}

#[get("/account/users/profile")]
pub async fn users_profile_route(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    html_ok(panel_shell(
        &user,
        "users",
        "View Profile",
        &users_profile_page(
            &user,
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
            None,
            None,
            None,
            None,
        ),
    ))
}

#[get("/account/users/modify")]
pub async fn users_modify_get(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    let mut enroll_secret = None;
    let mut enroll_qr = None;
    if query.get("enroll").map(String::as_str) == Some("1")
        && let Ok(secret) = load_pending_secret(&user)
    {
        let uri = crate::account_totp::otpauth_uri("CPN Panel", &user, &secret);
        if let Ok(svg) = otpauth_qr_svg(&uri) {
            enroll_secret = Some(secret);
            enroll_qr = Some(svg);
        }
    }
    modify_html(
        &user,
        query.get("notice").map(String::as_str),
        query.get("error").map(String::as_str),
        enroll_secret.as_deref(),
        enroll_qr.as_deref(),
        None,
        None,
    )
}

#[post("/account/users/profile/details")]
pub async fn users_profile_details_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<ProfileDetailsForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    let rename_needed = !form.username.trim().eq_ignore_ascii_case(user.trim());
    if let Err(error) = update_own_profile(
        &user,
        Some(form.recovery_email.as_str()),
        Some(form.language.as_str()),
    ) {
        return redirect_notice("/account/users/modify", None, Some(&error));
    }
    let session_user = if rename_needed {
        match rename_own_account(&user, &form.username) {
            Ok(public) => public.username,
            Err(error) => {
                return redirect_notice("/account/users/modify", None, Some(&error));
            }
        }
    } else {
        user.clone()
    };
    let secret = session_secret(Some(&state.token));
    let token = create_session_token(&session_user, &secret);
    let secure = request_secure(&http);
    HttpResponse::SeeOther()
        .append_header(("Location", "/account/users/profile?notice=Profile+updated"))
        .append_header(("Set-Cookie", session_cookie_header(&token, secure)))
        .finish()
}

#[post("/account/users/profile/password")]
pub async fn users_profile_password_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<ProfilePasswordForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    let generate = parse_flag(&form.generate) || form.password.trim().is_empty();
    let password = if generate {
        None
    } else {
        Some(form.password.as_str())
    };
    match change_own_password(&user, &form.current_password, password, generate) {
        Ok(result) => {
            if result.generated_password.is_some() {
                modify_html(
                    &user,
                    Some("Password updated"),
                    None,
                    None,
                    None,
                    None,
                    result.generated_password.as_deref(),
                )
            } else {
                redirect_notice("/account/users/profile", Some("Password updated"), None)
            }
        }
        Err(error) => redirect_notice("/account/users/modify", None, Some(&error)),
    }
}

#[post("/account/users/profile/totp/begin")]
pub async fn users_profile_totp_begin(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    match begin_totp_enroll(&user) {
        Ok((secret, _uri, svg)) => modify_html(
            &user,
            Some("Scan the QR and confirm with a code"),
            None,
            Some(&secret),
            Some(&svg),
            None,
            None,
        ),
        Err(error) => redirect_notice("/account/users/modify", None, Some(&error)),
    }
}

#[post("/account/users/profile/totp/confirm")]
pub async fn users_profile_totp_confirm(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<ProfileTotpCodeForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    match confirm_totp_enroll(&user, &form.code) {
        Ok(codes) => modify_html(
            &user,
            Some("TOTP enabled. Store your backup codes."),
            None,
            None,
            None,
            Some(&codes),
            None,
        ),
        Err(error) => redirect_notice("/account/users/modify?enroll=1", None, Some(&error)),
    }
}

#[post("/account/users/profile/totp/disable")]
pub async fn users_profile_totp_disable(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<ProfileTotpDisableForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    let Ok((boot, _)) = find_account(&user) else {
        return redirect_notice("/account/users/modify", None, Some("Account not found"));
    };
    if !verify_password(
        &form.current_password,
        &boot.password_salt,
        &boot.password_hash,
    ) {
        return redirect_notice(
            "/account/users/modify",
            None,
            Some("Current password is incorrect"),
        );
    }
    match disable_totp(&user, &form.code) {
        Ok(()) => redirect_notice("/account/users/profile", Some("TOTP disabled"), None),
        Err(error) => redirect_notice("/account/users/modify", None, Some(&error)),
    }
}
