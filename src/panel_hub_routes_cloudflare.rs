//! Routes: Cloudflare DNS (`/dns/cloudflare`) and Let's Encrypt SSL (`/security/ssl`).

use crate::installer::AppState;
use crate::panel_admin::is_panel_admin;
use crate::panel_hub_http::{html_ok, login_redirect, redirect_notice, require_panel_user};
use crate::panel_hub_pages_cloudflare::cloudflare_dns_page;
use crate::panel_ops_cloudflare::save_cloudflare_settings;
use crate::panel_ops_cloudflare_api::{
    create_dns_record, delete_dns_record, list_dns_records, set_proxy,
    sync_local_zone_to_cloudflare,
};
use crate::panel_ops_ssl_le::{
    issue_le_for_all_without_custom, issue_lets_encrypt, renew_lets_encrypt_all,
    restore_lets_encrypt, set_custom_ssl, set_domain_provider, set_include_subdomains,
    upload_custom_ssl,
};
use crate::panel_ops_ssl_provider::{SslProvider, save_ssl_defaults};
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
    pub notice: Option<String>,
    pub error: Option<String>,
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

#[derive(Debug, Deserialize)]
pub struct CfDomainForm {
    pub domain: String,
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
    let back = format!(
        "/dns/cloudflare?tab=manage&domain={}",
        urlencoding_path(&form.domain)
    );
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
    let back = format!(
        "/dns/cloudflare?tab=manage&domain={}",
        urlencoding_path(&form.domain)
    );
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
    let back = format!(
        "/dns/cloudflare?tab=manage&domain={}",
        urlencoding_path(&form.domain)
    );
    if let Some(resp) = admin_gate(&user, &back) {
        return resp;
    }
    match delete_dns_record(&form.domain, &form.record_id) {
        Ok(msg) => redirect_notice(&back, Some(&msg), None),
        Err(err) => redirect_notice(&back, None, Some(&err)),
    }
}

#[derive(Debug, Deserialize)]
pub struct CfProxyForm {
    pub domain: String,
    pub record_id: String,
    pub proxied: String,
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
    let back = format!(
        "/dns/cloudflare?tab=manage&domain={}",
        urlencoding_path(&form.domain)
    );
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

// --- SSL provider actions (GET /security/ssl is owned by panel_hub_routes_security) ---

#[derive(Debug, Deserialize)]
pub struct SslDomainForm {
    pub domain: String,
    #[serde(default)]
    pub r#return: Option<String>,
}

fn ssl_back(form_return: Option<&str>) -> String {
    match form_return.map(str::trim).filter(|s| !s.is_empty()) {
        Some(r) if r.starts_with("/websites/manage") || r.starts_with("/security/ssl") => {
            r.to_string()
        }
        _ => "/security/ssl".into(),
    }
}

#[post("/security/ssl/issue")]
pub async fn security_ssl_issue(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<SslDomainForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    let back = ssl_back(form.r#return.as_deref());
    if let Some(resp) = admin_gate(&user, &back) {
        return resp;
    }
    match issue_lets_encrypt(&form.domain) {
        Ok(msg) => redirect_notice(&back, Some(&msg), None),
        Err(err) => redirect_notice(&back, None, Some(&err)),
    }
}

#[post("/security/ssl/issue-all")]
pub async fn security_ssl_issue_all(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    if let Some(resp) = admin_gate(&user, "/security/ssl") {
        return resp;
    }
    match issue_le_for_all_without_custom() {
        Ok(msg) => redirect_notice("/security/ssl", Some(&msg), None),
        Err(err) => redirect_notice("/security/ssl", None, Some(&err)),
    }
}

#[post("/security/ssl/renew")]
pub async fn security_ssl_renew(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    if let Some(resp) = admin_gate(&user, "/security/ssl") {
        return resp;
    }
    match renew_lets_encrypt_all() {
        Ok(msg) => redirect_notice("/security/ssl", Some(&msg), None),
        Err(err) => redirect_notice("/security/ssl", None, Some(&err)),
    }
}

#[post("/security/ssl/restore-le")]
pub async fn security_ssl_restore_le(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<SslDomainForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    let back = ssl_back(form.r#return.as_deref());
    if let Some(resp) = admin_gate(&user, &back) {
        return resp;
    }
    match restore_lets_encrypt(&form.domain) {
        Ok(msg) => redirect_notice(&back, Some(&msg), None),
        Err(err) => redirect_notice(&back, None, Some(&err)),
    }
}

#[post("/security/ssl/mark-custom")]
pub async fn security_ssl_mark_custom(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<SslDomainForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    let back = ssl_back(form.r#return.as_deref());
    if let Some(resp) = admin_gate(&user, &back) {
        return resp;
    }
    match set_custom_ssl(&form.domain) {
        Ok(msg) => redirect_notice(&back, Some(&msg), None),
        Err(err) => redirect_notice(&back, None, Some(&err)),
    }
}

#[derive(Debug, Deserialize)]
pub struct SslProviderForm {
    pub domain: String,
    pub provider: String,
    #[serde(default)]
    pub include_subdomains: Option<String>,
    #[serde(default)]
    pub r#return: Option<String>,
}

#[post("/security/ssl/provider")]
pub async fn security_ssl_provider(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<SslProviderForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    let back = ssl_back(form.r#return.as_deref());
    if let Some(resp) = admin_gate(&user, &back) {
        return resp;
    }
    let provider = match SslProvider::parse(&form.provider) {
        Ok(p) => p,
        Err(err) => return redirect_notice(&back, None, Some(&err)),
    };
    let include = form
        .include_subdomains
        .as_deref()
        .map(|v| v == "1" || v.eq_ignore_ascii_case("on"))
        .unwrap_or(false);
    match set_domain_provider(&form.domain, provider) {
        Ok(msg) => {
            let _ = set_include_subdomains(&form.domain, include);
            redirect_notice(&back, Some(&msg), None)
        }
        Err(err) => redirect_notice(&back, None, Some(&err)),
    }
}

#[derive(Debug, Deserialize)]
pub struct SslDefaultsForm {
    pub provider: String,
}

#[post("/security/ssl/defaults")]
pub async fn security_ssl_defaults(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<SslDefaultsForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    if let Some(resp) = admin_gate(&user, "/security/ssl") {
        return resp;
    }
    match SslProvider::parse(&form.provider).and_then(|p| {
        save_ssl_defaults(p)?;
        Ok(format!(
            "New-site SSL default set to {} (existing domains unchanged)",
            p.label()
        ))
    }) {
        Ok(msg) => redirect_notice("/security/ssl", Some(&msg), None),
        Err(err) => redirect_notice("/security/ssl", None, Some(&err)),
    }
}

#[derive(Debug, Deserialize)]
pub struct SslUploadForm {
    pub domain: String,
    pub cert_pem: String,
    pub key_pem: String,
    #[serde(default)]
    pub r#return: Option<String>,
}

#[post("/security/ssl/upload")]
pub async fn security_ssl_upload(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<SslUploadForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    let back = ssl_back(form.r#return.as_deref());
    if let Some(resp) = admin_gate(&user, &back) {
        return resp;
    }
    match upload_custom_ssl(&form.domain, &form.cert_pem, &form.key_pem) {
        Ok(msg) => redirect_notice(&back, Some(&msg), None),
        Err(err) => redirect_notice(&back, None, Some(&err)),
    }
}
