//! Passkey (WebAuthn) HTTP routes: register, list/delete, and login ceremonies.

use crate::account_mgmt::find_account;
use crate::account_passkeys::{delete_passkey, list_passkey_summaries};
use crate::installer::AppState;
use crate::panel_hub_http::{login_redirect, redirect_notice, require_panel_user};
use crate::panel_session::{
    clear_mfa_pending_cookie_header, create_session_token, request_https_from_headers,
    session_cookie_header, session_secret,
};
use crate::panel_webauthn::{
    finish_authentication, finish_registration, start_authentication, start_registration,
    webauthn_for_request,
};
use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;
use std::sync::Arc;
use webauthn_rs::prelude::{PublicKeyCredential, RegisterPublicKeyCredential};

fn host_header(http: &HttpRequest) -> Option<&str> {
    http.headers()
        .get(actix_web::http::header::HOST)
        .and_then(|v| v.to_str().ok())
}

fn json_err(status: actix_web::http::StatusCode, message: &str) -> HttpResponse {
    HttpResponse::build(status).json(serde_json::json!({ "error": message }))
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
        Ok(()) => json_ok(serde_json::json!({"ok": true})),
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
        return login_redirect();
    };
    match delete_passkey(&user, form.id.trim()) {
        Ok(()) => redirect_notice("/account/users/modify", Some("Passkey removed"), None),
        Err(error) => redirect_notice("/account/users/modify", None, Some(&error)),
    }
}

#[post("/login/passkey/start")]
pub async fn passkey_login_start(
    http: HttpRequest,
    body: web::Json<PasskeyLoginStartBody>,
) -> HttpResponse {
    let username = body.username.trim();
    if username.is_empty() {
        return json_err(
            actix_web::http::StatusCode::BAD_REQUEST,
            "Username is required",
        );
    }
    // Same error for missing accounts and accounts with no passkeys (no user enumeration).
    if find_account(username).is_err() || !crate::account_passkeys::has_passkeys(username) {
        return json_err(
            actix_web::http::StatusCode::BAD_REQUEST,
            "No passkeys registered for this account",
        );
    }
    let https = request_https_from_headers(&http);
    let webauthn = match webauthn_for_request(host_header(&http), https) {
        Ok((w, _)) => w,
        Err(error) => return json_err(actix_web::http::StatusCode::BAD_REQUEST, &error),
    };
    match start_authentication(&webauthn, username) {
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
    let https = request_https_from_headers(&http);
    let webauthn = match webauthn_for_request(host_header(&http), https) {
        Ok((w, _)) => w,
        Err(error) => return json_err(actix_web::http::StatusCode::BAD_REQUEST, &error),
    };
    match finish_authentication(&webauthn, body.ceremony_id.trim(), &body.credential) {
        Ok(username) => {
            // Prefer canonical casing from disk.
            let session_user = find_account(&username)
                .map(|(boot, _)| boot.username)
                .unwrap_or(username);
            let secret = session_secret(Some(&state.token));
            let token = create_session_token(&session_user, &secret);
            let secure = https;
            HttpResponse::Ok()
                .append_header(("Set-Cookie", session_cookie_header(&token, secure)))
                .append_header(("Set-Cookie", clear_mfa_pending_cookie_header(secure)))
                .json(serde_json::json!({
                    "ok": true,
                    "redirect": "/dashboard",
                    "username": session_user,
                }))
        }
        Err(error) => json_err(actix_web::http::StatusCode::UNAUTHORIZED, &error),
    }
}

pub fn passkey_list_for_profile(username: &str) -> Vec<(String, String, u64, u64)> {
    list_passkey_summaries(username)
}
