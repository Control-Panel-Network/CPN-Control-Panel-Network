//! DNS Zones, Nameservers, and Default Nameservers routes.

use crate::installer::AppState;
use crate::panel_admin::is_panel_admin;
use crate::panel_host_info::host_sidebar_info;
use crate::panel_hub_http::{html_ok, login_redirect, redirect_notice, require_panel_user};
use crate::panel_hub_pages_dns::{dns_zone_create_page, dns_zone_manage_page, dns_zones_page};
use crate::panel_hub_pages_dns_ns::{default_nameservers_page, nameservers_manage_page};
use crate::panel_ops_dns::{
    DnsRecord, add_ns_host, add_zone_record, create_zone, delete_ns_host, delete_zone,
    delete_zone_record, save_default_nameservers, verify_dns_csrf, write_zone,
};
use crate::panel_pages::panel_shell;
use actix_web::{HttpRequest, HttpResponse, get, post, web};
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

fn require_admin_csrf(
    http: &HttpRequest,
    user: &str,
    form: &HashMap<String, String>,
    back: &str,
) -> Option<HttpResponse> {
    if !is_panel_admin(user) {
        return Some(redirect_notice(back, None, Some("Admin only")));
    }
    if !same_origin_ok(http) {
        return Some(redirect_notice(
            back,
            None,
            Some("Rejected cross-origin form post"),
        ));
    }
    let csrf = form.get("csrf").map(String::as_str).unwrap_or("");
    if !verify_dns_csrf(user, csrf) {
        return Some(redirect_notice(
            back,
            None,
            Some("Invalid or expired CSRF token"),
        ));
    }
    None
}

#[get("/server/dns/zones")]
pub async fn server_dns_zones(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    html_ok(panel_shell(
        &user,
        "server",
        "DNS Zones",
        &dns_zones_page(
            &user,
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}

#[get("/server/dns/zones/create")]
pub async fn server_dns_zones_create_get(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    html_ok(panel_shell(
        &user,
        "server",
        "Create DNS Zone",
        &dns_zone_create_page(
            &user,
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}

#[post("/server/dns/zones/create")]
pub async fn server_dns_zones_create_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Some(deny) = require_admin_csrf(&http, &user, &form, "/server/dns/zones/create") {
        return deny;
    }
    let name = form.get("name").map(String::as_str).unwrap_or("");
    let host_ip = host_sidebar_info().ip;
    match create_zone(name, Some(&host_ip)) {
        Ok(zone) => redirect_notice(
            &format!("/server/dns/zones/manage?name={zone}"),
            Some(&format!("Created zone {zone}")),
            None,
        ),
        Err(err) => redirect_notice("/server/dns/zones/create", None, Some(&err)),
    }
}

#[get("/server/dns/zones/manage")]
pub async fn server_dns_zones_manage(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let Some(name) = query.get("name").filter(|s| !s.is_empty()) else {
        return redirect_notice("/server/dns/zones", None, Some("Missing zone name"));
    };
    html_ok(panel_shell(
        &user,
        "server",
        "Manage DNS Zone",
        &dns_zone_manage_page(
            &user,
            name,
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}

#[post("/server/dns/zones/save")]
pub async fn server_dns_zones_save(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let name = form.get("name").cloned().unwrap_or_default();
    let back = if name.is_empty() {
        "/server/dns/zones".to_string()
    } else {
        format!("/server/dns/zones/manage?name={name}")
    };
    if let Some(deny) = require_admin_csrf(&http, &user, &form, &back) {
        return deny;
    }
    let content = form.get("content").map(String::as_str).unwrap_or("");
    match write_zone(&name, content) {
        Ok(()) => redirect_notice(&back, Some("Saved zone file"), None),
        Err(err) => redirect_notice(&back, None, Some(&err)),
    }
}

#[post("/server/dns/zones/delete")]
pub async fn server_dns_zones_delete(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Some(deny) = require_admin_csrf(&http, &user, &form, "/server/dns/zones") {
        return deny;
    }
    let name = form.get("name").map(String::as_str).unwrap_or("");
    match delete_zone(name) {
        Ok(()) => redirect_notice(
            "/server/dns/zones",
            Some(&format!("Deleted zone {}", name.trim())),
            None,
        ),
        Err(err) => redirect_notice("/server/dns/zones", None, Some(&err)),
    }
}

#[post("/server/dns/zones/record/add")]
pub async fn server_dns_record_add(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let zone = form.get("zone").cloned().unwrap_or_default();
    let back = format!("/server/dns/zones/manage?name={zone}");
    if let Some(deny) = require_admin_csrf(&http, &user, &form, &back) {
        return deny;
    }
    let ttl = form
        .get("ttl")
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(3600);
    let priority = form.get("priority").and_then(|s| s.parse::<u16>().ok());
    let weight = form.get("weight").and_then(|s| s.parse::<u16>().ok());
    let port = form.get("port").and_then(|s| s.parse::<u16>().ok());
    let rec = DnsRecord {
        id: String::new(),
        name: form
            .get("record_name")
            .cloned()
            .unwrap_or_else(|| "@".into()),
        rtype: form.get("rtype").cloned().unwrap_or_else(|| "A".into()),
        ttl,
        priority,
        weight,
        port,
        content: form.get("content").cloned().unwrap_or_default(),
    };
    match add_zone_record(&zone, rec) {
        Ok(()) => redirect_notice(&back, Some("Record added"), None),
        Err(err) => redirect_notice(&back, None, Some(&err)),
    }
}

#[post("/server/dns/zones/record/delete")]
pub async fn server_dns_record_delete(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let zone = form.get("zone").cloned().unwrap_or_default();
    let back = format!("/server/dns/zones/manage?name={zone}");
    if let Some(deny) = require_admin_csrf(&http, &user, &form, &back) {
        return deny;
    }
    let id = form.get("id").map(String::as_str).unwrap_or("");
    match delete_zone_record(&zone, id) {
        Ok(()) => redirect_notice(&back, Some("Record deleted"), None),
        Err(err) => redirect_notice(&back, None, Some(&err)),
    }
}

#[get("/server/dns/nameservers")]
pub async fn server_dns_nameservers(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    html_ok(panel_shell(
        &user,
        "server",
        "Nameservers",
        &nameservers_manage_page(
            &user,
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}

#[post("/server/dns/nameservers/add")]
pub async fn server_dns_nameservers_add(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Some(deny) = require_admin_csrf(&http, &user, &form, "/server/dns/nameservers") {
        return deny;
    }
    let hostname = form.get("hostname").map(String::as_str).unwrap_or("");
    let ipv4 = form.get("ipv4").map(String::as_str);
    let ipv6 = form.get("ipv6").map(String::as_str);
    match add_ns_host(hostname, ipv4, ipv6) {
        Ok(h) => redirect_notice(
            "/server/dns/nameservers",
            Some(&format!("Added nameserver {h}")),
            None,
        ),
        Err(err) => redirect_notice("/server/dns/nameservers", None, Some(&err)),
    }
}

#[post("/server/dns/nameservers/delete")]
pub async fn server_dns_nameservers_delete(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Some(deny) = require_admin_csrf(&http, &user, &form, "/server/dns/nameservers") {
        return deny;
    }
    let hostname = form.get("hostname").map(String::as_str).unwrap_or("");
    match delete_ns_host(hostname) {
        Ok(()) => redirect_notice("/server/dns/nameservers", Some("Deleted nameserver"), None),
        Err(err) => redirect_notice("/server/dns/nameservers", None, Some(&err)),
    }
}

#[get("/server/dns/defaults")]
pub async fn server_dns_defaults(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    html_ok(panel_shell(
        &user,
        "server",
        "Default Nameservers",
        &default_nameservers_page(
            &user,
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}

#[post("/server/dns/defaults/save")]
pub async fn server_dns_defaults_save(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Some(deny) = require_admin_csrf(&http, &user, &form, "/server/dns/defaults") {
        return deny;
    }
    let mut values: Vec<String> = Vec::new();
    for key in ["nameservers", "extra", "ns"] {
        if let Some(raw) = form.get(key) {
            for line in raw.lines() {
                let t = line.trim();
                if !t.is_empty() && !values.iter().any(|x| x.eq_ignore_ascii_case(t)) {
                    values.push(t.to_string());
                }
            }
        }
    }
    match save_default_nameservers(&values) {
        Ok(()) => redirect_notice(
            "/server/dns/defaults",
            Some(&format!("Saved {} default nameserver(s)", values.len())),
            None,
        ),
        Err(err) => redirect_notice("/server/dns/defaults", None, Some(&err)),
    }
}

/// Legacy POST path used by older textarea UI; keep for bookmarks.
#[post("/server/dns/nameservers/save")]
pub async fn server_dns_nameservers_save(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Some(deny) = require_admin_csrf(&http, &user, &form, "/server/dns/nameservers") {
        return deny;
    }
    let raw = form
        .get("nameservers")
        .or_else(|| form.get("extra"))
        .map(String::as_str)
        .unwrap_or("");
    let values: Vec<String> = raw
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect();
    match save_default_nameservers(&values) {
        Ok(()) => redirect_notice(
            "/server/dns/defaults",
            Some(&format!("Saved {} nameserver(s)", values.len())),
            None,
        ),
        Err(err) => redirect_notice("/server/dns/nameservers", None, Some(&err)),
    }
}
