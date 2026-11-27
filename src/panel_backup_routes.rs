//! Authenticated Backups panel routes.

use crate::auth_api::panel_user_from_request;
use crate::backups::{BackupRequest, create_selective_backup};
use crate::installer::AppState;
use crate::login_next::login_redirect;
use crate::panel_hub_http::authenticated_panel_shell;
use crate::panel_hub_pages_backups::backups_hub_main;
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
        return login_redirect(&http);
    };
    let _ = query;
    html_ok(authenticated_panel_shell(
        &http,
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
    #[serde(default)]
    email: String,
    #[serde(default)]
    email_stack: String,
}

#[post("/backups/run")]
pub async fn backups_run(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<BackupRunForm>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
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
        email: form.email.clone(),
        email_stack: form.email_stack.clone(),
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

#[derive(Debug, serde::Deserialize)]
pub struct BackupDeleteForm {
    #[serde(default)]
    scope: String,
    #[serde(default)]
    domain: String,
    #[serde(default)]
    dir: String,
    #[serde(default)]
    archive: String,
    #[serde(default)]
    return_to: String,
}

#[post("/backups/delete")]
pub async fn backups_delete(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<BackupDeleteForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let return_to = form.return_to.trim();
    let return_to = if return_to.starts_with("/backups/") {
        return_to
    } else {
        "/backups/restore"
    };
    match crate::backups_delete::delete_backup_archive(&form.dir, &form.archive) {
        Ok(msg) => {
            let _ = crate::panel_audit::record(
                &user,
                "backups.delete",
                &form.archive,
                true,
                &format!("Deleted backup archive from {}", form.dir.trim()),
            );
            let loc = format!(
                "{}{}notice={}",
                return_to,
                if return_to.contains('?') { "&" } else { "?" },
                urlencoding_simple(&msg)
            );
            let _ = (&form.scope, &form.domain);
            HttpResponse::SeeOther()
                .append_header(("Location", loc))
                .finish()
        }
        Err(error) => {
            let _ = crate::panel_audit::record(
                &user,
                "backups.delete",
                &form.archive,
                false,
                "Backup archive delete failed",
            );
            let loc = format!(
                "{}{}error={}",
                return_to,
                if return_to.contains('?') { "&" } else { "?" },
                urlencoding_simple(&error)
            );
            HttpResponse::SeeOther()
                .append_header(("Location", loc))
                .finish()
        }
    }
}
