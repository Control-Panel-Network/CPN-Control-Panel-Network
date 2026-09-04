//! Authenticated Backups panel routes.

use crate::auth_api::panel_user_from_request;
use crate::backups::{BackupRequest, create_selective_backup};
use crate::installer::AppState;
use crate::panel_hub_pages_backups::backups_hub_main;
use crate::panel_pages::panel_shell;
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use std::sync::Arc;

fn require_panel_user(state: &AppState, http: &HttpRequest) -> Option<String> {
    panel_user_from_request(state, http)
}

fn html_ok(body: String) -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(body)
}

fn login_redirect() -> HttpResponse {
    HttpResponse::SeeOther()
        .append_header(("Location", "/login"))
        .finish()
}

fn urlencoding_simple(value: &str) -> String {
    let mut out = String::with_capacity(value.len() * 3);
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

#[get("/backups")]
pub async fn backups_page(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    let _ = query;
    html_ok(panel_shell(
        &user,
        "backups",
        "Backups",
        &backups_hub_main(),
    ))
}

#[derive(Debug, serde::Deserialize)]
pub struct BackupRunForm {
    #[serde(default)]
    scope: String,
    #[serde(default)]
    domain: String,
    #[serde(default)]
    panel_config: String,
    #[serde(default)]
    website_files: String,
    #[serde(default)]
    backups_folder: String,
    #[serde(default)]
    plugins_folder: String,
    #[serde(default)]
    databases: String,
    #[serde(default)]
    ftp: String,
}

#[post("/backups/run")]
pub async fn backups_run(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<BackupRunForm>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    let scope = if form.scope.trim().is_empty() {
        "panel"
    } else {
        form.scope.trim()
    };
    let domain = form.domain.trim();
    match create_selective_backup(&BackupRequest {
        scope: scope.to_string(),
        domain: domain.to_string(),
        panel_config: form.panel_config.clone(),
        website_files: form.website_files.clone(),
        backups_folder: form.backups_folder.clone(),
        plugins_folder: form.plugins_folder.clone(),
        databases: form.databases.clone(),
        ftp: form.ftp.clone(),
    }) {
        Ok(name) => {
            let mut loc = format!(
                "/backups/create?scope={}&notice={}",
                urlencoding_simple(scope),
                urlencoding_simple(&format!("Created {name}"))
            );
            if !domain.is_empty() {
                loc.push_str(&format!("&domain={}", urlencoding_simple(domain)));
            }
            HttpResponse::SeeOther()
                .append_header(("Location", loc))
                .finish()
        }
        Err(error) => {
            let mut loc = format!(
                "/backups/create?scope={}&error={}",
                urlencoding_simple(scope),
                urlencoding_simple(&error)
            );
            if !domain.is_empty() {
                loc.push_str(&format!("&domain={}", urlencoding_simple(domain)));
            }
            HttpResponse::SeeOther()
                .append_header(("Location", loc))
                .finish()
        }
    }
}
