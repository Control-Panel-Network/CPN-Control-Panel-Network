//! HTTP routes for site Domain Alias and Cron Jobs.

use crate::installer::AppState;
use crate::panel_hub_http::{login_redirect, require_panel_user, urlencoding_simple};
use crate::panel_ops_site_alias::{add_site_alias, remove_site_alias, verify_alias_csrf};
use crate::panel_ops_site_cron::{
    add_site_cron_job, delete_site_cron_job, update_site_cron_job, verify_cron_csrf,
};
use crate::site_acl::{SitePerm, require_manage_site};
use actix_web::{HttpRequest, HttpResponse, post, web};
use std::collections::HashMap;
use std::sync::Arc;

fn same_origin_ok(http: &HttpRequest) -> bool {
    let host = http
        .headers()
        .get("host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if host.is_empty() {
        return true;
    }
    if let Some(origin) = http.headers().get("origin").and_then(|v| v.to_str().ok()) {
        return origin.contains(host);
    }
    if let Some(referer) = http.headers().get("referer").and_then(|v| v.to_str().ok()) {
        return referer.contains(host);
    }
    true
}

fn redirect_manage(
    domain: &str,
    tab: &str,
    notice: Option<&str>,
    error: Option<&str>,
) -> HttpResponse {
    let mut loc = format!(
        "/websites/manage?domain={}&tab={}",
        urlencoding_simple(domain),
        urlencoding_simple(tab)
    );
    if let Some(n) = notice {
        loc.push_str("&notice=");
        loc.push_str(&urlencoding_simple(n));
    }
    if let Some(e) = error {
        loc.push_str("&error=");
        loc.push_str(&urlencoding_simple(e));
    }
    HttpResponse::SeeOther()
        .append_header(("Location", loc))
        .finish()
}

#[post("/websites/alias/add")]
pub async fn websites_alias_add(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !same_origin_ok(&http) {
        return HttpResponse::Forbidden().body("Origin not allowed");
    }
    let domain = form.get("domain").map(String::as_str).unwrap_or("");
    let alias = form.get("alias").map(String::as_str).unwrap_or("");
    let dns_mode = form.get("dns_mode").map(String::as_str).unwrap_or("cname");
    let csrf = form.get("csrf").map(String::as_str).unwrap_or("");
    if !verify_alias_csrf(&user, csrf) {
        return redirect_manage(domain, "alias", None, Some("Invalid or expired CSRF token"));
    }
    if let Err(err) = require_manage_site(&user, domain, SitePerm::Enable) {
        return redirect_manage(domain, "alias", None, Some(&err));
    }
    match add_site_alias(domain, alias, dns_mode) {
        Ok((site, msg)) => redirect_manage(&site.domain, "alias", Some(&msg), None),
        Err(err) => redirect_manage(domain, "alias", None, Some(&err)),
    }
}

#[post("/websites/alias/remove")]
pub async fn websites_alias_remove(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !same_origin_ok(&http) {
        return HttpResponse::Forbidden().body("Origin not allowed");
    }
    let domain = form.get("domain").map(String::as_str).unwrap_or("");
    let alias = form.get("alias").map(String::as_str).unwrap_or("");
    let csrf = form.get("csrf").map(String::as_str).unwrap_or("");
    if !verify_alias_csrf(&user, csrf) {
        return redirect_manage(domain, "alias", None, Some("Invalid or expired CSRF token"));
    }
    if let Err(err) = require_manage_site(&user, domain, SitePerm::Enable) {
        return redirect_manage(domain, "alias", None, Some(&err));
    }
    match remove_site_alias(domain, alias) {
        Ok((site, msg)) => redirect_manage(&site.domain, "alias", Some(&msg), None),
        Err(err) => redirect_manage(domain, "alias", None, Some(&err)),
    }
}

#[post("/websites/cron/add")]
pub async fn websites_cron_add(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !same_origin_ok(&http) {
        return HttpResponse::Forbidden().body("Origin not allowed");
    }
    let domain = form.get("domain").map(String::as_str).unwrap_or("");
    let csrf = form.get("csrf").map(String::as_str).unwrap_or("");
    if !verify_cron_csrf(&user, csrf) {
        return redirect_manage(domain, "cron", None, Some("Invalid or expired CSRF token"));
    }
    if let Err(err) = require_manage_site(&user, domain, SitePerm::Enable) {
        return redirect_manage(domain, "cron", None, Some(&err));
    }
    match add_site_cron_job(
        domain,
        form.get("minute").map(String::as_str).unwrap_or("*"),
        form.get("hour").map(String::as_str).unwrap_or("*"),
        form.get("day").map(String::as_str).unwrap_or("*"),
        form.get("month").map(String::as_str).unwrap_or("*"),
        form.get("weekday").map(String::as_str).unwrap_or("*"),
        form.get("command").map(String::as_str).unwrap_or(""),
        form.get("comment").map(String::as_str).unwrap_or(""),
    ) {
        Ok((site, msg)) => redirect_manage(&site.domain, "cron", Some(&msg), None),
        Err(err) => redirect_manage(domain, "cron", None, Some(&err)),
    }
}

#[post("/websites/cron/update")]
pub async fn websites_cron_update(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !same_origin_ok(&http) {
        return HttpResponse::Forbidden().body("Origin not allowed");
    }
    let domain = form.get("domain").map(String::as_str).unwrap_or("");
    let csrf = form.get("csrf").map(String::as_str).unwrap_or("");
    if !verify_cron_csrf(&user, csrf) {
        return redirect_manage(domain, "cron", None, Some("Invalid or expired CSRF token"));
    }
    if let Err(err) = require_manage_site(&user, domain, SitePerm::Enable) {
        return redirect_manage(domain, "cron", None, Some(&err));
    }
    let enabled = form.get("enabled").map(String::as_str).unwrap_or("") == "1";
    match update_site_cron_job(
        domain,
        form.get("job_id").map(String::as_str).unwrap_or(""),
        form.get("minute").map(String::as_str).unwrap_or("*"),
        form.get("hour").map(String::as_str).unwrap_or("*"),
        form.get("day").map(String::as_str).unwrap_or("*"),
        form.get("month").map(String::as_str).unwrap_or("*"),
        form.get("weekday").map(String::as_str).unwrap_or("*"),
        form.get("command").map(String::as_str).unwrap_or(""),
        form.get("comment").map(String::as_str).unwrap_or(""),
        enabled,
    ) {
        Ok((site, msg)) => redirect_manage(&site.domain, "cron", Some(&msg), None),
        Err(err) => redirect_manage(domain, "cron", None, Some(&err)),
    }
}

#[post("/websites/cron/delete")]
pub async fn websites_cron_delete(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !same_origin_ok(&http) {
        return HttpResponse::Forbidden().body("Origin not allowed");
    }
    let domain = form.get("domain").map(String::as_str).unwrap_or("");
    let csrf = form.get("csrf").map(String::as_str).unwrap_or("");
    if !verify_cron_csrf(&user, csrf) {
        return redirect_manage(domain, "cron", None, Some("Invalid or expired CSRF token"));
    }
    if let Err(err) = require_manage_site(&user, domain, SitePerm::Enable) {
        return redirect_manage(domain, "cron", None, Some(&err));
    }
    match delete_site_cron_job(domain, form.get("job_id").map(String::as_str).unwrap_or("")) {
        Ok((site, msg)) => redirect_manage(&site.domain, "cron", Some(&msg), None),
        Err(err) => redirect_manage(domain, "cron", None, Some(&err)),
    }
}
