//! Authenticated `/wordpress` panel routes.

use crate::installer::AppState;
use crate::panel_hub_http::{html_ok, login_redirect, redirect_notice, require_panel_user};
use crate::panel_pages::panel_shell;
use crate::panel_wordpress_ui::{
    wordpress_install_page, wordpress_list_page, wordpress_manage_page,
};
use crate::wordpress_install::{WordpressInstallRequest, install_wordpress, parse_plugin_sources};
use crate::wordpress_manage::{
    activate_theme, all_sites_snapshot, delete_wordpress, install_plugin, refresh_wordpress_site,
    scan_wordpress_sites, set_debugging, set_maintenance, set_password_protection,
    set_search_indexing,
};
use crate::wordpress_wpcli::{detect_wp_cli, ensure_wp_cli};
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use std::sync::Arc;

fn wp_redirect(base: &str, notice: Option<&str>, error: Option<&str>) -> HttpResponse {
    redirect_notice(base, notice, error)
}

fn parse_bool_flag(raw: &str) -> bool {
    matches!(
        raw.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

#[get("/wordpress")]
pub async fn wordpress_list_route(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let wp_cli = detect_wp_cli();
    html_ok(panel_shell(
        &user,
        "wordpress",
        "WordPress",
        &wordpress_list_page(
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
            &wp_cli,
        ),
    ))
}

#[get("/wordpress/install")]
pub async fn wordpress_install_get(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    html_ok(panel_shell(
        &user,
        "wordpress",
        "Install WordPress",
        &wordpress_install_page(
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}

#[derive(Debug, serde::Deserialize)]
pub struct WordpressInstallForm {
    #[serde(default)]
    domain: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    site_url: String,
    #[serde(default)]
    admin_user: String,
    #[serde(default)]
    admin_password: String,
    #[serde(default)]
    admin_email: String,
    #[serde(default)]
    plugin_sources: String,
    #[serde(default)]
    create_site_if_missing: String,
}

#[post("/wordpress/install")]
pub async fn wordpress_install_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<WordpressInstallForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let req = WordpressInstallRequest {
        domain: form.domain.clone(),
        owner: user.clone(),
        title: form.title.clone(),
        site_url: form.site_url.clone(),
        admin_user: form.admin_user.clone(),
        admin_password: form.admin_password.clone(),
        admin_email: form.admin_email.clone(),
        plugin_sources: parse_plugin_sources(&form.plugin_sources),
        create_site_if_missing: parse_bool_flag(&form.create_site_if_missing),
    };
    match install_wordpress(req) {
        Ok(result) => {
            let msg = format!(
                "WordPress installed on {}. {}",
                result.site.domain,
                result.notes.join(" ")
            );
            wp_redirect(
                &format!(
                    "/wordpress/manage?domain={}",
                    crate::panel_hub_http::urlencoding_simple(&result.site.domain)
                ),
                Some(&msg),
                None,
            )
        }
        Err(error) => wp_redirect("/wordpress/install", None, Some(&error)),
    }
}

#[get("/wordpress/manage")]
pub async fn wordpress_manage_route(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let domain = query.get("domain").map(String::as_str).unwrap_or("");
    if domain.trim().is_empty() {
        return wp_redirect("/wordpress", None, Some("Domain is required"));
    }
    let tab = query.get("tab").map(String::as_str).unwrap_or("general");
    let snapshots = match all_sites_snapshot() {
        Ok(list) => list,
        Err(error) => {
            return wp_redirect("/wordpress", None, Some(&error));
        }
    };
    let Some(snapshot) = snapshots
        .into_iter()
        .find(|s| s.site.domain.eq_ignore_ascii_case(domain))
    else {
        return wp_redirect(
            "/wordpress",
            None,
            Some(&format!("WordPress site `{domain}` not found")),
        );
    };
    html_ok(panel_shell(
        &user,
        "wordpress",
        "Manage WordPress",
        &wordpress_manage_page(
            &snapshot,
            tab,
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}

#[derive(Debug, serde::Deserialize)]
pub struct WordpressDomainForm {
    #[serde(default)]
    domain: String,
}

#[post("/wordpress/scan")]
pub async fn wordpress_scan_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    match scan_wordpress_sites() {
        Ok(results) => wp_redirect(
            "/wordpress",
            Some(&format!(
                "Scan complete. Refreshed {} site(s).",
                results.len()
            )),
            None,
        ),
        Err(error) => wp_redirect("/wordpress", None, Some(&error)),
    }
}

#[post("/wordpress/ensure-wpcli")]
pub async fn wordpress_ensure_wpcli_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    match ensure_wp_cli() {
        Ok(status) => wp_redirect("/wordpress", Some(&status.detail), None),
        Err(error) => wp_redirect("/wordpress", None, Some(&error)),
    }
}

#[post("/wordpress/refresh")]
pub async fn wordpress_refresh_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<WordpressDomainForm>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let domain = form.domain.trim();
    let base = format!(
        "/wordpress/manage?domain={}",
        crate::panel_hub_http::urlencoding_simple(domain)
    );
    match refresh_wordpress_site(domain) {
        Ok(result) => wp_redirect(
            &base,
            Some(&format!("Refreshed {}.", result.site.domain)),
            None,
        ),
        Err(error) => wp_redirect(&base, None, Some(&error)),
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct WordpressDeleteForm {
    #[serde(default)]
    domain: String,
    #[serde(default)]
    remove_files: String,
}

#[post("/wordpress/delete")]
pub async fn wordpress_delete_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<WordpressDeleteForm>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let remove_files = parse_bool_flag(&form.remove_files);
    match delete_wordpress(&form.domain, remove_files) {
        Ok(msg) => wp_redirect("/wordpress", Some(&msg), None),
        Err(error) => wp_redirect(
            &format!(
                "/wordpress/manage?domain={}",
                crate::panel_hub_http::urlencoding_simple(form.domain.trim())
            ),
            None,
            Some(&error),
        ),
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct WordpressPluginForm {
    #[serde(default)]
    domain: String,
    #[serde(default)]
    source: String,
}

#[post("/wordpress/plugins/install")]
pub async fn wordpress_plugin_install_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<WordpressPluginForm>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let base = format!(
        "/wordpress/manage?domain={}&tab=plugins",
        crate::panel_hub_http::urlencoding_simple(form.domain.trim())
    );
    match install_plugin(&form.domain, &form.source) {
        Ok(msg) => wp_redirect(&base, Some(&msg), None),
        Err(error) => wp_redirect(&base, None, Some(&error)),
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct WordpressThemeForm {
    #[serde(default)]
    domain: String,
    #[serde(default)]
    slug: String,
}

#[post("/wordpress/themes/activate")]
pub async fn wordpress_theme_activate_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<WordpressThemeForm>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let base = format!(
        "/wordpress/manage?domain={}&tab=themes",
        crate::panel_hub_http::urlencoding_simple(form.domain.trim())
    );
    match activate_theme(&form.domain, &form.slug) {
        Ok(msg) => wp_redirect(&base, Some(&msg), None),
        Err(error) => wp_redirect(&base, None, Some(&error)),
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct WordpressToggleForm {
    #[serde(default)]
    domain: String,
    #[serde(default)]
    enabled: String,
    #[serde(default)]
    username: String,
    #[serde(default)]
    password: String,
}

#[post("/wordpress/toggle/search-indexing")]
pub async fn wordpress_toggle_search_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<WordpressToggleForm>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let enabled = parse_bool_flag(&form.enabled);
    let base = format!(
        "/wordpress/manage?domain={}",
        crate::panel_hub_http::urlencoding_simple(form.domain.trim())
    );
    match set_search_indexing(&form.domain, enabled) {
        Ok(msg) => wp_redirect(&base, Some(&msg), None),
        Err(error) => wp_redirect(&base, None, Some(&error)),
    }
}

#[post("/wordpress/toggle/debugging")]
pub async fn wordpress_toggle_debug_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<WordpressToggleForm>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let enabled = parse_bool_flag(&form.enabled);
    let base = format!(
        "/wordpress/manage?domain={}",
        crate::panel_hub_http::urlencoding_simple(form.domain.trim())
    );
    match set_debugging(&form.domain, enabled) {
        Ok(msg) => wp_redirect(&base, Some(&msg), None),
        Err(error) => wp_redirect(&base, None, Some(&error)),
    }
}

#[post("/wordpress/toggle/maintenance")]
pub async fn wordpress_toggle_maintenance_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<WordpressToggleForm>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let enabled = parse_bool_flag(&form.enabled);
    let base = format!(
        "/wordpress/manage?domain={}",
        crate::panel_hub_http::urlencoding_simple(form.domain.trim())
    );
    match set_maintenance(&form.domain, enabled) {
        Ok(msg) => wp_redirect(&base, Some(&msg), None),
        Err(error) => wp_redirect(&base, None, Some(&error)),
    }
}

#[post("/wordpress/toggle/password-protection")]
pub async fn wordpress_toggle_password_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<WordpressToggleForm>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let enabled = parse_bool_flag(&form.enabled);
    let base = format!(
        "/wordpress/manage?domain={}",
        crate::panel_hub_http::urlencoding_simple(form.domain.trim())
    );
    match set_password_protection(&form.domain, enabled, &form.username, &form.password) {
        Ok(msg) => wp_redirect(&base, Some(&msg), None),
        Err(error) => wp_redirect(&base, None, Some(&error)),
    }
}
