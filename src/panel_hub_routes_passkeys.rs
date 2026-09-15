//! Passkey (WebAuthn) HTTP routes: register, list/delete, and login ceremonies.

use crate::account_mgmt::find_account;
use crate::account_passkeys::{delete_passkey, list_passkey_summaries};
use crate::installer::AppState;
use crate::login_service_gate::{evaluate_login_services, login_services_ready};
use crate::panel_hub_http::{login_redirect, redirect_notice, require_panel_user};
use crate::panel_session::{
    create_session_token, read_mfa_pending_cookie, request_https_from_headers,
    session_secret, verify_mfa_pending_token,
};
use crate::panel_webauthn::{
    finish_authentication, finish_registration, start_authentication, start_authentication_for_user,
    start_registration, webauthn_for_request,
};
use actix_web::{HttpRequest, HttpResponse, post, web};
use serde::Deserialize;
use std::sync::Arc;
use webauthn_rs::prelude::{PublicKeyCredential, RegisterPublicKeyCredential};

fn host_header(http: &HttpRequest) -> Option<&str> {
    http.headers()
        .get(actix_web::http::header::HOST)
        .and_then(|v| v.to_str().ok())
}

fn strip_urls(message: &str) -> String {
    let mut out = String::with_capacity(message.len());
    let mut rest = message;
    while let Some(idx) = rest.find("http://").or_else(|| rest.find("https://")) {
        out.push_str(&rest[..idx]);
        let after = &rest[idx..];
        let end = after
            .find(|c: char| c.is_whitespace() || matches!(c, ')' | ']' | ',' | ';' | '"' | '\''))
            .unwrap_or(after.len());
        rest = &after[end..];
    }
    out.push_str(rest);
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn json_err(status: actix_web::http::StatusCode, message: &str) -> HttpResponse {
    let safe = strip_urls(message);
    HttpResponse::build(status).json(serde_json::json!({ "error": safe }))
}

fn json_ok(value: serde_json::Value) -> HttpResponse {
    HttpResponse::Ok().json(value)
}

#[derive(Debug, Deserialize)]
pub struct PasskeyRegisterFinishBody {
    #[serde(default)]
    ceremony_id: String,
    #[serde(default)]
    label: String,
    #[serde(default)]
    next: Option<String>,
    credential: RegisterPublicKeyCredential,
}

#[derive(Debug, Deserialize)]
pub struct PasskeyDeleteForm {
    #[serde(default)]
    id: String,
}

#[derive(Debug, Deserialize)]
pub struct PasskeyLoginStartBody {
    #[serde(default)]
    username: String,
}

#[derive(Debug, Deserialize)]
pub struct PasskeyLoginFinishBody {
    #[serde(default)]
    ceremony_id: String,
    credential: PublicKeyCredential,
    #[serde(default)]
    next: Option<String>,
}

#[post("/account/users/profile/passkey/register/start")]
pub async fn passkey_register_start(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return HttpResponse::Unauthorized().json(serde_json::json!({"error": "Sign in required"}));
    };
    let https = request_https_from_headers(&http);
    let webauthn = match webauthn_for_request(host_header(&http), https) {
        Ok((w, _)) => w,
        Err(error) => return json_err(actix_web::http::StatusCode::BAD_REQUEST, &error),
    };
    match start_registration(&webauthn, &user) {
        Ok((ceremony_id, ccr)) => {
            let mut value = match serde_json::to_value(&ccr) {
                Ok(v) => v,
                Err(err) => {
                    return json_err(
                        actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                        &format!("Could not encode challenge: {err}"),
                    );
                }
            };
            if let Some(obj) = value.as_object_mut() {
                obj.insert("ceremony_id".into(), serde_json::Value::String(ceremony_id));
            }
            json_ok(value)
        }
        Err(error) => json_err(actix_web::http::StatusCode::BAD_REQUEST, &error),
    }
}

#[post("/account/users/profile/passkey/register/finish")]
pub async fn passkey_register_finish(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<PasskeyRegisterFinishBody>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return HttpResponse::Unauthorized().json(serde_json::json!({"error": "Sign in required"}));
    };
    let https = request_https_from_headers(&http);
    let webauthn = match webauthn_for_request(host_header(&http), https) {
        Ok((w, _)) => w,
        Err(error) => return json_err(actix_web::http::StatusCode::BAD_REQUEST, &error),
    };
    match finish_registration(
        &webauthn,
        &user,
        body.ceremony_id.trim(),
        body.label.trim(),
        &body.credential,
    ) {
        Ok(()) => {
            let redirect = crate::login_next::passkey_register_location(body.next.as_deref());
            json_ok(serde_json::json!({ "ok": true, "redirect": redirect }))
        }
        Err(error) => json_err(actix_web::http::StatusCode::BAD_REQUEST, &error),
    }
}

#[post("/account/users/profile/passkey/delete")]
pub async fn passkey_delete_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<PasskeyDeleteForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    match delete_passkey(&user, form.id.trim()) {
        Ok(()) => redirect_notice("/account/users/modify", Some("Passkey removed"), None),
        Err(error) => redirect_notice("/account/users/modify", None, Some(&error)),
    }
}

fn services_unavailable_json() -> HttpResponse {
    let gate = evaluate_login_services();
    let message = if gate.message.is_empty() {
        "Panel services are still starting. Sign-in is temporarily disabled.".to_string()
    } else {
        gate.message
    };
    json_err(
        actix_web::http::StatusCode::SERVICE_UNAVAILABLE,
        &message,
    )
}

fn json_session_ok(
    http: &HttpRequest,
    state: &AppState,
    session_user: &str,
    next: Option<&str>,
) -> HttpResponse {
    let secret = session_secret(Some(&state.token));
    let token = create_session_token(session_user, &secret);
    let response =
        crate::auth_api::login_success_response(http, &token, session_user, next);
    let location = response
        .headers()
        .get(actix_web::http::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("/dashboard")
        .to_string();
    let mut builder = HttpResponse::Ok();
    for cookie_hdr in response.headers().get_all(actix_web::http::header::SET_COOKIE) {
        builder.append_header((actix_web::http::header::SET_COOKIE, cookie_hdr.clone()));
    }
    builder.json(serde_json::json!({
        "ok": true,
        "redirect": location,
        "username": session_user,
    }))
}

#[post("/login/passkey/start")]
pub async fn passkey_login_start(
    http: HttpRequest,
    body: web::Json<PasskeyLoginStartBody>,
) -> HttpResponse {
    let _ = &body.username; // Accepted for backward compatibility; login is RP-wide.
    if !login_services_ready() {
        return services_unavailable_json();
    }
    let https = request_https_from_headers(&http);
    let webauthn = match webauthn_for_request(host_header(&http), https) {
        Ok((w, _)) => w,
        Err(error) => return json_err(actix_web::http::StatusCode::BAD_REQUEST, &error),
    };
    match start_authentication(&webauthn) {
        Ok((ceremony_id, rcr)) => {
            let mut value = match serde_json::to_value(&rcr) {
                Ok(v) => v,
                Err(err) => {
                    return json_err(
                        actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                        &format!("Could not encode challenge: {err}"),
                    );
                }
            };
            if let Some(obj) = value.as_object_mut() {
                obj.insert("ceremony_id".into(), serde_json::Value::String(ceremony_id));
            }
            json_ok(value)
        }
        Err(error) => json_err(actix_web::http::StatusCode::BAD_REQUEST, &error),
    }
}

#[post("/login/passkey/finish")]
pub async fn passkey_login_finish(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<PasskeyLoginFinishBody>,
) -> HttpResponse {
    if !login_services_ready() {
        return services_unavailable_json();
    }
    let https = request_https_from_headers(&http);
    let webauthn = match webauthn_for_request(host_header(&http), https) {
        Ok((w, _)) => w,
        Err(error) => return json_err(actix_web::http::StatusCode::BAD_REQUEST, &error),
    };
    match finish_authentication(&webauthn, body.ceremony_id.trim(), &body.credential) {
        Ok(username) => {
            let session_user = find_account(&username)
                .map(|(boot, _)| boot.username)
                .unwrap_or(username);
            let cookie = http
                .headers()
                .get(actix_web::http::header::COOKIE)
                .and_then(|value| value.to_str().ok());
            let next = crate::login_next::first_safe_next(&[
                body.next.as_deref(),
                crate::login_next::read_login_return_cookie(cookie).as_deref(),
            ]);
            json_session_ok(&http, &state, &session_user, next.as_deref())
        }
        Err(error) => json_err(actix_web::http::StatusCode::UNAUTHORIZED, &error),
    }
}

fn mfa_pending_username(http: &HttpRequest, state: &AppState) -> Option<String> {
    let cookie = http
        .headers()
        .get(actix_web::http::header::COOKIE)
        .and_then(|value| value.to_str().ok());
    let secret = session_secret(Some(&state.token));
    read_mfa_pending_cookie(cookie).and_then(|token| verify_mfa_pending_token(&token, &secret))
}

#[post("/login/2fa/passkey/start")]
pub async fn passkey_mfa_start(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(username) = mfa_pending_username(&http, &state) else {
        return json_err(
            actix_web::http::StatusCode::UNAUTHORIZED,
            "Sign in again, then complete two-factor authentication.",
        );
    };
    let https = request_https_from_headers(&http);
    let webauthn = match webauthn_for_request(host_header(&http), https) {
        Ok((w, _)) => w,
        Err(error) => return json_err(actix_web::http::StatusCode::BAD_REQUEST, &error),
    };
    match start_authentication_for_user(&webauthn, &username) {
        Ok((ceremony_id, rcr)) => {
            let mut value = match serde_json::to_value(&rcr) {
                Ok(v) => v,
                Err(err) => {
                    return json_err(
                        actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                        &format!("Could not encode challenge: {err}"),
                    );
                }
            };
            if let Some(obj) = value.as_object_mut() {
                obj.insert("ceremony_id".into(), serde_json::Value::String(ceremony_id));
            }
            json_ok(value)
        }
        Err(error) => json_err(actix_web::http::StatusCode::BAD_REQUEST, &error),
    }
}

#[post("/login/2fa/passkey/finish")]
pub async fn passkey_mfa_finish(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<PasskeyLoginFinishBody>,
) -> HttpResponse {
    let Some(pending_user) = mfa_pending_username(&http, &state) else {
        return json_err(
            actix_web::http::StatusCode::UNAUTHORIZED,
            "Sign in again, then complete two-factor authentication.",
        );
    };
    let https = request_https_from_headers(&http);
    let webauthn = match webauthn_for_request(host_header(&http), https) {
        Ok((w, _)) => w,
        Err(error) => return json_err(actix_web::http::StatusCode::BAD_REQUEST, &error),
    };
    match finish_authentication(&webauthn, body.ceremony_id.trim(), &body.credential) {
        Ok(username) => {
            if !username.eq_ignore_ascii_case(&pending_user) {
                return json_err(
                    actix_web::http::StatusCode::UNAUTHORIZED,
                    "That passkey belongs to a different account.",
                );
            }
            let session_user = find_account(&username)
                .map(|(boot, _)| boot.username)
                .unwrap_or(username);
            let cookie = http
                .headers()
                .get(actix_web::http::header::COOKIE)
                .and_then(|value| value.to_str().ok());
            let next = crate::login_next::first_safe_next(&[
                body.next.as_deref(),
                crate::login_next::read_login_return_cookie(cookie).as_deref(),
            ]);
            // login_success_response (via json_session_ok) already clears the MFA pending cookie.
            json_session_ok(&http, &state, &session_user, next.as_deref())
        }
        Err(error) => json_err(actix_web::http::StatusCode::UNAUTHORIZED, &error),
    }
}

pub fn passkey_list_for_profile(username: &str) -> Vec<(String, String, u64, u64)> {
    list_passkey_summaries(username)
}
