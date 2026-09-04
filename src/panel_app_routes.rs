//! Authenticated Apps panel routes.

use crate::apps::{AppId, install_app_on, reinstall_app_on, uninstall_app_on};
use crate::apps_control::{start_app, stop_app};
use crate::auth_api::panel_user_from_request;
use crate::installer::AppState;
use crate::panel_apps::{AppsPageQuery, apps_main};
use crate::panel_pages::panel_shell;
use crate::site_acl::{SitePerm, require_manage_site, sites_manageable_by};
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

fn apps_redirect(domain: &str, notice: Option<&str>, error: Option<&str>) -> String {
    let mut url = "/apps".to_string();
    let mut sep = '?';
    if !domain.trim().is_empty() {
        url.push(sep);
        sep = '&';
        url.push_str(&format!("domain={}", urlencoding_simple(domain.trim())));
    }
    if let Some(notice) = notice {
        url.push(sep);
        url.push_str(&format!("notice={}", urlencoding_simple(notice)));
    } else if let Some(error) = error {
        url.push(sep);
        url.push_str(&format!("error={}", urlencoding_simple(error)));
    }
    url
}

fn optional_domain_for_user(
    user: &str,
    domain: &str,
    perm: SitePerm,
) -> Result<Option<String>, String> {
    let domain = domain.trim();
    if domain.is_empty() {
        return Ok(None);
    }
    let site = require_manage_site(user, domain, perm)?;
    Ok(Some(site.domain))
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
    let domain = query.get("domain").map(String::as_str).unwrap_or("");
    let sites = sites_manageable_by(&user).unwrap_or_default();
    let domain = if domain.trim().is_empty() {
        ""
    } else if sites
        .iter()
        .any(|s| s.domain.eq_ignore_ascii_case(domain.trim()))
    {
        domain.trim()
    } else {
        ""
    };
    html_ok(panel_shell(
        &user,
        "apps",
        "Apps",
        &apps_main(AppsPageQuery {
            notice,
            error,
            domain,
            sites: &sites,
        }),
    ))
}

#[derive(Debug, serde::Deserialize)]
pub struct AppNameForm {
    #[serde(default)]
    name: String,
    #[serde(default)]
    domain: String,
}

#[post("/apps/install")]
pub async fn apps_install(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<AppNameForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    let domain = match optional_domain_for_user(&user, &form.domain, SitePerm::Install) {
        Ok(v) => v,
        Err(error) => {
            return HttpResponse::SeeOther()
                .append_header(("Location", apps_redirect(&form.domain, None, Some(&error))))
                .finish();
        }
    };
    match AppId::parse(&form.name).and_then(|id| install_app_on(id, domain.as_deref())) {
        Ok(message) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                apps_redirect(domain.as_deref().unwrap_or(""), Some(&message), None),
            ))
            .finish(),
        Err(error) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                apps_redirect(domain.as_deref().unwrap_or(""), None, Some(&error)),
            ))
            .finish(),
    }
}

#[post("/apps/reinstall")]
pub async fn apps_reinstall(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<AppNameForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    let domain = match optional_domain_for_user(&user, &form.domain, SitePerm::Install) {
        Ok(v) => v,
        Err(error) => {
            return HttpResponse::SeeOther()
                .append_header(("Location", apps_redirect(&form.domain, None, Some(&error))))
                .finish();
        }
    };
    match AppId::parse(&form.name).and_then(|id| reinstall_app_on(id, domain.as_deref())) {
        Ok(message) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                apps_redirect(domain.as_deref().unwrap_or(""), Some(&message), None),
            ))
            .finish(),
        Err(error) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                apps_redirect(domain.as_deref().unwrap_or(""), None, Some(&error)),
            ))
            .finish(),
    }
}

#[post("/apps/uninstall")]
pub async fn apps_uninstall(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<AppNameForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    let domain = match optional_domain_for_user(&user, &form.domain, SitePerm::Uninstall) {
        Ok(v) => v,
        Err(error) => {
            return HttpResponse::SeeOther()
                .append_header(("Location", apps_redirect(&form.domain, None, Some(&error))))
                .finish();
        }
    };
    match AppId::parse(&form.name).and_then(|id| uninstall_app_on(id, domain.as_deref())) {
        Ok(message) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                apps_redirect(domain.as_deref().unwrap_or(""), Some(&message), None),
            ))
            .finish(),
        Err(error) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                apps_redirect(domain.as_deref().unwrap_or(""), None, Some(&error)),
            ))
            .finish(),
    }
}

#[post("/apps/start")]
pub async fn apps_start(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<AppNameForm>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    match AppId::parse(&form.name).and_then(start_app) {
        Ok(message) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                apps_redirect(form.domain.trim(), Some(&message), None),
            ))
            .finish(),
        Err(error) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                apps_redirect(form.domain.trim(), None, Some(&error)),
            ))
            .finish(),
    }
}

#[post("/apps/stop")]
pub async fn apps_stop(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<AppNameForm>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    match AppId::parse(&form.name).and_then(stop_app) {
        Ok(message) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                apps_redirect(form.domain.trim(), Some(&message), None),
            ))
            .finish(),
        Err(error) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                apps_redirect(form.domain.trim(), None, Some(&error)),
            ))
            .finish(),
    }
}
