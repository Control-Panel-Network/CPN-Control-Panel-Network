//! Authenticated Apps panel routes.

use crate::apps::{AppId, install_app, reinstall_app, uninstall_app};
use crate::auth_api::panel_user_from_request;
use crate::installer::AppState;
use crate::panel_apps::apps_main;
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

fn apps_redirect(notice: Option<&str>, error: Option<&str>) -> String {
    match (notice, error) {
        (Some(notice), _) => format!("/apps?notice={}", urlencoding_simple(notice)),
        (_, Some(error)) => format!("/apps?error={}", urlencoding_simple(error)),
        _ => "/apps".into(),
    }
}

#[get("/apps")]
pub async fn apps_page(
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
        "apps",
        "Apps",
        &apps_main(notice, error),
    ))
}

#[derive(Debug, serde::Deserialize)]
pub struct AppNameForm {
    #[serde(default)]
    name: String,
}

#[post("/apps/install")]
pub async fn apps_install(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<AppNameForm>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    match AppId::parse(&form.name).and_then(install_app) {
        Ok(message) => HttpResponse::SeeOther()
            .append_header(("Location", apps_redirect(Some(&message), None)))
            .finish(),
        Err(error) => HttpResponse::SeeOther()
            .append_header(("Location", apps_redirect(None, Some(&error))))
            .finish(),
    }
}

#[post("/apps/reinstall")]
pub async fn apps_reinstall(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<AppNameForm>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    match AppId::parse(&form.name).and_then(reinstall_app) {
        Ok(message) => HttpResponse::SeeOther()
            .append_header(("Location", apps_redirect(Some(&message), None)))
            .finish(),
        Err(error) => HttpResponse::SeeOther()
            .append_header(("Location", apps_redirect(None, Some(&error))))
            .finish(),
    }
}

#[post("/apps/uninstall")]
pub async fn apps_uninstall(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<AppNameForm>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    match AppId::parse(&form.name).and_then(uninstall_app) {
        Ok(message) => HttpResponse::SeeOther()
            .append_header(("Location", apps_redirect(Some(&message), None)))
            .finish(),
        Err(error) => HttpResponse::SeeOther()
            .append_header(("Location", apps_redirect(None, Some(&error))))
            .finish(),
    }
}
