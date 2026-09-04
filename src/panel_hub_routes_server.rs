//! Server, Settings, and Security hub routes.

use crate::installer::AppState;
use crate::panel_admin::is_panel_admin;
use crate::panel_hub_http::{html_ok, login_redirect, redirect_notice, require_panel_user};
use crate::panel_hub_pages_backups::{security_stub_page, settings_stub_page};
use crate::panel_hub_pages_server::{
    docker_page, files_page, package_manager_page, php_configs_page, php_extensions_page,
    php_tuning_page, processes_page, run_service_control, server_hub_main, services_page,
};
use crate::panel_hub_pages_server_net::{
    change_port_page, dns_zones_page, nameservers_page, remove_dns_zone, save_dns_zone,
    save_ns_lines,
};
use crate::panel_pages::panel_shell;
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use std::sync::Arc;

#[get("/server")]
pub async fn server_page(http: HttpRequest, state: web::Data<Arc<AppState>>) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    html_ok(panel_shell(&user, "server", "Server", &server_hub_main()))
}

#[get("/server/services")]
pub async fn server_services_page(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    html_ok(panel_shell(
        &user,
        "server",
        "Services Status",
        &services_page(
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
            is_panel_admin(&user),
        ),
    ))
}

#[derive(Debug, serde::Deserialize)]
pub struct ServiceControlForm {
    #[serde(default)]
    unit: String,
    #[serde(default)]
    action: String,
}

#[post("/server/services/control")]
pub async fn server_services_control(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<ServiceControlForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    match run_service_control(&user, form.unit.trim(), form.action.trim()) {
        Ok(msg) => redirect_notice("/server/services", Some(&msg), None),
        Err(err) => redirect_notice("/server/services", None, Some(&err)),
    }
}

#[get("/server/processes")]
pub async fn server_processes_page(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    html_ok(panel_shell(
        &user,
        "server",
        "Top Processes",
        &processes_page(),
    ))
}

#[get("/server/php/extensions")]
pub async fn server_php_extensions(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    html_ok(panel_shell(
        &user,
        "server",
        "PHP Extensions",
        &php_extensions_page(),
    ))
}

#[get("/server/php/configs")]
pub async fn server_php_configs(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    html_ok(panel_shell(
        &user,
        "server",
        "PHP Configs",
        &php_configs_page(),
    ))
}

#[get("/server/php/tuning")]
pub async fn server_php_tuning(http: HttpRequest, state: web::Data<Arc<AppState>>) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    html_ok(panel_shell(
        &user,
        "server",
        "PHP Tuning",
        &php_tuning_page(),
    ))
}

#[get("/server/packages")]
pub async fn server_packages_page(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    let q = query.get("q").map(String::as_str).unwrap_or("");
    html_ok(panel_shell(
        &user,
        "server",
        "Package Manager",
        &package_manager_page(q),
    ))
}

#[get("/server/docker/apps")]
pub async fn server_docker_apps(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    html_ok(panel_shell(
        &user,
        "server",
        "Docker Apps",
        &docker_page("Docker Apps"),
    ))
}

#[get("/server/docker/containers")]
pub async fn server_docker_containers(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    html_ok(panel_shell(
        &user,
        "server",
        "Containers",
        &docker_page("Containers"),
    ))
}

#[get("/server/docker/images")]
pub async fn server_docker_images(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    html_ok(panel_shell(
        &user,
        "server",
        "Docker Images",
        &docker_page("Docker Images"),
    ))
}

#[get("/server/files")]
pub async fn server_files_page(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    if !is_panel_admin(&user) {
        return html_ok(panel_shell(
            &user,
            "server",
            "Root File Manager",
            &files_page("/home", Some("Only the panel admin can browse root paths.")),
        ));
    }
    let path = query.get("path").map(String::as_str).unwrap_or("/home");
    html_ok(panel_shell(
        &user,
        "server",
        "Root File Manager",
        &files_page(path, None),
    ))
}

#[get("/server/dns/zones")]
pub async fn server_dns_zones(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    html_ok(panel_shell(
        &user,
        "server",
        "DNS Zones",
        &dns_zones_page(
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}

#[derive(Debug, serde::Deserialize)]
pub struct DnsZoneForm {
    #[serde(default)]
    name: String,
    #[serde(default)]
    content: String,
}

#[post("/server/dns/zones/save")]
pub async fn server_dns_zones_save(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<DnsZoneForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    if !is_panel_admin(&user) {
        return redirect_notice("/server/dns/zones", None, Some("Admin only"));
    }
    match save_dns_zone(&form.name, &form.content) {
        Ok(msg) => redirect_notice("/server/dns/zones", Some(&msg), None),
        Err(err) => redirect_notice("/server/dns/zones", None, Some(&err)),
    }
}

#[post("/server/dns/zones/delete")]
pub async fn server_dns_zones_delete(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<DnsZoneForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    if !is_panel_admin(&user) {
        return redirect_notice("/server/dns/zones", None, Some("Admin only"));
    }
    match remove_dns_zone(&form.name) {
        Ok(msg) => redirect_notice("/server/dns/zones", Some(&msg), None),
        Err(err) => redirect_notice("/server/dns/zones", None, Some(&err)),
    }
}

#[get("/server/dns/nameservers")]
pub async fn server_dns_nameservers(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    html_ok(panel_shell(
        &user,
        "server",
        "Nameservers",
        &nameservers_page(
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
            false,
        ),
    ))
}

#[get("/server/dns/defaults")]
pub async fn server_dns_defaults(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    html_ok(panel_shell(
        &user,
        "server",
        "Default Nameservers",
        &nameservers_page(
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
            true,
        ),
    ))
}

#[derive(Debug, serde::Deserialize)]
pub struct NameserversForm {
    #[serde(default)]
    nameservers: String,
}

#[post("/server/dns/nameservers/save")]
pub async fn server_dns_nameservers_save(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<NameserversForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    if !is_panel_admin(&user) {
        return redirect_notice("/server/dns/nameservers", None, Some("Admin only"));
    }
    match save_ns_lines(&form.nameservers) {
        Ok(msg) => redirect_notice("/server/dns/nameservers", Some(&msg), None),
        Err(err) => redirect_notice("/server/dns/nameservers", None, Some(&err)),
    }
}

#[get("/settings")]
pub async fn settings_page(http: HttpRequest, state: web::Data<Arc<AppState>>) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    html_ok(panel_shell(
        &user,
        "settings",
        "Settings",
        &settings_stub_page(),
    ))
}

#[get("/settings/port")]
pub async fn settings_port_page(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    html_ok(panel_shell(
        &user,
        "settings",
        "Change Port",
        &change_port_page(
            state.bind_port,
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}

#[get("/security")]
pub async fn security_page(http: HttpRequest, state: web::Data<Arc<AppState>>) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    html_ok(panel_shell(
        &user,
        "security",
        "Security",
        &security_stub_page(),
    ))
}
