//! Routes: Cloudflare DNS (`/dns/cloudflare`).

use crate::installer::AppState;
use crate::panel_admin::is_panel_admin;
use crate::panel_hub_http::{html_ok, login_redirect, redirect_notice, require_panel_user};
use crate::panel_hub_pages_cloudflare::{cloudflare_dns_page, preferred_manage_domain};
use crate::panel_hub_pages_cloudflare_pager::{
    CfTableOpts, dns_mode_from_query, dns_page_from_query, dns_per_page_from_query, manage_list_url,
};
use crate::panel_ops_cloudflare::save_cloudflare_settings;
use crate::panel_ops_cloudflare_api::{
    create_dns_record, delete_dns_record, list_dns_records, set_proxy,
    sync_local_zone_to_cloudflare, update_dns_record,
};
use crate::panel_ops_cloudflare_oauth::{
    begin_oauth_connect, disconnect_oauth, finish_oauth_callback, save_oauth_client,
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
    pub page: Option<String>,
    pub per_page: Option<String>,
    pub mode: Option<String>,
    pub notice: Option<String>,
    pub error: Option<String>,
}

fn table_opts_from_query(query: &CfQuery) -> CfTableOpts {
    let per_raw = query.per_page.as_deref().unwrap_or("10");
    let mode = if per_raw.eq_ignore_ascii_case("all") {
        "scroll".to_string()
    } else {
        dns_mode_from_query(query.mode.as_deref().unwrap_or("page")).to_string()
    };
    CfTableOpts {
        filter_type: query.filter_type.clone().unwrap_or_default(),
        page: dns_page_from_query(query.page.as_deref().unwrap_or("1")),
        per_page: dns_per_page_from_query(per_raw),
        mode,
    }
}

fn manage_back(domain: &str, opts: &CfTableOpts) -> String {
    manage_list_url(
        domain,
        &opts.filter_type,
        &opts.mode,
        opts.per_page,
        opts.page,
    )
}

fn opts_from_form(
    filter_type: Option<&str>,
    page: Option<&str>,
    per_page: Option<&str>,
    mode: Option<&str>,
) -> CfTableOpts {
    CfTableOpts {
        filter_type: filter_type.unwrap_or("").to_string(),
        page: dns_page_from_query(page.unwrap_or("1")),
        per_page: dns_per_page_from_query(per_page.unwrap_or("10")),
        mode: dns_mode_from_query(mode.unwrap_or("page")).to_string(),
    }
}

#[get("/dns/cloudflare")]
pub async fn cloudflare_dns_get(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<CfQuery>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let tab = query.tab.as_deref().unwrap_or("manage");
    let mut domain = query.domain.clone().unwrap_or_default();
    let table_opts = table_opts_from_query(&query);
    // After OAuth (or token auth), open the first Cloudflare zone even when no local website exists.
    if tab != "api"
        && domain.trim().is_empty()
        && let Some(preferred) = preferred_manage_domain()
    {
        let mut loc = manage_list_url(
            &preferred,
            &table_opts.filter_type,
            &table_opts.mode,
            table_opts.per_page,
            table_opts.page,
        );
        if let Some(n) = query.notice.as_deref().filter(|s| !s.is_empty()) {
            loc.push_str("&notice=");
            loc.push_str(&urlencoding_path(n));
        }
        if let Some(e) = query.error.as_deref().filter(|s| !s.is_empty()) {
            loc.push_str("&error=");
            loc.push_str(&urlencoding_path(e));
        }
        return HttpResponse::Found()
            .append_header(("Location", loc))
            .finish();
    }
    domain = domain.trim().to_string();
    let records = if tab != "api" && !domain.is_empty() {
        list_dns_records(&domain)
    } else {
        Ok(vec![])
    };
    let listen_port = state.bind_port;
    html_ok(panel_shell(
        &user,
        "server",
        "Cloudflare DNS",
        &cloudflare_dns_page(
            tab,
            &domain,
            records,
            &table_opts,
            listen_port,
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
        return login_redirect(&http);
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
        return login_redirect(&http);
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
    #[serde(default)]
    pub page: Option<String>,
    #[serde(default)]
    pub per_page: Option<String>,
    #[serde(default)]
    pub mode: Option<String>,
}

#[post("/dns/cloudflare/sync")]
pub async fn cloudflare_sync_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<CfDomainForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let opts = opts_from_form(
        form.filter_type.as_deref(),
        form.page.as_deref(),
        form.per_page.as_deref(),
        form.mode.as_deref(),
    );
    let back = manage_back(&form.domain, &opts);
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
    #[serde(default)]
    pub priority: Option<String>,
    pub proxied: Option<String>,
    #[serde(default)]
    pub filter_type: Option<String>,
    #[serde(default)]
    pub page: Option<String>,
    #[serde(default)]
    pub per_page: Option<String>,
    #[serde(default)]
    pub mode: Option<String>,
}

fn parse_optional_u16(raw: Option<&str>) -> Option<u16> {
    raw.map(str::trim)
        .filter(|s| !s.is_empty())
        .and_then(|s| s.parse::<u16>().ok())
}

#[post("/dns/cloudflare/add")]
pub async fn cloudflare_add_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<CfAddForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let opts = opts_from_form(
        form.filter_type.as_deref(),
        form.page.as_deref(),
        form.per_page.as_deref(),
        form.mode.as_deref(),
    );
    let back = manage_back(&form.domain, &opts);
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
        parse_optional_u16(form.priority.as_deref()),
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
    #[serde(default)]
    pub page: Option<String>,
    #[serde(default)]
    pub per_page: Option<String>,
    #[serde(default)]
    pub mode: Option<String>,
}

#[post("/dns/cloudflare/delete")]
pub async fn cloudflare_delete_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<CfRecordForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let opts = opts_from_form(
        form.filter_type.as_deref(),
        form.page.as_deref(),
        form.per_page.as_deref(),
        form.mode.as_deref(),
    );
    let back = manage_back(&form.domain, &opts);
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
    #[serde(default)]
    pub priority: Option<String>,
    pub proxied: Option<String>,
    #[serde(default)]
    pub filter_type: Option<String>,
    #[serde(default)]
    pub page: Option<String>,
    #[serde(default)]
    pub per_page: Option<String>,
    #[serde(default)]
    pub mode: Option<String>,
}

#[post("/dns/cloudflare/update")]
pub async fn cloudflare_update_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<CfUpdateForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let opts = opts_from_form(
        form.filter_type.as_deref(),
        form.page.as_deref(),
        form.per_page.as_deref(),
        form.mode.as_deref(),
    );
    let back = manage_back(&form.domain, &opts);
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
        parse_optional_u16(form.priority.as_deref()),
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
    #[serde(default)]
    pub page: Option<String>,
    #[serde(default)]
    pub per_page: Option<String>,
    #[serde(default)]
    pub mode: Option<String>,
}

#[post("/dns/cloudflare/proxy")]
pub async fn cloudflare_proxy_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<CfProxyForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let opts = opts_from_form(
        form.filter_type.as_deref(),
        form.page.as_deref(),
        form.per_page.as_deref(),
        form.mode.as_deref(),
    );
    let back = manage_back(&form.domain, &opts);
    if let Some(resp) = admin_gate(&user, &back) {
        return resp;
    }
    let proxied = form.proxied.trim() == "1";
    match set_proxy(&form.domain, &form.record_id, proxied) {
        Ok(msg) => redirect_notice(&back, Some(&msg), None),
        Err(err) => redirect_notice(&back, None, Some(&err)),
    }
}

#[derive(Debug, Deserialize)]
pub struct CfOauthClientForm {
    pub client_id: String,
    pub client_secret: String,
}

#[post("/dns/cloudflare/oauth/client")]
pub async fn cloudflare_oauth_client_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<CfOauthClientForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Some(resp) = admin_gate(&user, "/dns/cloudflare?tab=api") {
        return resp;
    }
    match save_oauth_client(&form.client_id, &form.client_secret) {
        Ok(msg) => redirect_notice("/dns/cloudflare?tab=api", Some(&msg), None),
        Err(err) => redirect_notice("/dns/cloudflare?tab=api", None, Some(&err)),
    }
}

#[post("/dns/cloudflare/oauth/connect")]
pub async fn cloudflare_oauth_connect_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Some(resp) = admin_gate(&user, "/dns/cloudflare?tab=api") {
        return resp;
    }
    match begin_oauth_connect(state.bind_port) {
        Ok(url) => HttpResponse::SeeOther()
            .append_header(("Location", url))
            .finish(),
        Err(err) => redirect_notice("/dns/cloudflare?tab=api", None, Some(&err)),
    }
}

#[derive(Debug, Deserialize)]
pub struct CfOauthCallbackQuery {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
    pub error_description: Option<String>,
}

#[get("/dns/cloudflare/oauth/callback")]
pub async fn cloudflare_oauth_callback_get(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<CfOauthCallbackQuery>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Some(resp) = admin_gate(&user, "/dns/cloudflare?tab=api") {
        return resp;
    }
    if let Some(err) = query.error.as_deref().filter(|s| !s.is_empty()) {
        let detail = query.error_description.as_deref().unwrap_or("");
        let msg = format!("Cloudflare OAuth denied: {err} {detail}");
        return redirect_notice("/dns/cloudflare?tab=api", None, Some(msg.trim()));
    }
    let code = query.code.as_deref().unwrap_or("");
    let oauth_state = query.state.as_deref().unwrap_or("");
    match finish_oauth_callback(code, oauth_state, state.bind_port) {
        Ok(msg) => redirect_notice("/dns/cloudflare?tab=api", Some(&msg), None),
        Err(err) => redirect_notice("/dns/cloudflare?tab=api", None, Some(&err)),
    }
}

#[post("/dns/cloudflare/oauth/disconnect")]
pub async fn cloudflare_oauth_disconnect_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Some(resp) = admin_gate(&user, "/dns/cloudflare?tab=api") {
        return resp;
    }
    match disconnect_oauth() {
        Ok(msg) => redirect_notice("/dns/cloudflare?tab=api", Some(&msg), None),
        Err(err) => redirect_notice("/dns/cloudflare?tab=api", None, Some(&err)),
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
