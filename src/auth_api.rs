//! Login, forgot-password, panel dashboard, logout, and first-account setup HTTP handlers.

use crate::account::{
    hash_password, password_hash_needs_upgrade, verify_password, write_account_file,
};
use crate::account_mgmt::find_account;
use crate::auth_pages::{
    forgot_password_ack_html, forgot_password_html, installer_token_required_html, panel_login_html,
};
use crate::http_helpers::{
    authorized_request, enrich_status, install_finished, normalize_language, panel_account_ready,
    panel_login_url_for, smtp_status_public, token_matches,
};
use crate::installer::AppState;
use crate::mail_outbound::{
    build_password_reset_notice, build_setup_confirmation, send_mail_with_settings,
};
use crate::model::{AccountSetupRequest, OptionalTokenQuery, TokenQuery};
use crate::panel_pages::panel_dashboard_html;
use crate::panel_session::{
    clear_session_cookie_header, create_session_token, read_session_cookie, request_is_https,
    session_cookie_header, session_secret, verify_session_token,
};
use crate::smtp_settings::{identifier_matches_account, persist_smtp, validate_smtp_input};
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use std::sync::Arc;

fn login_error_message(locale: &str) -> &'static str {
    match locale {
        "es" => "Usuario o contraseña no válidos.",
        "nb" => "Ugyldig brukernavn eller passord.",
        _ => "Invalid username or password.",
    }
}

fn request_secure(http: &HttpRequest) -> bool {
    let forwarded = http
        .headers()
        .get("X-Forwarded-Proto")
        .and_then(|value| value.to_str().ok());
    request_is_https(http.connection_info().scheme(), forwarded)
}

pub fn panel_user_from_request(state: &AppState, http: &HttpRequest) -> Option<String> {
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

#[get("/login")]
pub async fn login_page(
    state: web::Data<Arc<AppState>>,
    query: web::Query<OptionalTokenQuery>,
) -> HttpResponse {
    let status = state.status.read().await.clone();
    let allow_login = install_finished(&status)
        || panel_account_ready(&status)
        || token_matches(&state, query.token.as_deref());
    if !allow_login {
        return HttpResponse::Unauthorized()
            .content_type("text/html; charset=utf-8")
            .body(installer_token_required_html());
    }
    let payload = enrich_status(status, &state.token);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(panel_login_html(&payload, None))
}

#[derive(Debug, serde::Deserialize)]
struct LoginForm {
    #[serde(default)]
    username: String,
    #[serde(default)]
    password: String,
    #[serde(default)]
    remember_me: String,
}

#[post("/login")]
pub async fn login_submit(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<OptionalTokenQuery>,
    form: web::Form<LoginForm>,
) -> HttpResponse {
    let status = state.status.read().await.clone();
    let allow_login = install_finished(&status)
        || panel_account_ready(&status)
        || token_matches(&state, query.token.as_deref());
    if !allow_login {
        return HttpResponse::Unauthorized()
            .content_type("text/html; charset=utf-8")
            .body(installer_token_required_html());
    }

    let payload = enrich_status(status, &state.token);
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
            ));
    };

    let secret = session_secret(Some(&state.token));
    let token = create_session_token(&session_user, &secret);
    let secure = request_secure(&http);
    HttpResponse::SeeOther()
        .append_header(("Location", "/dashboard"))
        .append_header(("Set-Cookie", session_cookie_header(&token, secure)))
        .finish()
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
    HttpResponse::SeeOther()
        .append_header(("Location", "/login"))
        .finish()
}

#[get("/dashboard")]
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
    HttpResponse::SeeOther()
        .append_header(("Location", "/login"))
        .append_header(("Set-Cookie", clear_session_cookie_header(secure)))
        .finish()
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

#[get("/forgot-password")]
pub async fn forgot_password_page() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(forgot_password_html())
}

#[post("/forgot-password")]
pub async fn forgot_password_submit(
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

    if let Some(boot) = crate::account::load_bootstrap()
        && identifier_matches_account(&boot.username, &boot.recovery_email, &identifier)
        && let Some(settings) = crate::smtp_settings::load_smtp()
    {
        let status = state.status.read().await.clone();
        let login_url = panel_login_url_for(&status, &state.token);
        let mut message = build_password_reset_notice(&login_url);
        message.to = boot.recovery_email.clone();
        let _ = send_mail_with_settings(&settings, &message);
    }

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(forgot_password_ack_html())
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
    let mut current = state.status.write().await;
    if ["downloading", "installing", "testing"].contains(&current.phase) {
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
    let policy = request
        .password_policy
        .clone()
        .unwrap_or_else(|| current.password_policy.clone());

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
        None
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

            let mut setup_email_sent = false;
            let mut setup_email_error: Option<String> = None;
            if request.send_username_email {
                if let Some(settings) = smtp_settings
                    .clone()
                    .or_else(crate::smtp_settings::load_smtp)
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
                        "SMTP is not configured; setup completed without sending email".into(),
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
