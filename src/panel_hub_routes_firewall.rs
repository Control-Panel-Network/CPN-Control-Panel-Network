//! Firewall manager routes under `/security/firewall`.

use crate::installer::AppState;
use crate::panel_admin::is_panel_admin;
use crate::panel_hub_http::{html_ok, login_redirect, redirect_notice, require_panel_user};
use crate::panel_hub_pages_firewall::firewall_manager_page;
use crate::panel_ops_firewall::{
    add_rule_live, add_trusted_live, ban_ip_live, delete_rule_live, export_banned_json,
    export_rules_json, import_banned_json, import_rules_json, reload_firewall, remove_trusted_live,
    start_firewall, stop_firewall, unban_ip_live, verify_firewall_csrf,
};
use crate::panel_pages::panel_shell;
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use std::collections::HashMap;
use std::sync::Arc;

fn peer_ip(http: &HttpRequest) -> Option<String> {
    http.peer_addr().map(|a| a.ip().to_string())
}

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

fn tab_base(tab: &str) -> String {
    format!("/security/firewall?tab={tab}")
}

fn require_admin_csrf(
    http: &HttpRequest,
    user: &str,
    form: &HashMap<String, String>,
    tab: &str,
) -> Option<HttpResponse> {
    if !is_panel_admin(user) {
        return Some(redirect_notice(
            &tab_base(tab),
            None,
            Some("Only the panel admin can manage the firewall"),
        ));
    }
    if !same_origin_ok(http) {
        return Some(redirect_notice(
            &tab_base(tab),
            None,
            Some("Rejected cross-origin form post"),
        ));
    }
    let csrf = form.get("csrf").map(String::as_str).unwrap_or("");
    if !verify_firewall_csrf(user, csrf) {
        return Some(redirect_notice(
            &tab_base(tab),
            None,
            Some("Invalid or expired CSRF token"),
        ));
    }
    None
}

#[get("/security/firewall")]
pub async fn security_firewall(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let tab = query.get("tab").map(String::as_str).unwrap_or("rules");
    html_ok(panel_shell(
        &user,
        "security",
        "Firewall",
        &firewall_manager_page(
            &user,
            tab,
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
            is_panel_admin(&user),
            query.get("q").map(String::as_str),
            peer_ip(&http).as_deref(),
        ),
    ))
}

fn do_firewall_start(
    http: &HttpRequest,
    state: &web::Data<Arc<AppState>>,
    form: &HashMap<String, String>,
) -> HttpResponse {
    let Some(user) = require_panel_user(state, http) else {
        return login_redirect(http);
    };
    if let Some(resp) = require_admin_csrf(http, &user, form, "rules") {
        return resp;
    }
    match start_firewall() {
        Ok(msg) => redirect_notice(&tab_base("rules"), Some(&msg), None),
        Err(err) => redirect_notice(&tab_base("rules"), None, Some(&err)),
    }
}

#[post("/security/firewall/enable")]
pub async fn security_firewall_enable(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    do_firewall_start(&http, &state, &form)
}

#[post("/security/firewall/start")]
pub async fn security_firewall_start(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    do_firewall_start(&http, &state, &form)
}

#[post("/security/firewall/stop")]
pub async fn security_firewall_stop(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Some(resp) = require_admin_csrf(&http, &user, &form, "rules") {
        return resp;
    }
    match stop_firewall() {
        Ok(msg) => redirect_notice(&tab_base("rules"), Some(&msg), None),
        Err(err) => redirect_notice(&tab_base("rules"), None, Some(&err)),
    }
}

#[post("/security/firewall/reload")]
pub async fn security_firewall_reload(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Some(resp) = require_admin_csrf(&http, &user, &form, "rules") {
        return resp;
    }
    match reload_firewall() {
        Ok(msg) => redirect_notice(&tab_base("rules"), Some(&msg), None),
        Err(err) => redirect_notice(&tab_base("rules"), None, Some(&err)),
    }
}

#[post("/security/firewall/rules/add")]
pub async fn security_firewall_rule_add(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Some(resp) = require_admin_csrf(&http, &user, &form, "rules") {
        return resp;
    }
    let name = form.get("name").map(String::as_str).unwrap_or("");
    let protocol = form.get("protocol").map(String::as_str).unwrap_or("tcp");
    let port = form.get("port").map(String::as_str).unwrap_or("");
    let source = form.get("source").map(String::as_str).unwrap_or("0.0.0.0/0");
    match add_rule_live(name, protocol, port, source) {
        Ok(msg) => redirect_notice(&tab_base("rules"), Some(&msg), None),
        Err(err) => redirect_notice(&tab_base("rules"), None, Some(&err)),
    }
}

#[post("/security/firewall/rules/delete")]
pub async fn security_firewall_rule_delete(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Some(resp) = require_admin_csrf(&http, &user, &form, "rules") {
        return resp;
    }
    let id = form.get("id").map(String::as_str).unwrap_or("");
    match delete_rule_live(id) {
        Ok(msg) => redirect_notice(&tab_base("rules"), Some(&msg), None),
        Err(err) => redirect_notice(&tab_base("rules"), None, Some(&err)),
    }
}

#[post("/security/firewall/rules/import")]
pub async fn security_firewall_rules_import(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Some(resp) = require_admin_csrf(&http, &user, &form, "rules") {
        return resp;
    }
    let payload = form.get("payload").map(String::as_str).unwrap_or("");
    match import_rules_json(payload) {
        Ok(msg) => redirect_notice(&tab_base("rules"), Some(&msg), None),
        Err(err) => redirect_notice(&tab_base("rules"), None, Some(&err)),
    }
}

#[get("/security/firewall/export/rules")]
pub async fn security_firewall_export_rules(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !is_panel_admin(&user) {
        return redirect_notice(
            &tab_base("rules"),
            None,
            Some("Only the panel admin can export rules"),
        );
    }
    HttpResponse::Ok()
        .content_type("application/json; charset=utf-8")
        .append_header((
            "Content-Disposition",
            "attachment; filename=\"cpn-firewall-rules.json\"",
        ))
        .body(export_rules_json())
}

#[post("/security/firewall/banned/add")]
pub async fn security_firewall_ban_add(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Some(resp) = require_admin_csrf(&http, &user, &form, "banned") {
        return resp;
    }
    let ip = form.get("ip").map(String::as_str).unwrap_or("");
    let reason = form.get("reason").map(String::as_str).unwrap_or("");
    let duration = form
        .get("duration")
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|v| *v > 0);
    match ban_ip_live(ip, reason, duration) {
        Ok(msg) => redirect_notice(&tab_base("banned"), Some(&msg), None),
        Err(err) => redirect_notice(&tab_base("banned"), None, Some(&err)),
    }
}

fn do_firewall_unban(
    http: &HttpRequest,
    state: &web::Data<Arc<AppState>>,
    form: &HashMap<String, String>,
) -> HttpResponse {
    let Some(user) = require_panel_user(state, http) else {
        return login_redirect(http);
    };
    if let Some(resp) = require_admin_csrf(http, &user, form, "banned") {
        return resp;
    }
    let ip = form.get("ip").map(String::as_str).unwrap_or("");
    match unban_ip_live(ip) {
        Ok(msg) => redirect_notice(&tab_base("banned"), Some(&msg), None),
        Err(err) => redirect_notice(&tab_base("banned"), None, Some(&err)),
    }
}

#[post("/security/firewall/banned/unban")]
pub async fn security_firewall_ban_unban(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    do_firewall_unban(&http, &state, &form)
}

#[post("/security/firewall/banned/delete")]
pub async fn security_firewall_ban_delete(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    do_firewall_unban(&http, &state, &form)
}

#[get("/security/firewall/export/banned")]
pub async fn security_firewall_export_banned(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !is_panel_admin(&user) {
        return redirect_notice(
            &tab_base("banned"),
            None,
            Some("Only the panel admin can export banned IPs"),
        );
    }
    HttpResponse::Ok()
        .content_type("application/json; charset=utf-8")
        .append_header((
            "Content-Disposition",
            "attachment; filename=\"cpn-firewall-banned.json\"",
        ))
        .body(export_banned_json())
}

#[post("/security/firewall/banned/import")]
pub async fn security_firewall_banned_import(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Some(resp) = require_admin_csrf(&http, &user, &form, "banned") {
        return resp;
    }
    let payload = form.get("payload").map(String::as_str).unwrap_or("");
    match import_banned_json(payload) {
        Ok(msg) => redirect_notice(&tab_base("banned"), Some(&msg), None),
        Err(err) => redirect_notice(&tab_base("banned"), None, Some(&err)),
    }
}

#[post("/security/firewall/trusted/add")]
pub async fn security_firewall_trusted_add(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Some(resp) = require_admin_csrf(&http, &user, &form, "trusted") {
        return resp;
    }
    let ip = form.get("ip").map(String::as_str).unwrap_or("");
    let label = form.get("label").map(String::as_str).unwrap_or("");
    match add_trusted_live(ip, label) {
        Ok(msg) => redirect_notice(&tab_base("trusted"), Some(&msg), None),
        Err(err) => redirect_notice(&tab_base("trusted"), None, Some(&err)),
    }
}

#[post("/security/firewall/trusted/delete")]
pub async fn security_firewall_trusted_delete(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Some(resp) = require_admin_csrf(&http, &user, &form, "trusted") {
        return resp;
    }
    let ip = form.get("ip").map(String::as_str).unwrap_or("");
    match remove_trusted_live(ip) {
        Ok(msg) => redirect_notice(&tab_base("trusted"), Some(&msg), None),
        Err(err) => redirect_notice(&tab_base("trusted"), None, Some(&err)),
    }
}
