//! Login, panel dashboard, logout, and first-account setup HTTP handlers.
//! Forgot/reset password live in `auth_password_reset_api`.

use crate::account::{
    hash_password, password_hash_needs_upgrade, verify_password, write_account_file,
};
use crate::account_mfa::totp_enabled_for;
use crate::account_mgmt::find_account;
use crate::account_passkeys::has_passkeys;
use crate::auth_pages::{
    MfaPageOptions, installer_token_required_html, panel_login_html, panel_mfa_html,
};
use crate::http_helpers::{
    authorized_request, enrich_status, install_finished, normalize_language, panel_account_ready,
    panel_login_url_for, smtp_status_public, token_matches,
};
use crate::installer::AppState;
use crate::login_next::{
    clear_login_return_cookie_header, first_safe_next, login_location, login_return_cookie_header,
    mfa_location, post_login_location, read_login_return_cookie, referer_return_path,
    request_return_path,
};
use crate::login_service_gate::{evaluate_login_services, login_services_ready};
use crate::mail_outbound::{build_setup_confirmation, send_mail_with_settings};
use crate::model::{AccountSetupRequest, OptionalTokenQuery, TokenQuery};
use crate::panel_dashboard::panel_dashboard_html;
use crate::panel_session::{
    clear_mfa_pending_cookie_header, clear_session_cookie_header, create_mfa_pending_token,
    create_session_token, mfa_pending_cookie_header, read_mfa_pending_cookie, read_session_cookie,
    session_cookie_header, session_secret, verify_mfa_pending_token, verify_session_token,
};
use crate::postfix_fallback::ensure_postfix_default;
use crate::smtp_settings::persist_smtp;
use crate::smtp_settings::validate_smtp_input;
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use std::sync::Arc;

#[get("/cpn-logo.png")]
pub async fn cpn_logo() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("image/png")
        .insert_header(("Cache-Control", "public, max-age=86400"))
        .body(include_bytes!("../installer-ui/src/assets/cpn-logo.png").as_slice())
}

#[actix_web::route("/favicon.ico", method = "GET", method = "HEAD")]
pub async fn favicon_ico() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("image/x-icon")
        .insert_header(("Cache-Control", "public, max-age=86400"))
        .body(include_bytes!("../installer-ui/src/assets/favicon.ico").as_slice())
}

#[actix_web::route("/favicon.svg", method = "GET", method = "HEAD")]
pub async fn favicon_svg() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("image/svg+xml")
        .insert_header(("Cache-Control", "public, max-age=86400"))
        .body(include_bytes!("../installer-ui/src/assets/favicon.svg").as_slice())
}

#[actix_web::route("/apple-touch-icon.png", method = "GET", method = "HEAD")]
pub async fn apple_touch_icon() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("image/png")
        .insert_header(("Cache-Control", "public, max-age=86400"))
        .body(include_bytes!("../installer-ui/src/assets/apple-touch-icon.png").as_slice())
}

#[actix_web::route("/cpn-brand-mark.svg", method = "GET", method = "HEAD")]
pub async fn cpn_brand_mark() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("image/svg+xml")
        .insert_header(("Cache-Control", "public, max-age=86400"))
        .body(include_bytes!("../installer-ui/src/assets/cpn-brand-mark.svg").as_slice())
}

fn login_error_message(locale: &str) -> &'static str {
    match locale {
        "es" => "Usuario o contraseña no válidos.",
        "nb" => "Ugyldig brukernavn eller passord.",
        _ => "Invalid username or password.",
    }
}

fn request_secure(http: &HttpRequest) -> bool {
    crate::panel_session::request_https_from_headers(http)
}

pub fn panel_user_from_request(state: &AppState, http: &HttpRequest) -> Option<String> {
    if let Some(auth) = http
        .headers()
        .get(actix_web::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
    {
        let raw = auth.trim();
        if raw.len() > 7 && raw[..7].eq_ignore_ascii_case("bearer ") {
            let bearer = raw[7..].trim();
            if bearer.starts_with(crate::panel_api_tokens::TOKEN_PREFIX)
                && let Some((username, _scopes)) =
                    crate::panel_api_tokens::authenticate_bearer(bearer)
            {
                return Some(username);
            }
        }
    }
    let cookie = http
        .headers()
        .get(actix_web::http::header::COOKIE)
        .and_then(|value| value.to_str().ok());
    let token = read_session_cookie(cookie)?;
    let secret = session_secret(Some(&state.token));
    verify_session_token(&token, &secret)
}

fn maybe_upgrade_password_hash(
    path: &std::path::Path,
    boot: &mut crate::account::PanelBootstrap,
    password: &str,
) {
    if !password_hash_needs_upgrade(&boot.password_hash) {
        return;
    }
    boot.password_hash = hash_password(password, &boot.password_salt);
    let _ = write_account_file(path, boot);
}

pub(crate) fn login_success_response(
    http: &HttpRequest,
    token: &str,
    username: &str,
    next: Option<&str>,
) -> HttpResponse {
    let peer = http.peer_addr().map(|a| a.ip().to_string());
    crate::panel_firewall_store::record_admin_login_ip(username, peer.as_deref());
    let _ = crate::panel_firewall_store::ensure_protected_seeds(
        {
            let host = crate::panel_host_info::host_sidebar_info();
            if host.ip == "Unavailable" {
                None
            } else {
                Some(host.ip)
            }
        }
        .as_deref(),
        peer.as_deref(),
    );
    let secure = request_secure(http);
    let location = crate::account_security::post_login_security_path(username)
        .map(str::to_string)
        .unwrap_or_else(|| post_login_location(next));
    HttpResponse::SeeOther()
        .append_header(("Location", location))
        .append_header(("Set-Cookie", session_cookie_header(token, secure)))
        .append_header(("Set-Cookie", clear_mfa_pending_cookie_header(secure)))
        .append_header(("Set-Cookie", clear_login_return_cookie_header(secure)))
        .finish()
}

fn resolve_next_from_request(
    http: &HttpRequest,
    form_next: Option<&str>,
    query_next: Option<&str>,
) -> Option<String> {
    let cookie = http
        .headers()
        .get(actix_web::http::header::COOKIE)
        .and_then(|value| value.to_str().ok());
    let from_cookie = read_login_return_cookie(cookie);
    first_safe_next(&[form_next, query_next, from_cookie.as_deref()])
}

#[actix_web::route("/login", method = "GET", method = "HEAD")]
pub async fn login_page(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<OptionalTokenQuery>,
) -> HttpResponse {
    let status = state
        .status
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    let allow_login = install_finished(&status)
        || panel_account_ready(&status)
        || token_matches(&state, query.token.as_deref());
    if !allow_login {
        return HttpResponse::Unauthorized()
            .content_type("text/html; charset=utf-8")
            .body(installer_token_required_html());
    }
    let payload = enrich_status(status, &state.token);
    let next = first_safe_next(&[query.next.as_deref()]);
    if let Some(session_user) = panel_user_from_request(&state, &http) {
        return redirect_authed_after_login(&session_user, next.as_deref());
    }
    let secure = request_secure(&http);
    let mut builder = HttpResponse::Ok();
    builder.content_type("text/html; charset=utf-8");
    if let Some(ref path) = next
        && let Some(cookie) = login_return_cookie_header(path, secure)
    {
        builder.append_header(("Set-Cookie", cookie));
    } else if next.is_none() {
        builder.append_header(("Set-Cookie", clear_login_return_cookie_header(secure)));
    }
    builder.body(panel_login_html(&payload, None, next.as_deref()))
}

#[get("/api/login/services")]
pub async fn login_services_status() -> HttpResponse {
    HttpResponse::Ok().json(evaluate_login_services())
}

fn services_not_ready_login(
    payload: &crate::model::InstallerStatus,
    next: Option<&str>,
) -> HttpResponse {
    let gate = evaluate_login_services();
    HttpResponse::ServiceUnavailable()
        .content_type("text/html; charset=utf-8")
        .body(crate::auth_pages::panel_login_html_with_gate(
            payload,
            Some(gate.message.as_str()),
            next,
            &gate,
        ))
}

#[derive(Debug, serde::Deserialize)]
struct LoginForm {
    #[serde(default)]
    username: String,
    #[serde(default)]
    password: String,
    #[serde(default)]
    remember_me: String,
    #[serde(default)]
    next: String,
}

#[post("/login")]
pub async fn login_submit(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<OptionalTokenQuery>,
    form: web::Form<LoginForm>,
) -> HttpResponse {
    let status = state
        .status
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    let allow_login = install_finished(&status)
        || panel_account_ready(&status)
        || token_matches(&state, query.token.as_deref());
    if !allow_login {
        return HttpResponse::Unauthorized()
            .content_type("text/html; charset=utf-8")
            .body(installer_token_required_html());
    }

    let payload = enrich_status(status, &state.token);
    let next = resolve_next_from_request(&http, Some(form.next.as_str()), query.next.as_deref());
    if !login_services_ready() {
        return services_not_ready_login(&payload, next.as_deref());
    }

    let locale = payload.language.as_str();
    let username = form.username.trim();
    let password = form.password.as_str();
    let _remember_me = form.remember_me.trim() == "1";

    let authed = if username.is_empty() || password.is_empty() {
        None
    } else {
        match find_account(username) {
            Ok((mut boot, path)) => {
                if verify_password(password, &boot.password_salt, &boot.password_hash) {
                    maybe_upgrade_password_hash(&path, &mut boot, password);
                    Some(boot.username)
                } else {
                    None
                }
            }
            Err(_) => None,
        }
    };

    let Some(session_user) = authed else {
        return HttpResponse::Unauthorized()
            .content_type("text/html; charset=utf-8")
            .body(panel_login_html(
                &payload,
                Some(login_error_message(locale)),
                next.as_deref(),
            ));
    };

    let secret = session_secret(Some(&state.token));
    let secure = request_secure(&http);
    let needs_mfa = totp_enabled_for(&session_user) || has_passkeys(&session_user);

    if needs_mfa {
        let pending = create_mfa_pending_token(&session_user, &secret);
        let mut builder = HttpResponse::SeeOther();
        builder.append_header(("Location", mfa_location(next.as_deref())));
        builder.append_header(("Set-Cookie", mfa_pending_cookie_header(&pending, secure)));
        builder.append_header(("Set-Cookie", clear_session_cookie_header(secure)));
        if let Some(ref path) = next
            && let Some(cookie) = login_return_cookie_header(path, secure)
        {
            builder.append_header(("Set-Cookie", cookie));
        }
        return builder.finish();
    }

    let token = create_session_token(&session_user, &secret);
    login_success_response(&http, &token, &session_user, next.as_deref())
}

#[derive(Debug, serde::Deserialize)]
struct MfaForm {
    #[serde(default)]
    code: String,
    #[serde(default)]
    next: String,
}

fn mfa_options_for(username: &str) -> MfaPageOptions {
    MfaPageOptions {
        totp_available: totp_enabled_for(username),
        passkey_available: has_passkeys(username),
    }
}

fn mfa_factors_available(options: &MfaPageOptions) -> bool {
    options.totp_available || options.passkey_available
}

/// When MFA was cleared after password auth (empty passkey store + no TOTP),
/// finish sign-in instead of trapping the user on a passkey-only challenge.
fn finish_pending_without_mfa_factors(
    http: &HttpRequest,
    secret: &str,
    username: &str,
    next: Option<&str>,
) -> HttpResponse {
    let token = create_session_token(username, secret);
    login_success_response(http, &token, username, next)
}

fn redirect_authed_after_login(username: &str, next: Option<&str>) -> HttpResponse {
    let location = crate::account_security::post_login_security_path(username)
        .map(str::to_string)
        .unwrap_or_else(|| post_login_location(next));
    HttpResponse::SeeOther()
        .append_header(("Location", location))
        .finish()
}

#[get("/login/2fa")]
pub async fn login_mfa_page(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let status = state
        .status
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    let payload = enrich_status(status, &state.token);
    let cookie = http
        .headers()
        .get(actix_web::http::header::COOKIE)
        .and_then(|value| value.to_str().ok());
    let secret = session_secret(Some(&state.token));
    let pending_user =
        read_mfa_pending_cookie(cookie).and_then(|token| verify_mfa_pending_token(&token, &secret));
    let next = first_safe_next(&[query.get("next").map(String::as_str)]);
    let Some(username) = pending_user else {
        // Opaque-redirect login JS always lands here. If password login already
        // issued a session (no MFA enrolled), continue to enroll/dashboard.
        if let Some(session_user) = panel_user_from_request(&state, &http) {
            return redirect_authed_after_login(&session_user, next.as_deref());
        }
        return HttpResponse::SeeOther()
            .append_header(("Location", login_location(next.as_deref())))
            .finish();
    };
    let options = mfa_options_for(&username);
    if !mfa_factors_available(&options) {
        return finish_pending_without_mfa_factors(&http, &secret, &username, next.as_deref());
    }
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(panel_mfa_html(&payload, None, next.as_deref(), options))
}

#[post("/login/2fa")]
pub async fn login_mfa_submit(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<MfaForm>,
) -> HttpResponse {
    let status = state
        .status
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    let payload = enrich_status(status, &state.token);
    let cookie = http
        .headers()
        .get(actix_web::http::header::COOKIE)
        .and_then(|value| value.to_str().ok());
    let secret = session_secret(Some(&state.token));
    let next = resolve_next_from_request(&http, Some(form.next.as_str()), None);
    let Some(username) =
        read_mfa_pending_cookie(cookie).and_then(|token| verify_mfa_pending_token(&token, &secret))
    else {
        return HttpResponse::SeeOther()
            .append_header(("Location", login_location(next.as_deref())))
            .finish();
    };

    let options = mfa_options_for(&username);
    if !mfa_factors_available(&options) {
        return finish_pending_without_mfa_factors(&http, &secret, &username, next.as_deref());
    }
    if !options.totp_available {
        return HttpResponse::Unauthorized()
            .content_type("text/html; charset=utf-8")
            .body(panel_mfa_html(
                &payload,
                Some("Use a passkey to finish sign-in for this account."),
                next.as_deref(),
                options,
            ));
    }

    match crate::account_mfa::verify_mfa_challenge(&username, &form.code) {
        Ok(true) => {
            let token = create_session_token(&username, &secret);
            login_success_response(&http, &token, &username, next.as_deref())
        }
        Ok(false) => HttpResponse::Unauthorized()
            .content_type("text/html; charset=utf-8")
            .body(panel_mfa_html(
                &payload,
                Some("Invalid authenticator or backup code."),
                next.as_deref(),
                options,
            )),
        Err(error) => {
            let rate_limited = crate::account_mfa::is_mfa_rate_limit_error(&error);
            let mut builder = if rate_limited {
                HttpResponse::TooManyRequests()
            } else {
                HttpResponse::Unauthorized()
            };
            builder
                .content_type("text/html; charset=utf-8")
                .body(panel_mfa_html(
                    &payload,
                    Some(&error),
                    next.as_deref(),
                    options,
                ))
        }
    }
}

fn panel_html_response(
    http: &HttpRequest,
    state: &AppState,
    preview: bool,
    render: impl FnOnce(&str) -> String,
) -> HttpResponse {
    if let Some(user) = panel_user_from_request(state, http) {
        return HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(render(&user));
    }
    if preview {
        return HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(render("preview"));
    }
    crate::login_next::login_redirect(http)
}

#[actix_web::route("/dashboard", method = "GET", method = "HEAD")]
pub async fn dashboard_page(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let preview = query.get("preview").map(String::as_str) == Some("1");
    panel_html_response(&http, &state, preview, panel_dashboard_html)
}

#[get("/panel")]
pub async fn panel_alias() -> HttpResponse {
    HttpResponse::SeeOther()
        .append_header(("Location", "/dashboard"))
        .finish()
}

fn logout_response(http: &HttpRequest) -> HttpResponse {
    let secure = request_secure(http);
    let query_map = web::Query::<std::collections::HashMap<String, String>>::from_query(
        http.uri().query().unwrap_or(""),
    )
    .ok();
    let query_next = query_map
        .as_ref()
        .and_then(|m| m.get("next").map(String::as_str));
    let next = first_safe_next(&[
        query_next,
        referer_return_path(http).as_deref(),
        request_return_path(http).as_deref(),
    ]);
    let mut builder = HttpResponse::SeeOther();
    builder.append_header(("Location", login_location(next.as_deref())));
    builder.append_header(("Set-Cookie", clear_session_cookie_header(secure)));
    builder.append_header(("Set-Cookie", clear_mfa_pending_cookie_header(secure)));
    for cookie in crate::panel_phpmyadmin_proxy::clear_phpmyadmin_cookie_headers(secure) {
        builder.append_header(("Set-Cookie", cookie));
    }
    if let Some(ref path) = next
        && let Some(cookie) = login_return_cookie_header(path, secure)
    {
        builder.append_header(("Set-Cookie", cookie));
    } else if next.is_none() {
        builder.append_header(("Set-Cookie", clear_login_return_cookie_header(secure)));
    }
    builder.finish()
}

#[get("/logout")]
pub async fn logout_get(http: HttpRequest) -> HttpResponse {
    logout_response(&http)
}

#[post("/logout")]
pub async fn logout_post(http: HttpRequest) -> HttpResponse {
    logout_response(&http)
}

#[get("/api/logout")]
pub async fn api_logout_get(http: HttpRequest) -> HttpResponse {
    logout_response(&http)
}

#[post("/api/logout")]
pub async fn api_logout_post(http: HttpRequest) -> HttpResponse {
    logout_response(&http)
}

#[post("/api/account/setup")]
pub async fn account_setup(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<TokenQuery>,
    request: web::Json<AccountSetupRequest>,
) -> HttpResponse {
    if !authorized_request(&state, &query, &http) {
        return HttpResponse::Unauthorized().finish();
    }
    let mut current = state.status.write().unwrap_or_else(|e| e.into_inner());
    if ["configuring", "downloading", "installing", "testing"].contains(&current.phase) {
        return HttpResponse::Conflict()
            .json(serde_json::json!({"error": "Hay una instalación en curso"}));
    }
    let language = request
        .language
        .as_deref()
        .map(normalize_language)
        .transpose();
    let language = match language {
        Ok(Some(value)) => value,
        Ok(None) => current.language.clone(),
        Err(error) => {
            return HttpResponse::BadRequest().json(serde_json::json!({"error": error}));
        }
    };
    // CPN owns the security baseline; clients cannot weaken it during setup.
    let policy = crate::account::default_password_policy();

    let smtp_settings = if let Some(smtp_input) = request.smtp.as_ref() {
        match validate_smtp_input(smtp_input) {
            Ok(settings) => {
                if let Err(error) = persist_smtp(&settings) {
                    return HttpResponse::BadRequest().json(serde_json::json!({"error": error}));
                }
                Some(settings)
            }
            Err(error) => {
                return HttpResponse::BadRequest().json(serde_json::json!({"error": error}));
            }
        }
    } else {
        // No external SMTP: install/enable Postfix on Linux and persist localhost settings.
        match ensure_postfix_default(&request.recovery_email) {
            Ok(settings) => {
                if let Err(error) = persist_smtp(&settings) {
                    return HttpResponse::BadRequest().json(serde_json::json!({"error": error}));
                }
                Some(settings)
            }
            Err(_windows_or_guest) => None,
        }
    };

    let result = crate::account::setup_account(
        request.username.as_deref().unwrap_or(""),
        request.password.as_deref(),
        request.generate_password,
        &request.recovery_email,
        policy.clone(),
        &language,
    );
    match result {
        Ok(setup) => {
            current.account = Some(setup.public.clone());
            current.password_policy = policy;
            current.language = language;
            current.phase = "completed";
            current.message = "Cuenta inicial guardada".into();
            let login_url = panel_login_url_for(&current, &state.token);
            current.panel_login_url = Some(login_url.clone());
            current.smtp = Some(smtp_status_public());
            crate::paths::clear_installer_bootstrap_token();

            let mut setup_email_sent = false;
            let mut setup_email_error: Option<String> = None;
            if request.send_username_email {
                if let Some(settings) = smtp_settings
                    .clone()
                    .or_else(crate::smtp_settings::load_smtp)
                    .or_else(|| {
                        crate::mail_outbound::resolve_outbound_settings(Some(
                            setup.public.recovery_email.as_str(),
                        ))
                        .ok()
                    })
                {
                    let password_for_mail = if request.include_password_in_email {
                        setup
                            .generated_password
                            .as_deref()
                            .or(request.password.as_deref())
                    } else {
                        None
                    };
                    let mut message = build_setup_confirmation(
                        &setup.public.username,
                        &login_url,
                        request.include_password_in_email,
                        password_for_mail,
                    );
                    message.to = setup.public.recovery_email.clone();
                    match send_mail_with_settings(&settings, &message) {
                        Ok(()) => setup_email_sent = true,
                        Err(error) => setup_email_error = Some(error),
                    }
                } else {
                    setup_email_error = Some(
                        "No outbound mail path (SMTP or local Postfix); setup completed without sending email"
                            .into(),
                    );
                }
            }

            HttpResponse::Ok().json(serde_json::json!({
                "account": setup.public,
                "generated_password": setup.generated_password,
                "panel_login_url": login_url,
                "setup_email_sent": setup_email_sent,
                "setup_email_error": setup_email_error,
                "smtp": smtp_status_public(),
            }))
        }
        Err(error) => HttpResponse::BadRequest().json(serde_json::json!({"error": error})),
    }
}
