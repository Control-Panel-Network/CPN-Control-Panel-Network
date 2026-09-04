//! Authenticated Panel section routes and mutating actions.

use crate::auth_api::panel_user_from_request;
use crate::installer::AppState;
use crate::panel_pages::panel_shell;
use crate::panel_sections::{
    backups_main, create_panel_backup, databases_main, email_main, websites_main,
};
use crate::sites::{create_site, delete_site};
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

#[get("/websites")]
pub async fn websites_page(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    let notice = query.get("notice").map(String::as_str);
    let error = query.get("error").map(String::as_str);
    html_ok(panel_shell(
        &user,
        "websites",
        "Websites",
        &websites_main(notice, error),
    ))
}

#[derive(Debug, serde::Deserialize)]
pub struct SiteCreateForm {
    #[serde(default)]
    domain: String,
    #[serde(default)]
    owner: String,
    #[serde(default)]
    docroot: String,
}

#[post("/websites/create")]
pub async fn websites_create(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<SiteCreateForm>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    let docroot = form.docroot.trim();
    let result = create_site(
        &form.domain,
        &form.owner,
        if docroot.is_empty() {
            None
        } else {
            Some(docroot)
        },
        None,
        None,
    );
    match result {
        Ok(site) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                format!(
                    "/websites?notice={}",
                    urlencoding_simple(&format!("Created {}", site.domain))
                ),
            ))
            .finish(),
        Err(error) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                format!("/websites?error={}", urlencoding_simple(&error)),
            ))
            .finish(),
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct SiteDeleteForm {
    #[serde(default)]
    domain: String,
}

#[post("/websites/delete")]
pub async fn websites_delete(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<SiteDeleteForm>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    match delete_site(&form.domain) {
        Ok(()) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                format!(
                    "/websites?notice={}",
                    urlencoding_simple(&format!("Deleted {}", form.domain.trim()))
                ),
            ))
            .finish(),
        Err(error) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                format!("/websites?error={}", urlencoding_simple(&error)),
            ))
            .finish(),
    }
}

#[get("/email")]
pub async fn email_page(http: HttpRequest, state: web::Data<Arc<AppState>>) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    let status = state.status.read().await.clone();
    html_ok(panel_shell(
        &user,
        "email",
        "Email",
        &email_main(
            status.selected_mail,
            status.mail_client_ready,
            status.mail_backend_ready,
        ),
    ))
}

#[get("/databases")]
pub async fn databases_page(http: HttpRequest, state: web::Data<Arc<AppState>>) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    html_ok(panel_shell(
        &user,
        "databases",
        "Databases",
        &databases_main(),
    ))
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
    let notice = query.get("notice").map(String::as_str);
    let error = query.get("error").map(String::as_str);
    html_ok(panel_shell(
        &user,
        "backups",
        "Backups",
        &backups_main(notice, error),
    ))
}

#[post("/backups/run")]
pub async fn backups_run(http: HttpRequest, state: web::Data<Arc<AppState>>) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    match create_panel_backup() {
        Ok(name) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                format!(
                    "/backups?notice={}",
                    urlencoding_simple(&format!("Created {name}"))
                ),
            ))
            .finish(),
        Err(error) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                format!("/backups?error={}", urlencoding_simple(&error)),
            ))
            .finish(),
    }
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
