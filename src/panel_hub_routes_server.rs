//! Server and Settings hub routes.

use crate::installer::AppState;
use crate::panel_admin::is_panel_admin;
use crate::panel_hub_http::{
    html_blocking, html_ok, login_redirect, owner_only_html, redirect_flash, redirect_notice,
    require_panel_user, urlencoding_simple,
};
use crate::panel_hub_pages_docker::{
    docker_create_page, docker_images_page, docker_logs_page, docker_manage_page,
};
use crate::panel_hub_pages_docker_stacks::docker_stacks_page;
use crate::panel_hub_pages_docker_view::docker_container_view_page;
use crate::panel_hub_pages_litespeed::{
    litespeed_manage_page, open_ols_page, open_olse_page, run_apply_serial, run_downgrade,
    run_set_tier, run_set_webadmin_url, run_upgrade,
};
use crate::panel_hub_pages_server::{
    docker_page, package_manager_page, php_tuning_page, processes_page, run_service_control,
    server_hub_main, services_page,
};
use crate::panel_hub_pages_server_net::change_port_page;
use crate::panel_hub_pages_settings::{
    connect_page, design_settings_page_with_tab, design_settings_tab, settings_hub_main_with,
    setup_wizard_page_with, version_management_page,
};
use crate::panel_hub_pages_site_messages::site_messages_settings_page;
use crate::panel_pages::panel_shell;
use crate::site_messages::{
    SiteMessageDefaults, builtin_site_ready_html, load_defaults, restore_factory_site_ready,
    restore_factory_suspend_message, sanitize_site_ready_html, save_defaults,
};
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use std::sync::Arc;

#[get("/server")]
pub async fn server_page(http: HttpRequest, state: web::Data<Arc<AppState>>) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    // Host probes behind the sidebar and hub tiles run on the blocking pool with a bounded
    // wait, so a slow probe can no longer pin a worker and surface as `408 Request Timeout`.
    html_blocking(move || panel_shell(&user, "server", "Server", &server_hub_main())).await
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
    let user = form
        .get("username")
        .map(String::as_str)
        .filter(|u| !u.is_empty())
        .unwrap_or("admin");
    // Require a submitted password field (no hard-coded empty default into the sink).
    let Some(pass) = form.get("password").map(String::as_str) else {
        return redirect_notice("/server/openlitespeed", None, Some("Password is required."));
    };
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
    let user = form.get("username").map(String::as_str).unwrap_or_default();
    let Some(pass) = form.get("password").map(String::as_str) else {
        return redirect_notice("/server/openlitespeed", None, Some("Password is required."));
    };
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
    if query.contains_key("notice") || query.contains_key("error") {
        let mut loc = String::from("/server/litespeed/plans");
        let mut first = true;
        for (key, value) in query.iter() {
            loc.push(if first { '?' } else { '&' });
            first = false;
            loc.push_str(key);
            loc.push('=');
            loc.push_str(&urlencoding_simple(value));
        }
        return HttpResponse::SeeOther()
            .append_header(("Location", loc))
            .finish();
    }
    html_ok(panel_shell(
        &user,
        "server",
        "LiteSpeed",
        &crate::panel_hub_pages_category_overviews::litespeed_hub_main(),
    ))
}

#[get("/server/litespeed/plans")]
pub async fn server_litespeed_plans_page(
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
        Ok(msg) => redirect_notice("/server/litespeed/plans", Some(&msg), None),
        Err(err) => redirect_notice("/server/litespeed/plans", None, Some(&err)),
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
        Ok(msg) => redirect_notice("/server/litespeed/plans", Some(&msg), None),
        Err(err) => redirect_notice("/server/litespeed/plans", None, Some(&err)),
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
        Ok(msg) => redirect_notice("/server/litespeed/plans", Some(&msg), None),
        Err(err) => redirect_notice("/server/litespeed/plans", None, Some(&err)),
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
        Ok(msg) => redirect_notice("/server/litespeed/plans", Some(&msg), None),
        Err(err) => redirect_notice("/server/litespeed/plans", None, Some(&err)),
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
        Ok(msg) => redirect_notice("/server/litespeed/plans", Some(&msg), None),
        Err(err) => redirect_notice("/server/litespeed/plans", None, Some(&err)),
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

#[actix_web::route("/server/docker/apps", method = "GET", method = "HEAD")]
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

#[actix_web::route("/server/docker/containers", method = "GET", method = "HEAD")]
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
        &docker_manage_page(None, None),
    ))
}

#[actix_web::route("/server/docker/images", method = "GET", method = "HEAD")]
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
        &docker_images_page(None, None, None, &[]),
    ))
}

#[actix_web::route("/docker", method = "GET", method = "HEAD")]
pub async fn docker_home(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let notice = query.get("notice").map(String::as_str);
    let error = query.get("error").map(String::as_str);
    if let Some(img) = query.get("image") {
        let base = format!("/docker/create?image={}", urlencoding_simple(img.as_str()));
        return redirect_notice(&base, notice, error);
    }
    if query.contains_key("notice") || query.contains_key("error") {
        let mut loc = String::from("/docker/list");
        let mut first = true;
        for (key, value) in query.iter() {
            loc.push(if first { '?' } else { '&' });
            first = false;
            loc.push_str(key);
            loc.push('=');
            loc.push_str(&urlencoding_simple(value));
        }
        return HttpResponse::SeeOther()
            .append_header(("Location", loc))
            .finish();
    }
    html_ok(panel_shell(
        &user,
        "server",
        "Docker",
        &crate::panel_hub_pages_category_overviews::docker_hub_main(),
    ))
}

#[actix_web::route("/docker/list", method = "GET", method = "HEAD")]
pub async fn docker_list_route(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let notice = query.get("notice").cloned();
    let error = query.get("error").cloned();
    html_blocking(move || {
        panel_shell(
            &user,
            "server",
            "Active Containers",
            &docker_manage_page(notice.as_deref(), error.as_deref()),
        )
    })
    .await
}

#[actix_web::route("/docker/create", method = "GET", method = "HEAD")]
pub async fn docker_create_route(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !is_panel_admin(&user) {
        return redirect_notice(
            "/docker",
            None,
            Some("Only panel admins can create containers."),
        );
    }
    let notice = query.get("notice").map(String::as_str);
    let error = query.get("error").map(String::as_str);
    let image = query.get("image").map(String::as_str);
    html_ok(panel_shell(
        &user,
        "server",
        "Create Container",
        &docker_create_page(notice, error, image),
    ))
}

#[actix_web::route("/docker/images", method = "GET", method = "HEAD")]
pub async fn docker_images_route(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let notice = query.get("notice").map(String::as_str);
    let error = query.get("error").map(String::as_str);
    let q = query.get("q").map(|s| s.as_str()).unwrap_or("");
    let hits = if q.is_empty() {
        Vec::new()
    } else {
        match crate::panel_ops_docker_images::search_docker_hub(q) {
            Ok(h) => h,
            Err(e) => {
                return html_ok(panel_shell(
                    &user,
                    "server",
                    "Docker Images",
                    &docker_images_page(notice, Some(&e), Some(q), &[]),
                ));
            }
        }
    };
    html_ok(panel_shell(
        &user,
        "server",
        "Docker Images",
        &docker_images_page(notice, error, Some(q).filter(|s| !s.is_empty()), &hits),
    ))
}

#[actix_web::route("/docker/logs", method = "GET", method = "HEAD")]
pub async fn docker_logs_route(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let name = query.get("name").map(String::as_str).unwrap_or("");
    if name.is_empty() {
        return redirect_notice("/docker/list", None, Some("Missing container name."));
    }
    html_ok(panel_shell(
        &user,
        "server",
        "Container Logs",
        &docker_logs_page(name, None, None),
    ))
}

#[derive(serde::Deserialize)]
pub struct DockerContainerForm {
    pub action: String,
    pub name: String,
    #[serde(default)]
    pub return_to: String,
}

fn docker_action_redirect_base(return_to: &str) -> String {
    let rt = return_to.trim();
    if rt.starts_with("/docker/view/")
        && !rt.contains("..")
        && !rt.contains('\n')
        && rt.len() <= 200
    {
        rt.to_string()
    } else {
        "/docker/list".into()
    }
}

#[post("/docker/container")]
pub async fn docker_container_action(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<DockerContainerForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !is_panel_admin(&user) {
        return redirect_notice(
            "/docker",
            None,
            Some("Only panel admins can change containers."),
        );
    }
    let redirect_base = docker_action_redirect_base(&form.return_to);
    match crate::panel_ops_docker::container_action(&form.action, &form.name) {
        Ok(msg) => redirect_notice(&redirect_base, Some(&msg), None),
        Err(err) => redirect_notice(&redirect_base, None, Some(&err)),
    }
}

#[actix_web::route("/docker/view/{name}", method = "GET", method = "HEAD")]
pub async fn docker_view_route(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    name: web::Path<String>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let notice = query.get("notice").map(String::as_str);
    let error = query.get("error").map(String::as_str);
    let tail = query
        .get("tail")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(200)
        .clamp(20, 500);
    html_ok(panel_shell(
        &user,
        "server",
        "Container",
        &docker_container_view_page(&name, notice, error, None, tail),
    ))
}

#[derive(serde::Deserialize)]
pub struct DockerExecForm {
    pub command: String,
}

#[post("/docker/view/{name}/exec")]
pub async fn docker_view_exec(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    name: web::Path<String>,
    form: web::Form<DockerExecForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !is_panel_admin(&user) {
        return redirect_notice(
            &format!("/docker/view/{}", urlencoding_simple(name.as_str())),
            None,
            Some("Only panel admins can run commands in containers."),
        );
    }
    let cmd = form.command.clone();
    let cname = name.to_string();
    let output =
        web::block(move || crate::panel_ops_docker_detail::container_exec_command(&cname, &cmd))
            .await
            .unwrap_or_else(|e| Err(format!("Exec task failed: {e}")));
    let exec_out = match &output {
        Ok(s) => Some(s.as_str()),
        Err(e) => Some(e.as_str()),
    };
    html_ok(panel_shell(
        &user,
        "server",
        "Container",
        &docker_container_view_page(name.as_str(), None, None, exec_out, 200),
    ))
}

#[actix_web::route("/docker/export", method = "GET", method = "HEAD")]
pub async fn docker_export_route(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !is_panel_admin(&user) {
        return redirect_notice(
            "/docker",
            None,
            Some("Only panel admins can export containers."),
        );
    }
    let name = query.get("name").map(String::as_str).unwrap_or("");
    if name.is_empty() {
        return redirect_notice(
            "/docker/list",
            None,
            Some("Missing container name for export."),
        );
    }
    if *http.method() == actix_web::http::Method::HEAD {
        let filename = format!("{name}.tar");
        return HttpResponse::Ok()
            .content_type("application/x-tar")
            .append_header((
                "Content-Disposition",
                format!("attachment; filename=\"{filename}\""),
            ))
            .finish();
    }
    let cname = name.to_string();
    let result = web::block(move || crate::panel_ops_docker_detail::container_export_tar(&cname))
        .await
        .unwrap_or_else(|e| Err(format!("Export task failed: {e}")));
    match result {
        Ok(bytes) => {
            let filename = format!("{name}.tar");
            HttpResponse::Ok()
                .content_type("application/x-tar")
                .append_header((
                    "Content-Disposition",
                    format!("attachment; filename=\"{filename}\""),
                ))
                .body(bytes)
        }
        Err(err) => redirect_notice(
            &format!("/docker/view/{}", urlencoding_simple(name)),
            None,
            Some(&err),
        ),
    }
}

#[derive(serde::Deserialize)]
pub struct DockerImagePullForm {
    pub image: String,
}

#[post("/docker/images/pull")]
pub async fn docker_image_pull(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<DockerImagePullForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !is_panel_admin(&user) {
        return redirect_notice(
            "/docker/images",
            None,
            Some("Only panel admins can pull images."),
        );
    }
    let image = form.image.clone();
    let result = web::block(move || crate::panel_ops_docker_images::pull_image(&image))
        .await
        .unwrap_or_else(|e| Err(format!("Pull task failed: {e}")));
    match result {
        Ok(msg) => redirect_notice("/docker/images", Some(&msg), None),
        Err(err) => redirect_notice("/docker/images", None, Some(&err)),
    }
}

#[derive(serde::Deserialize)]
pub struct DockerImageDeleteForm {
    pub repository: String,
    pub tag: String,
    pub id: String,
}

#[post("/docker/images/delete")]
pub async fn docker_image_delete(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<DockerImageDeleteForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !is_panel_admin(&user) {
        return redirect_notice(
            "/docker/images",
            None,
            Some("Only panel admins can delete images."),
        );
    }
    let repository = form.repository.clone();
    let tag = form.tag.clone();
    let id = form.id.clone();
    let result =
        web::block(move || crate::panel_ops_docker_images::delete_image(&repository, &tag, &id))
            .await
            .unwrap_or_else(|e| Err(format!("Delete task failed: {e}")));
    match result {
        Ok(msg) => redirect_notice("/docker/images", Some(&msg), None),
        Err(err) => redirect_notice("/docker/images", None, Some(&err)),
    }
}

#[post("/docker/images/prune")]
pub async fn docker_image_prune(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !is_panel_admin(&user) {
        return redirect_notice(
            "/docker/images",
            None,
            Some("Only panel admins can prune images."),
        );
    }
    let result = web::block(crate::panel_ops_docker_images::prune_unused_images)
        .await
        .unwrap_or_else(|e| Err(format!("Prune task failed: {e}")));
    match result {
        Ok(msg) => redirect_notice("/docker/images", Some(&msg), None),
        Err(err) => redirect_notice("/docker/images", None, Some(&err)),
    }
}

#[derive(serde::Deserialize)]
pub struct DockerCreateForm {
    pub image: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub ports: String,
    #[serde(default)]
    pub volumes: String,
    #[serde(default)]
    pub env: String,
    #[serde(default)]
    pub restart: String,
    #[serde(default)]
    pub start: Option<String>,
}

#[post("/docker/create")]
pub async fn docker_create_container(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<DockerCreateForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !is_panel_admin(&user) {
        return redirect_notice(
            "/docker",
            None,
            Some("Only panel admins can create containers."),
        );
    }
    let image = form.image.clone();
    let name = form.name.clone();
    let ports = form.ports.clone();
    let volumes = form.volumes.clone();
    let env = form.env.clone();
    let restart = form.restart.clone();
    let start = form
        .start
        .as_deref()
        .map(|s| s == "1" || s.eq_ignore_ascii_case("on") || s.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let owner_username = user.clone();
    let result = web::block(move || {
        crate::panel_ops_docker_images::create_container(
            crate::panel_ops_docker_images::CreateContainerRequest {
                image: &image,
                name: &name,
                ports: &ports,
                volumes: &volumes,
                env: &env,
                restart: &restart,
                start,
                owner: &owner_username,
            },
        )
    })
    .await
    .unwrap_or_else(|e| Err(format!("Create task failed: {e}")));
    match result {
        Ok(msg) => redirect_notice("/docker/list", Some(&msg), None),
        Err(err) => redirect_notice("/docker/create", None, Some(&err)),
    }
}

#[actix_web::route("/docker/stacks", method = "GET", method = "HEAD")]
pub async fn docker_stacks_route(
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
        "Compose Stacks",
        &docker_stacks_page(
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
            query.get("image").map(String::as_str),
            query.get("template").map(String::as_str),
        ),
    ))
}

#[derive(serde::Deserialize)]
pub struct DockerStackCreateForm {
    pub stack: String,
    pub image: String,
    #[serde(default)]
    pub template: String,
    #[serde(default)]
    pub ports: String,
    #[serde(default)]
    pub env: String,
    pub container_data_path: String,
}

#[post("/docker/stacks/create")]
pub async fn docker_stack_create(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<DockerStackCreateForm>,
) -> HttpResponse {
    let Some(username) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !is_panel_admin(&username) {
        return redirect_notice(
            "/docker/stacks",
            None,
            Some("Only panel admins can create compose stacks."),
        );
    }
    let stack = form.stack.clone();
    let image = form.image.clone();
    let template = form.template.clone();
    let ports = form.ports.clone();
    let env = form.env.clone();
    let container_data_path = form.container_data_path.clone();
    let owner = username.clone();
    let tpl = crate::panel_ops_docker_compose::OfficialStackTemplate::from_form(&template);
    let result = web::block(move || {
        crate::panel_ops_docker_compose::create_compose_stack(
            crate::panel_ops_docker_compose::CreateComposeStackRequest {
                stack_id: &stack,
                image: &image,
                ports: &ports,
                env: &env,
                container_data_path: &container_data_path,
                template: tpl,
                owner: &owner,
            },
        )
    })
    .await
    .unwrap_or_else(|e| Err(format!("Create stack task failed: {e}")));
    match result {
        Ok(msg) => redirect_notice("/docker/stacks", Some(&msg), None),
        Err(err) => redirect_notice("/docker/stacks", None, Some(&err)),
    }
}

#[derive(serde::Deserialize)]
pub struct DockerStackRefreshForm {
    pub stack: String,
}

#[post("/docker/stacks/refresh")]
pub async fn docker_stack_refresh(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<DockerStackRefreshForm>,
) -> HttpResponse {
    let Some(username) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !is_panel_admin(&username) {
        return redirect_notice(
            "/docker/stacks",
            None,
            Some("Only panel admins can refresh compose stacks."),
        );
    }
    let stack = form.stack.clone();
    let result = web::block(move || crate::panel_ops_docker_compose::refresh_compose_stack(&stack))
        .await
        .unwrap_or_else(|e| Err(format!("Refresh task failed: {e}")));
    match result {
        Ok(msg) => redirect_notice("/docker/stacks", Some(&msg), None),
        Err(err) => redirect_notice("/docker/stacks", None, Some(&err)),
    }
}

#[get("/settings")]
pub async fn settings_page(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let notice = query.get("notice").cloned();
    let error = query.get("error").cloned();
    html_blocking(move || {
        let owner = is_panel_admin(&user);
        panel_shell(
            &user,
            "settings",
            "Settings",
            &settings_hub_main_with(owner, notice.as_deref(), error.as_deref()),
        )
    })
    .await
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
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let tab = design_settings_tab(query.get("tab").map(String::as_str));
    let title = match tab {
        "store" => "Theme Store",
        "installed" => "Installed themes",
        _ => "Design",
    };
    html_ok(panel_shell(
        &user,
        "settings",
        title,
        &design_settings_page_with_tab(&user, tab),
    ))
}

#[get("/settings/setup")]
pub async fn settings_setup_page(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let notice = query.get("notice").map(String::as_str);
    let error = query.get("error").map(String::as_str);
    html_ok(panel_shell(
        &user,
        "settings",
        "Setup Wizard",
        &setup_wizard_page_with(notice, error),
    ))
}

#[derive(Debug, serde::Deserialize)]
pub struct SetupOnboardingForm {
    #[serde(default)]
    hostname: String,
    #[serde(default)]
    mail_mode: String,
    #[serde(default)]
    skip_rdns: String,
    #[serde(default)]
    external_imap_host: String,
    #[serde(default)]
    external_smtp_host: String,
}

#[post("/settings/setup")]
pub async fn settings_setup_save(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<SetupOnboardingForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !is_panel_admin(&user) {
        return redirect_notice(
            "/settings/setup",
            None,
            Some("Only the panel admin can change server onboarding."),
        );
    }
    use crate::panel_ops_mail_onboarding::{MailMode, MailOnboarding, save_mail_onboarding};
    let mail_mode = match form.mail_mode.trim().to_ascii_lowercase().as_str() {
        "external" => MailMode::External,
        _ => MailMode::Local,
    };
    let cfg = MailOnboarding {
        schema_version: 1,
        hostname: form.hostname.trim().to_string(),
        skip_rdns: matches!(
            form.skip_rdns.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "on" | "yes"
        ),
        mail_mode,
        external_imap_host: form.external_imap_host.trim().to_string(),
        external_smtp_host: form.external_smtp_host.trim().to_string(),
        updated_at_unix: 0,
    };
    match save_mail_onboarding(cfg) {
        Ok(()) => redirect_notice(
            "/settings/setup",
            Some("Server mail onboarding saved."),
            None,
        ),
        Err(e) => redirect_notice("/settings/setup", None, Some(&e)),
    }
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
        return owner_only_html(&user, "settings", "Site messages");
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

#[derive(Debug, serde::Deserialize)]
pub struct SiteReadyPreviewForm {
    #[serde(default)]
    html: String,
}

#[post("/settings/site-messages/preview-site-ready")]
pub async fn settings_site_messages_preview_site_ready(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<SiteReadyPreviewForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return HttpResponse::Unauthorized().finish();
    };
    if !is_panel_admin(&user) {
        return HttpResponse::Forbidden().finish();
    }
    match sanitize_site_ready_html(&form.html) {
        Ok(html) => HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(html),
        Err(error) => HttpResponse::BadRequest()
            .content_type("text/plain; charset=utf-8")
            .body(error),
    }
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
        return redirect_flash("/settings", None, Some("Admin only"));
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
        return redirect_flash("/settings", None, Some("Admin only"));
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
        return redirect_flash("/settings", None, Some("Admin only"));
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
        return redirect_flash("/settings", None, Some("Admin only"));
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
