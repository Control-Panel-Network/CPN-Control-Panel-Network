//! Login, forgot-password, and first-account setup HTTP handlers.

use crate::account::{load_bootstrap, setup_account};
use crate::auth_pages::{
    forgot_password_ack_html, forgot_password_html, installer_token_required_html,
    login_post_ack_html, panel_login_html,
};
use crate::http_helpers::{
    authorized, enrich_status, install_finished, normalize_language, panel_login_url_for,
    smtp_status_public, token_matches,
};
use crate::installer::AppState;
use crate::mail_outbound::{
    build_password_reset_notice, build_setup_confirmation, send_mail_with_settings,
};
use crate::model::{AccountSetupRequest, OptionalTokenQuery, TokenQuery};
use crate::smtp_settings::{identifier_matches_account, persist_smtp, validate_smtp_input};
use actix_web::{HttpResponse, get, post, web};
use std::sync::Arc;

#[get("/login")]
pub async fn login_page(
    state: web::Data<Arc<AppState>>,
    query: web::Query<OptionalTokenQuery>,
) -> HttpResponse {
    let status = state.status.read().await.clone();
    let finished = install_finished(&status);
    let valid_token = token_matches(&state, query.token.as_deref());
    if !(finished || valid_token) {
        return HttpResponse::Unauthorized()
            .content_type("text/html; charset=utf-8")
            .body(installer_token_required_html());
    }
    let payload = enrich_status(status, &state.token);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(panel_login_html(&payload))
}

#[post("/login")]
pub async fn login_submit(
    state: web::Data<Arc<AppState>>,
    query: web::Query<OptionalTokenQuery>,
) -> HttpResponse {
    let status = state.status.read().await.clone();
    let finished = install_finished(&status);
    let valid_token = token_matches(&state, query.token.as_deref());
    if !(finished || valid_token) {
        return HttpResponse::Unauthorized()
            .content_type("text/html; charset=utf-8")
            .body(installer_token_required_html());
    }
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(login_post_ack_html(query.token.as_deref()))
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

    if let Some(boot) = load_bootstrap()
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
    state: web::Data<Arc<AppState>>,
    query: web::Query<TokenQuery>,
    request: web::Json<AccountSetupRequest>,
) -> HttpResponse {
    if !authorized(&state, &query) {
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

    let result = setup_account(
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
