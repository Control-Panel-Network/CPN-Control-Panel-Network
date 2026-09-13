//! Server and Settings hub routes.

use crate::installer::AppState;
use crate::panel_admin::is_panel_admin;
use crate::panel_hub_http::{html_ok, login_redirect, redirect_notice, require_panel_user};
use crate::panel_hub_pages_litespeed::{
    litespeed_manage_page, open_ols_page, open_olse_page, run_apply_serial, run_downgrade,
    run_set_tier, run_set_webadmin_url, run_upgrade,
};
use crate::panel_hub_pages_server::{
    docker_page, files_page, package_manager_page, php_configs_page, php_extensions_page,
    php_tuning_page, processes_page, run_service_control, server_hub_main, services_page,
};
use crate::panel_hub_pages_server_net::{
    change_port_page, dns_zones_page, nameservers_page, remove_dns_zone, save_dns_zone,
    save_ns_lines,
};
use crate::panel_hub_pages_settings::{
    connect_page, design_settings_page, settings_hub_main, setup_wizard_page,
    version_management_page,
};
use crate::panel_hub_pages_site_messages::site_messages_settings_page;
use crate::panel_pages::panel_shell;
use crate::site_messages::{
    SiteMessageDefaults, builtin_site_ready_html, load_defaults, restore_factory_site_ready,
    restore_factory_suspend_message, save_defaults,
};
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use std::sync::Arc;

#[get("/server")]
pub async fn server_page(http: HttpRequest, state: web::Data<Arc<AppState>>) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
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
        return login_redirect(&http);
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

#[get("/server/openlitespeed")]
pub async fn server_openlitespeed_page(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    html_ok(panel_shell(
        &user,
        "server",
        "Open OLS",
        &open_ols_page(
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}

#[post("/server/openlitespeed/password")]
pub async fn server_openlitespeed_password(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    if let Some(resp) = litespeed_admin_redirect(&state, &http) {
        return resp;
    }
    let user = form.get("username").map(String::as_str).unwrap_or("admin");
    let pass = form.get("password").map(String::as_str).unwrap_or("");
    match crate::litespeed_webadmin_users::set_webadmin_password(user, pass) {
        Ok(msg) => redirect_notice("/server/openlitespeed", Some(&msg), None),
        Err(err) => redirect_notice("/server/openlitespeed", None, Some(&err)),
    }
}

#[post("/server/openlitespeed/reset-cpn")]
pub async fn server_openlitespeed_reset_cpn(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    if let Some(resp) = litespeed_admin_redirect(&state, &http) {
        return resp;
    }
    let confirmed = form
        .get("confirm")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("yes") || v.eq_ignore_ascii_case("on"))
        .unwrap_or(false);
    if !confirmed {
        return redirect_notice(
            "/server/openlitespeed",
            None,
            Some("Confirm the reset checkbox to align WebAdmin with the CPN admin account."),
        );
    }
    let Some(pass) = form
        .get("password")
        .map(|s| s.as_str())
        .filter(|p| !p.is_empty())
    else {
        return redirect_notice(
            "/server/openlitespeed",
            None,
            Some("CPN admin password is required to reset WebAdmin."),
        );
    };
    match crate::litespeed_webadmin_users::reset_webadmin_to_cpn_admin(pass) {
        Ok(msg) => redirect_notice("/server/openlitespeed", Some(&msg), None),
        Err(err) => redirect_notice("/server/openlitespeed", None, Some(&err)),
    }
}

#[post("/server/openlitespeed/guest")]
pub async fn server_openlitespeed_guest(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    if let Some(resp) = litespeed_admin_redirect(&state, &http) {
        return resp;
    }
    let user = form.get("username").map(String::as_str).unwrap_or("");
    let pass = form.get("password").map(String::as_str).unwrap_or("");
    match crate::litespeed_webadmin_users::add_webadmin_guest(user, pass) {
        Ok(msg) => redirect_notice("/server/openlitespeed", Some(&msg), None),
        Err(err) => redirect_notice("/server/openlitespeed", None, Some(&err)),
    }
}

#[post("/server/openlitespeed/guest/remove")]
pub async fn server_openlitespeed_guest_remove(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    if let Some(resp) = litespeed_admin_redirect(&state, &http) {
        return resp;
    }
    let user = form.get("username").map(String::as_str).unwrap_or("");
    match crate::litespeed_webadmin_users::remove_webadmin_user(user) {
        Ok(msg) => redirect_notice("/server/openlitespeed", Some(&msg), None),
        Err(err) => redirect_notice("/server/openlitespeed", None, Some(&err)),
    }
}

#[get("/server/litespeed-enterprise")]
pub async fn server_litespeed_enterprise_page(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    html_ok(panel_shell(
        &user,
        "server",
        "Open OLSE",
        &open_olse_page(
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}

#[get("/server/litespeed")]
pub async fn server_litespeed_page(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    html_ok(panel_shell(
        &user,
        "server",
        "LiteSpeed plans",
        &litespeed_manage_page(
            is_panel_admin(&user),
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}

fn litespeed_admin_redirect(
    state: &web::Data<Arc<AppState>>,
    http: &HttpRequest,
) -> Option<HttpResponse> {
    let Some(user) = require_panel_user(state, http) else {
        return Some(login_redirect(http));
    };
    if !is_panel_admin(&user) {
        return Some(redirect_notice(
            "/server/litespeed",
            None,
            Some("Only the panel admin can manage LiteSpeed."),
        ));
    }
    None
}

#[post("/server/litespeed/tier")]
pub async fn server_litespeed_tier(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    if let Some(resp) = litespeed_admin_redirect(&state, &http) {
        return resp;
    }
    let tier = form.get("tier").map(String::as_str).unwrap_or("");
    match run_set_tier(tier) {
        Ok(msg) => redirect_notice("/server/litespeed", Some(&msg), None),
        Err(err) => redirect_notice("/server/litespeed", None, Some(&err)),
    }
}

#[post("/server/litespeed/serial")]
pub async fn server_litespeed_serial(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    if let Some(resp) = litespeed_admin_redirect(&state, &http) {
        return resp;
    }
    let serial = form.get("serial").map(String::as_str).unwrap_or("");
    match run_apply_serial(serial) {
        Ok(msg) => redirect_notice("/server/litespeed", Some(&msg), None),
        Err(err) => redirect_notice("/server/litespeed", None, Some(&err)),
    }
}

#[post("/server/litespeed/webadmin-url")]
pub async fn server_litespeed_webadmin_url(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    if let Some(resp) = litespeed_admin_redirect(&state, &http) {
        return resp;
    }
    let url = form.get("webadmin_url").map(String::as_str).unwrap_or("");
    match run_set_webadmin_url(url) {
        Ok(msg) => redirect_notice("/server/litespeed", Some(&msg), None),
        Err(err) => redirect_notice("/server/litespeed", None, Some(&err)),
    }
}

#[post("/server/litespeed/upgrade")]
pub async fn server_litespeed_upgrade(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    if let Some(resp) = litespeed_admin_redirect(&state, &http) {
        return resp;
    }
    match run_upgrade() {
        Ok(msg) => redirect_notice("/server/litespeed", Some(&msg), None),
        Err(err) => redirect_notice("/server/litespeed", None, Some(&err)),
    }
}

#[post("/server/litespeed/downgrade")]
pub async fn server_litespeed_downgrade(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    if let Some(resp) = litespeed_admin_redirect(&state, &http) {
        return resp;
    }
    let version = form.get("version").map(String::as_str).unwrap_or("");
    match run_downgrade(version) {
        Ok(msg) => redirect_notice("/server/litespeed", Some(&msg), None),
        Err(err) => redirect_notice("/server/litespeed", None, Some(&err)),
    }
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
        return login_redirect(&http);
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
        return login_redirect(&http);
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
        return login_redirect(&http);
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
        return login_redirect(&http);
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
        return login_redirect(&http);
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
        return login_redirect(&http);
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
        return login_redirect(&http);
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
        return login_redirect(&http);
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
        return login_redirect(&http);
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
        return login_redirect(&http);
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
        return login_redirect(&http);
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
        return login_redirect(&http);
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
        return login_redirect(&http);
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
        return login_redirect(&http);
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
        return login_redirect(&http);
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
        return login_redirect(&http);
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
        return login_redirect(&http);
    };
    html_ok(panel_shell(
        &user,
        "settings",
        "Settings",
        &settings_hub_main(),
    ))
}

#[get("/settings/version")]
pub async fn settings_version_page(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let can_manage = is_panel_admin(&user);
    html_ok(panel_shell(
        &user,
        "settings",
        "Version Management",
        &version_management_page(can_manage),
    ))
}

#[get("/settings/design")]
pub async fn settings_design_page(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    html_ok(panel_shell(
        &user,
        "settings",
        "Design",
        &design_settings_page(&user),
    ))
}

#[get("/settings/setup")]
pub async fn settings_setup_page(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    html_ok(panel_shell(
        &user,
        "settings",
        "Setup Wizard",
        &setup_wizard_page(),
    ))
}

#[get("/settings/connect")]
pub async fn settings_connect_page(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    html_ok(panel_shell(&user, "settings", "Connect", &connect_page()))
}

#[get("/settings/site-messages")]
pub async fn settings_site_messages_page(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !is_panel_admin(&user) {
        return HttpResponse::SeeOther()
            .append_header(("Location", "/settings?error=Admin%20only"))
            .finish();
    }
    html_ok(panel_shell(
        &user,
        "settings",
        "Site messages",
        &site_messages_settings_page(
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}

#[derive(Debug, serde::Deserialize)]
pub struct SiteMessagesForm {
    #[serde(default)]
    suspend_message_html: String,
    #[serde(default)]
    site_ready_html: String,
}

#[post("/settings/site-messages")]
pub async fn settings_site_messages_save(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<SiteMessagesForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !is_panel_admin(&user) {
        return HttpResponse::SeeOther()
            .append_header(("Location", "/settings?error=Admin%20only"))
            .finish();
    }
    let mut defaults = load_defaults();
    defaults.suspend_message_html = form.suspend_message_html.clone();
    defaults.site_ready_html = form.site_ready_html.clone();
    match save_defaults(&defaults) {
        Ok(()) => redirect_notice("/settings/site-messages", Some("Site messages saved"), None),
        Err(error) => redirect_notice("/settings/site-messages", None, Some(&error)),
    }
}

#[post("/settings/site-messages/restore-suspend")]
pub async fn settings_site_messages_restore_suspend(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !is_panel_admin(&user) {
        return HttpResponse::SeeOther()
            .append_header(("Location", "/settings?error=Admin%20only"))
            .finish();
    }
    match restore_factory_suspend_message() {
        Ok(()) => redirect_notice(
            "/settings/site-messages",
            Some("Factory suspend message restored"),
            None,
        ),
        Err(error) => redirect_notice("/settings/site-messages", None, Some(&error)),
    }
}

#[post("/settings/site-messages/restore-site-ready")]
pub async fn settings_site_messages_restore_site_ready(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !is_panel_admin(&user) {
        return HttpResponse::SeeOther()
            .append_header(("Location", "/settings?error=Admin%20only"))
            .finish();
    }
    match restore_factory_site_ready() {
        Ok(()) => redirect_notice(
            "/settings/site-messages",
            Some("Factory site-ready template restored"),
            None,
        ),
        Err(error) => redirect_notice("/settings/site-messages", None, Some(&error)),
    }
}

#[post("/settings/site-messages/reset-builtins")]
pub async fn settings_site_messages_reset(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !is_panel_admin(&user) {
        return HttpResponse::SeeOther()
            .append_header(("Location", "/settings?error=Admin%20only"))
            .finish();
    }
    match save_defaults(&SiteMessageDefaults {
        site_ready_html: builtin_site_ready_html().to_string(),
        ..SiteMessageDefaults::default()
    }) {
        Ok(()) => redirect_notice(
            "/settings/site-messages",
            Some("Built-in defaults restored"),
            None,
        ),
        Err(error) => redirect_notice("/settings/site-messages", None, Some(&error)),
    }
}

#[get("/settings/port")]
pub async fn settings_port_page(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
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
