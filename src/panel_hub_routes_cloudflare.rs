//! Routes: Cloudflare DNS (`/dns/cloudflare`).

use crate::installer::AppState;
use crate::panel_admin::is_panel_admin;
use crate::panel_hub_http::{html_ok, login_redirect, redirect_notice, require_panel_user};
use crate::panel_hub_pages_cloudflare::cloudflare_dns_page;
use crate::panel_ops_cloudflare::save_cloudflare_settings;
use crate::panel_ops_cloudflare_api::{
    create_dns_record, delete_dns_record, list_dns_records, set_proxy,
    sync_local_zone_to_cloudflare, update_dns_record,
};
use crate::panel_ops_cloudflare_verify::verify_cloudflare_connection;
use crate::panel_pages::panel_shell;
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use serde::Deserialize;
use std::sync::Arc;

fn admin_gate(user: &str, back: &str) -> Option<HttpResponse> {
    if is_panel_admin(user) {
        None
    } else {
        Some(redirect_notice(back, None, Some("Admin only")))
    }
}

#[derive(Debug, Deserialize)]
pub struct CfQuery {
    pub tab: Option<String>,
    pub domain: Option<String>,
    #[serde(rename = "type")]
    pub filter_type: Option<String>,
    pub notice: Option<String>,
    pub error: Option<String>,
}

fn manage_back(domain: &str, filter_type: Option<&str>) -> String {
    let mut back = format!(
        "/dns/cloudflare?tab=manage&domain={}",
        urlencoding_path(domain)
    );
    if let Some(ft) = filter_type
        .map(str::trim)
        .filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("all"))
    {
        back.push_str("&type=");
        back.push_str(&urlencoding_path(ft));
    }
    back
}

#[get("/dns/cloudflare")]
pub async fn cloudflare_dns_get(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<CfQuery>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    let tab = query.tab.as_deref().unwrap_or("manage");
    let domain = query.domain.clone().unwrap_or_default();
    let filter_type = query.filter_type.clone().unwrap_or_default();
    let records = if tab != "api" && !domain.trim().is_empty() {
        list_dns_records(&domain)
    } else {
        Ok(vec![])
    };
    html_ok(panel_shell(
        &user,
        "server",
        "Cloudflare DNS",
        &cloudflare_dns_page(
            tab,
            domain.trim(),
            records,
            filter_type.trim(),
            query.notice.as_deref(),
            query.error.as_deref(),
        ),
    ))
}

/// Alias under Server DNS hub.
#[get("/server/dns/cloudflare")]
pub async fn server_cloudflare_redirect() -> HttpResponse {
    HttpResponse::Found()
        .append_header(("Location", "/dns/cloudflare"))
        .finish()
}

#[derive(Debug, Deserialize)]
pub struct CfSettingsForm {
    pub auth_type: String,
    pub email: String,
    pub api_token: String,
    pub sync_local: String,
}

#[post("/dns/cloudflare/settings")]
pub async fn cloudflare_settings_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<CfSettingsForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    if let Some(resp) = admin_gate(&user, "/dns/cloudflare?tab=api") {
        return resp;
    }
    let sync = form.sync_local.trim() == "1" || form.sync_local.eq_ignore_ascii_case("enable");
    match save_cloudflare_settings(&form.auth_type, &form.email, &form.api_token, sync) {
        Ok(msg) => redirect_notice("/dns/cloudflare?tab=api", Some(&msg), None),
        Err(err) => redirect_notice("/dns/cloudflare?tab=api", None, Some(&err)),
    }
}

#[post("/dns/cloudflare/test")]
pub async fn cloudflare_test_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    if let Some(resp) = admin_gate(&user, "/dns/cloudflare?tab=api") {
        return resp;
    }
    match verify_cloudflare_connection() {
        Ok(result) if result.ok => {
            redirect_notice("/dns/cloudflare?tab=api", Some(&result.message), None)
        }
        Ok(result) => redirect_notice("/dns/cloudflare?tab=api", None, Some(&result.message)),
        Err(err) => redirect_notice("/dns/cloudflare?tab=api", None, Some(&err)),
    }
}

#[derive(Debug, Deserialize)]
pub struct CfDomainForm {
    pub domain: String,
    #[serde(default)]
    pub filter_type: Option<String>,
}

#[post("/dns/cloudflare/sync")]
pub async fn cloudflare_sync_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<CfDomainForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    let back = manage_back(&form.domain, form.filter_type.as_deref());
    if let Some(resp) = admin_gate(&user, &back) {
        return resp;
    }
    match sync_local_zone_to_cloudflare(&form.domain) {
        Ok(msg) => redirect_notice(&back, Some(&msg), None),
        Err(err) => redirect_notice(&back, None, Some(&err)),
    }
}

#[derive(Debug, Deserialize)]
pub struct CfAddForm {
    pub domain: String,
    pub record_type: String,
    pub name: String,
    pub content: String,
    pub ttl: Option<u32>,
    pub priority: Option<u16>,
    pub proxied: Option<String>,
    #[serde(default)]
    pub filter_type: Option<String>,
}

#[post("/dns/cloudflare/add")]
pub async fn cloudflare_add_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<CfAddForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    let back = manage_back(&form.domain, form.filter_type.as_deref());
    if let Some(resp) = admin_gate(&user, &back) {
        return resp;
    }
    let proxied = form
        .proxied
        .as_deref()
        .map(|v| v == "1" || v.eq_ignore_ascii_case("on"))
        .unwrap_or(false);
    match create_dns_record(
        &form.domain,
        &form.record_type,
        &form.name,
        &form.content,
        form.ttl.unwrap_or(3600),
        form.priority,
        proxied,
    ) {
        Ok(msg) => redirect_notice(&back, Some(&msg), None),
        Err(err) => redirect_notice(&back, None, Some(&err)),
    }
}

#[derive(Debug, Deserialize)]
pub struct CfRecordForm {
    pub domain: String,
    pub record_id: String,
    #[serde(default)]
    pub filter_type: Option<String>,
}

#[post("/dns/cloudflare/delete")]
pub async fn cloudflare_delete_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<CfRecordForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    let back = manage_back(&form.domain, form.filter_type.as_deref());
    if let Some(resp) = admin_gate(&user, &back) {
        return resp;
    }
    match delete_dns_record(&form.domain, &form.record_id) {
        Ok(msg) => redirect_notice(&back, Some(&msg), None),
        Err(err) => redirect_notice(&back, None, Some(&err)),
    }
}

#[derive(Debug, Deserialize)]
pub struct CfUpdateForm {
    pub domain: String,
    pub record_id: String,
    pub name: String,
    pub content: String,
    pub ttl: Option<u32>,
    pub priority: Option<u16>,
    pub proxied: Option<String>,
    #[serde(default)]
    pub filter_type: Option<String>,
}

#[post("/dns/cloudflare/update")]
pub async fn cloudflare_update_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<CfUpdateForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    let back = manage_back(&form.domain, form.filter_type.as_deref());
    if let Some(resp) = admin_gate(&user, &back) {
        return resp;
    }
    let proxied = form
        .proxied
        .as_deref()
        .map(|v| v == "1" || v.eq_ignore_ascii_case("on"))
        .unwrap_or(false);
    match update_dns_record(
        &form.domain,
        &form.record_id,
        &form.name,
        &form.content,
        form.ttl.unwrap_or(1),
        form.priority,
        proxied,
    ) {
        Ok(msg) => redirect_notice(&back, Some(&msg), None),
        Err(err) => redirect_notice(&back, None, Some(&err)),
    }
}

#[derive(Debug, Deserialize)]
pub struct CfProxyForm {
    pub domain: String,
    pub record_id: String,
    pub proxied: String,
    #[serde(default)]
    pub filter_type: Option<String>,
}

#[post("/dns/cloudflare/proxy")]
pub async fn cloudflare_proxy_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<CfProxyForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    let back = manage_back(&form.domain, form.filter_type.as_deref());
    if let Some(resp) = admin_gate(&user, &back) {
        return resp;
    }
    let proxied = form.proxied.trim() == "1";
    match set_proxy(&form.domain, &form.record_id, proxied) {
        Ok(msg) => redirect_notice(&back, Some(&msg), None),
        Err(err) => redirect_notice(&back, None, Some(&err)),
    }
}

fn urlencoding_path(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            _ => format!("%{:02X}", c as u8),
        })
        .collect()
}
