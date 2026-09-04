//! Authenticated Panel section routes and mutating actions.

use crate::auth_api::panel_user_from_request;
use crate::installer::AppState;
use crate::panel_hub_routes::{databases_hub_html, email_hub_html};
use crate::panel_pages::panel_shell;
use crate::panel_plugin_settings::{
    plugin_dashboard_main, plugin_settings_main, settings_from_form,
};
use crate::panel_plugins::{PluginsPageQuery, plugins_main};
use crate::panel_sections::{run_mariadb_install, set_websites_docroot_pref, websites_main};
use crate::panel_website_manage::website_manage_main;
use crate::plugins::{install_plugin, set_plugin_enabled, uninstall_plugin};
use crate::plugins_settings::{
    declared_settings_fields, load_plugin_settings, save_plugin_settings,
};
use crate::site_acl::{SitePerm, require_manage_site, sites_manageable_by};
use crate::sites::{SiteModify, create_site, delete_site, modify_site};
pub use crate::website_preview_routes::{
    preview_content, preview_mode_page, websites_pretty_manage, websites_preview_redirect,
};
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use std::sync::Arc;

pub use crate::panel_app_routes::{apps_install, apps_page, apps_reinstall, apps_uninstall};
pub use crate::panel_backup_routes::{backups_page, backups_run};
pub use crate::panel_mail_routes::{
    email_account_create, email_account_disable, email_account_enable,
};

fn require_panel_user(state: &AppState, http: &HttpRequest) -> Option<String> {
    panel_user_from_request(state, http)
}

fn html_ok(body: String) -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(body)
}

fn login_redirect() -> HttpResponse {
    HttpResponse::SeeOther()
        .append_header(("Location", "/login"))
        .finish()
}

#[get("/websites")]
pub async fn websites_page(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    let notice = query.get("notice").map(String::as_str);
    let error = query.get("error").map(String::as_str);
    html_ok(panel_shell(
        &user,
        "websites",
        "Websites",
        &websites_main(notice, error),
    ))
}

#[derive(Debug, serde::Deserialize)]
pub struct SiteCreateForm {
    #[serde(default)]
    domain: String,
    #[serde(default)]
    owner: String,
    #[serde(default)]
    docroot: String,
}

#[post("/websites/create")]
pub async fn websites_create(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<SiteCreateForm>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    let docroot = form.docroot.trim();
    let result = create_site(
        &form.domain,
        &form.owner,
        if docroot.is_empty() {
            None
        } else {
            Some(docroot)
        },
        None,
        None,
    );
    match result {
        Ok(site) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                format!(
                    "/websites?notice={}",
                    urlencoding_simple(&format!("Created {} at {}", site.domain, site.docroot))
                ),
            ))
            .finish(),
        Err(error) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                format!("/websites?error={}", urlencoding_simple(&error)),
            ))
            .finish(),
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct SiteDeleteForm {
    #[serde(default)]
    domain: String,
}

#[post("/websites/delete")]
pub async fn websites_delete(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<SiteDeleteForm>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    match delete_site(&form.domain) {
        Ok(()) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                format!(
                    "/websites?notice={}",
                    urlencoding_simple(&format!("Deleted {}", form.domain.trim()))
                ),
            ))
            .finish(),
        Err(error) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                format!("/websites?error={}", urlencoding_simple(&error)),
            ))
            .finish(),
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct WebsitePrefsForm {
    #[serde(default)]
    show_document_roots: String,
}

#[post("/websites/prefs")]
pub async fn websites_prefs(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<WebsitePrefsForm>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    let show = matches!(form.show_document_roots.trim(), "1" | "true" | "on" | "yes");
    match set_websites_docroot_pref(show) {
        Ok(()) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                format!(
                    "/websites?notice={}",
                    urlencoding_simple(if show {
                        "Document roots visible"
                    } else {
                        "Document roots hidden"
                    })
                ),
            ))
            .finish(),
        Err(error) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                format!("/websites?error={}", urlencoding_simple(&error)),
            ))
            .finish(),
    }
}

#[get("/websites/manage")]
pub async fn websites_manage(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    let domain = query.get("domain").map(String::as_str).unwrap_or("");
    let tab = query.get("tab").map(String::as_str);
    let notice = query.get("notice").map(String::as_str);
    let error = query.get("error").map(String::as_str);
    match require_manage_site(&user, domain, SitePerm::Enable) {
        Ok(site) => html_ok(panel_shell(
            &user,
            "websites",
            &format!("Manage {}", site.domain),
            &website_manage_main(&site, &user, tab, notice, error),
        )),
        Err(err) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                format!("/websites?error={}", urlencoding_simple(&err)),
            ))
            .finish(),
    }
}

#[post("/websites/suspend")]
pub async fn websites_suspend(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<SiteDeleteForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    if let Err(error) = require_manage_site(&user, &form.domain, SitePerm::Enable) {
        return HttpResponse::SeeOther()
            .append_header((
                "Location",
                format!("/websites?error={}", urlencoding_simple(&error)),
            ))
            .finish();
    }
    match modify_site(
        &form.domain,
        SiteModify {
            enabled: Some(false),
            ..Default::default()
        },
    ) {
        Ok(site) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                format!(
                    "/websites/manage?domain={}&notice={}",
                    urlencoding_simple(&site.domain),
                    urlencoding_simple("Site suspended")
                ),
            ))
            .finish(),
        Err(error) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                format!("/websites?error={}", urlencoding_simple(&error)),
            ))
            .finish(),
    }
}

#[post("/websites/resume")]
pub async fn websites_resume(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<SiteDeleteForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    if let Err(error) = require_manage_site(&user, &form.domain, SitePerm::Enable) {
        return HttpResponse::SeeOther()
            .append_header((
                "Location",
                format!("/websites?error={}", urlencoding_simple(&error)),
            ))
            .finish();
    }
    match modify_site(
        &form.domain,
        SiteModify {
            enabled: Some(true),
            ..Default::default()
        },
    ) {
        Ok(site) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                format!(
                    "/websites/manage?domain={}&notice={}",
                    urlencoding_simple(&site.domain),
                    urlencoding_simple("Site resumed")
                ),
            ))
            .finish(),
        Err(error) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                format!("/websites?error={}", urlencoding_simple(&error)),
            ))
            .finish(),
    }
}

#[get("/email")]
pub async fn email_page(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    let status = state
        .status
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    let notice = query.get("notice").map(String::as_str);
    let error = query.get("error").map(String::as_str);
    let _ = (status, notice, error);
    html_ok(panel_shell(&user, "email", "Email", &email_hub_html()))
}

#[get("/databases")]
pub async fn databases_page(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    let notice = query.get("notice").map(String::as_str);
    let error = query.get("error").map(String::as_str);
    let _ = (notice, error);
    html_ok(panel_shell(
        &user,
        "databases",
        "Databases & FTP",
        &databases_hub_html(),
    ))
}

#[post("/databases/install-mariadb")]
pub async fn databases_install_mariadb(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    match run_mariadb_install() {
        Ok(message) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                format!("/databases/manager?notice={}", urlencoding_simple(&message)),
            ))
            .finish(),
        Err(error) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                format!("/databases/manager?error={}", urlencoding_simple(&error)),
            ))
            .finish(),
    }
}

#[get("/plugins")]
pub async fn plugins_page(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    let view = query.get("view").map(String::as_str).unwrap_or("installed");
    let layout = query.get("layout").map(String::as_str).unwrap_or("grid");
    let q = query.get("q").map(String::as_str).unwrap_or("");
    let category = query.get("category").map(String::as_str).unwrap_or("");
    let domain = query.get("domain").map(String::as_str).unwrap_or("");
    let notice = query.get("notice").map(String::as_str);
    let error = query.get("error").map(String::as_str);
    let refresh = query.get("refresh").map(String::as_str) == Some("1");
    let sites = sites_manageable_by(&user).unwrap_or_default();
    let domain = if domain.trim().is_empty() {
        ""
    } else if sites
        .iter()
        .any(|s| s.domain.eq_ignore_ascii_case(domain.trim()))
    {
        domain.trim()
    } else {
        ""
    };
    html_ok(panel_shell(
        &user,
        "plugins",
        "Plugins",
        &plugins_main(PluginsPageQuery {
            view,
            layout,
            q,
            category,
            domain,
            notice,
            error,
            refresh,
            sites: &sites,
        }),
    ))
}

#[derive(Debug, serde::Deserialize)]
pub struct PluginIdForm {
    #[serde(default)]
    id: String,
    #[serde(default)]
    domain: String,
}

fn plugins_redirect(domain: &str, view: &str, notice: Option<&str>, error: Option<&str>) -> String {
    let mut url = format!(
        "/plugins?view={}&domain={}",
        urlencoding_simple(view),
        urlencoding_simple(domain)
    );
    if let Some(notice) = notice {
        url.push_str(&format!("&notice={}", urlencoding_simple(notice)));
    }
    if let Some(error) = error {
        url.push_str(&format!("&error={}", urlencoding_simple(error)));
    }
    url
}

#[post("/plugins/install")]
pub async fn plugins_install(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<PluginIdForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    if let Err(error) = require_manage_site(&user, &form.domain, SitePerm::Install) {
        return HttpResponse::SeeOther()
            .append_header((
                "Location",
                plugins_redirect(&form.domain, "store", None, Some(&error)),
            ))
            .finish();
    }
    match install_plugin(&form.domain, &form.id) {
        Ok(manifest) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                plugins_redirect(
                    &form.domain,
                    "installed",
                    Some(&format!("Installed {}", manifest.name)),
                    None,
                ),
            ))
            .finish(),
        Err(error) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                plugins_redirect(&form.domain, "store", None, Some(&error)),
            ))
            .finish(),
    }
}

#[post("/plugins/uninstall")]
pub async fn plugins_uninstall(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<PluginIdForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    if let Err(error) = require_manage_site(&user, &form.domain, SitePerm::Uninstall) {
        return HttpResponse::SeeOther()
            .append_header((
                "Location",
                plugins_redirect(&form.domain, "installed", None, Some(&error)),
            ))
            .finish();
    }
    match uninstall_plugin(&form.domain, &form.id) {
        Ok(()) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                plugins_redirect(
                    &form.domain,
                    "installed",
                    Some(&format!("Uninstalled {}", form.id.trim())),
                    None,
                ),
            ))
            .finish(),
        Err(error) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                plugins_redirect(&form.domain, "installed", None, Some(&error)),
            ))
            .finish(),
    }
}

#[post("/plugins/enable")]
pub async fn plugins_enable(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<PluginIdForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    if let Err(error) = require_manage_site(&user, &form.domain, SitePerm::Enable) {
        return HttpResponse::SeeOther()
            .append_header((
                "Location",
                plugins_redirect(&form.domain, "installed", None, Some(&error)),
            ))
            .finish();
    }
    match set_plugin_enabled(&form.domain, &form.id, true) {
        Ok(manifest) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                plugins_redirect(
                    &form.domain,
                    "installed",
                    Some(&format!("Activated {}", manifest.name)),
                    None,
                ),
            ))
            .finish(),
        Err(error) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                plugins_redirect(&form.domain, "installed", None, Some(&error)),
            ))
            .finish(),
    }
}

#[post("/plugins/disable")]
pub async fn plugins_disable(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<PluginIdForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    if let Err(error) = require_manage_site(&user, &form.domain, SitePerm::Enable) {
        return HttpResponse::SeeOther()
            .append_header((
                "Location",
                plugins_redirect(&form.domain, "installed", None, Some(&error)),
            ))
            .finish();
    }
    match set_plugin_enabled(&form.domain, &form.id, false) {
        Ok(manifest) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                plugins_redirect(
                    &form.domain,
                    "installed",
                    Some(&format!("Deactivated {}", manifest.name)),
                    None,
                ),
            ))
            .finish(),
        Err(error) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                plugins_redirect(&form.domain, "installed", None, Some(&error)),
            ))
            .finish(),
    }
}

#[get("/plugins/settings")]
pub async fn plugins_settings_page(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    let domain = query.get("domain").map(String::as_str).unwrap_or("");
    let id = query.get("id").map(String::as_str).unwrap_or("");
    let notice = query.get("notice").map(String::as_str);
    let error = query.get("error").map(String::as_str);
    let sites = sites_manageable_by(&user).unwrap_or_default();
    if !domain.trim().is_empty()
        && let Err(err) = require_manage_site(&user, domain, SitePerm::Enable)
    {
        return HttpResponse::SeeOther()
            .append_header((
                "Location",
                plugins_redirect(domain, "installed", None, Some(&err)),
            ))
            .finish();
    }
    html_ok(panel_shell(
        &user,
        "plugins",
        "Plugin settings",
        &plugin_settings_main(&sites, domain, id, notice, error),
    ))
}

#[post("/plugins/settings")]
pub async fn plugins_settings_save(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    let domain = form.get("domain").map(String::as_str).unwrap_or("");
    let id = form.get("id").map(String::as_str).unwrap_or("");
    if let Err(error) = require_manage_site(&user, domain, SitePerm::Enable) {
        return HttpResponse::SeeOther()
            .append_header((
                "Location",
                format!(
                    "/plugins/settings?domain={}&id={}&error={}",
                    urlencoding_simple(domain),
                    urlencoding_simple(id),
                    urlencoding_simple(&error)
                ),
            ))
            .finish();
    }
    let previous = load_plugin_settings(domain, id).unwrap_or_default();
    let declared: Vec<String> = declared_settings_fields(domain, id)
        .into_iter()
        .map(|f| f.key)
        .collect();
    let settings = settings_from_form(&form, &previous, &declared);
    match save_plugin_settings(domain, id, &settings) {
        Ok(()) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                format!(
                    "/plugins/settings?domain={}&id={}&notice={}",
                    urlencoding_simple(domain),
                    urlencoding_simple(id),
                    urlencoding_simple("Settings saved")
                ),
            ))
            .finish(),
        Err(error) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                format!(
                    "/plugins/settings?domain={}&id={}&error={}",
                    urlencoding_simple(domain),
                    urlencoding_simple(id),
                    urlencoding_simple(&error)
                ),
            ))
            .finish(),
    }
}

#[get("/plugins/dashboard")]
pub async fn plugins_dashboard_page(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    let domain = query.get("domain").map(String::as_str).unwrap_or("");
    let id = query.get("id").map(String::as_str).unwrap_or("");
    let notice = query.get("notice").map(String::as_str);
    let error = query.get("error").map(String::as_str);
    if !domain.trim().is_empty()
        && let Err(err) = require_manage_site(&user, domain, SitePerm::Enable)
    {
        return HttpResponse::SeeOther()
            .append_header((
                "Location",
                plugins_redirect(domain, "installed", None, Some(&err)),
            ))
            .finish();
    }
    let active = if domain.is_empty() || id.is_empty() {
        "plugins".to_string()
    } else {
        format!("plugin-{domain}-{id}")
    };
    html_ok(panel_shell(
        &user,
        &active,
        "Plugin dashboard",
        &plugin_dashboard_main(domain, id, notice, error),
    ))
}

fn urlencoding_simple(value: &str) -> String {
    let mut out = String::with_capacity(value.len() * 3);
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}
