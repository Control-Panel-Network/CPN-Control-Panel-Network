//! Authenticated Panel section routes and mutating actions.

use crate::auth_api::panel_user_from_request;
use crate::installer::AppState;
use crate::login_next::login_redirect;
use crate::packages::require_site_create_allowed;
use crate::panel_admin::is_panel_admin;
use crate::panel_hub_routes::{databases_hub_html, email_hub_html};
use crate::panel_pages::panel_shell;
use crate::panel_plugin_settings::{
    plugin_dashboard_main, plugin_settings_main, settings_from_form,
};
use crate::panel_plugins::{PluginsPageQuery, plugins_main};
use crate::panel_sections::{run_mariadb_install, set_websites_docroot_pref, websites_main};
use crate::panel_website_manage::website_manage_main;
use crate::plugin_activation::{
    activate_host_plugin_for_domain, deactivate_host_plugin_for_domain, install_host_plugin,
    is_host_owned_install, is_host_scoped_plugin, uninstall_host_plugin,
};
use crate::plugins::{install_plugin, set_plugin_enabled, uninstall_plugin};
use crate::plugins_settings::{
    declared_settings_fields, load_plugin_settings, save_plugin_settings,
};
use crate::site_acl::{SitePerm, require_manage_site, sites_manageable_by};
pub use crate::site_preview_thumb_routes::{site_preview_image, site_preview_refresh};
use crate::sites::{SiteModify, SuspendActor, create_site, delete_site, modify_site};
pub use crate::website_preview_routes::{
    preview_content, preview_mode_page, websites_pretty_manage, websites_preview_redirect,
};
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use std::sync::Arc;

pub use crate::panel_app_routes::{
    apps_activate, apps_deactivate, apps_install, apps_page, apps_reinstall, apps_start, apps_stop,
    apps_uninstall,
};
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

#[get("/websites")]
pub async fn websites_page(
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
        "websites",
        "Websites",
        &websites_main(&user, notice, error),
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
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let owner = if form.owner.trim().is_empty() {
        user.clone()
    } else if crate::packages::is_panel_admin(&user) {
        form.owner.trim().to_string()
    } else {
        // Non-admins may only create sites for themselves.
        user.clone()
    };
    let docroot = form.docroot.trim();
    if let Err(error) = require_site_create_allowed(&owner, &form.domain) {
        return HttpResponse::SeeOther()
            .append_header((
                "Location",
                format!("/websites?error={}", urlencoding_simple(&error)),
            ))
            .finish();
    }
    let result = create_site(
        &form.domain,
        &owner,
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
                    urlencoding_simple(&format!(
                        "Created {} at {}. Auto SSL and SPF/DKIM/DMARC were attempted.",
                        site.domain, site.docroot
                    ))
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
        return login_redirect(&http);
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
        return login_redirect(&http);
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
        return login_redirect(&http);
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
        return login_redirect(&http);
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
            suspended_by: Some(Some(if is_panel_admin(&user) {
                SuspendActor::Admin
            } else {
                SuspendActor::Owner
            })),
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
        return login_redirect(&http);
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
            suspended_by: Some(None),
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

#[derive(Debug, serde::Deserialize)]
pub struct SuspendMessageForm {
    domain: String,
    #[serde(default)]
    owner_suspend_message: String,
}

#[post("/websites/suspend-message")]
pub async fn websites_suspend_message(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<SuspendMessageForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
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
            owner_suspend_message: Some(form.owner_suspend_message.clone()),
            ..Default::default()
        },
    ) {
        Ok(site) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                format!(
                    "/websites/manage?domain={}&tab=config&notice={}",
                    urlencoding_simple(&site.domain),
                    urlencoding_simple("Suspend message saved")
                ),
            ))
            .finish(),
        Err(error) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                format!(
                    "/websites/manage?domain={}&tab=config&error={}",
                    urlencoding_simple(form.domain.trim()),
                    urlencoding_simple(&error)
                ),
            ))
            .finish(),
    }
}

#[post("/websites/suspend-message/restore")]
pub async fn websites_suspend_message_restore(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<SiteDeleteForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
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
            owner_suspend_message: Some(String::new()),
            ..Default::default()
        },
    ) {
        Ok(site) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                format!(
                    "/websites/manage?domain={}&tab=config&notice={}",
                    urlencoding_simple(&site.domain),
                    urlencoding_simple("Suspend message restored to panel default")
                ),
            ))
            .finish(),
        Err(error) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                format!(
                    "/websites/manage?domain={}&tab=config&error={}",
                    urlencoding_simple(form.domain.trim()),
                    urlencoding_simple(&error)
                ),
            ))
            .finish(),
    }
}

#[post("/websites/reset-placeholder")]
pub async fn websites_reset_placeholder(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<SiteDeleteForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let site = match require_manage_site(&user, &form.domain, SitePerm::Enable) {
        Ok(site) => site,
        Err(error) => {
            return HttpResponse::SeeOther()
                .append_header((
                    "Location",
                    format!("/websites?error={}", urlencoding_simple(&error)),
                ))
                .finish();
        }
    };
    match crate::site_messages::reset_placeholder_index(&site.docroot) {
        Ok(()) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                format!(
                    "/websites/manage?domain={}&tab=config&notice={}",
                    urlencoding_simple(&site.domain),
                    urlencoding_simple("Placeholder index.html reset")
                ),
            ))
            .finish(),
        Err(error) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                format!(
                    "/websites/manage?domain={}&tab=config&error={}",
                    urlencoding_simple(&site.domain),
                    urlencoding_simple(&error)
                ),
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
        return login_redirect(&http);
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
        return login_redirect(&http);
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
        return login_redirect(&http);
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

#[derive(Debug, serde::Deserialize)]
pub struct DatabaseCreateForm {
    #[serde(default)]
    name: String,
    #[serde(default)]
    owner: String,
    #[serde(default)]
    domain: String,
}

#[post("/databases/create")]
pub async fn databases_create(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<DatabaseCreateForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let owner = if form.owner.trim().is_empty() {
        user.clone()
    } else if crate::packages::is_panel_admin(&user) {
        form.owner.trim().to_string()
    } else {
        user.clone()
    };
    let result = crate::packages::require_quota(&owner, crate::packages::QuotaResource::Databases)
        .and_then(|_| crate::resource_accounts::create_database(&owner, &form.name, &form.domain));
    match result {
        Ok(db) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                format!(
                    "/databases?notice={}",
                    urlencoding_simple(&format!("Registered database {}", db.name))
                ),
            ))
            .finish(),
        Err(error) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                format!("/databases?error={}", urlencoding_simple(&error)),
            ))
            .finish(),
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct FtpCreateForm {
    #[serde(default)]
    username: String,
    #[serde(default)]
    owner: String,
    #[serde(default)]
    domain: String,
}

#[post("/databases/ftp-create")]
pub async fn databases_ftp_create(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<FtpCreateForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let owner = if form.owner.trim().is_empty() {
        user.clone()
    } else if crate::packages::is_panel_admin(&user) {
        form.owner.trim().to_string()
    } else {
        user.clone()
    };
    let result =
        crate::packages::require_quota(&owner, crate::packages::QuotaResource::FtpAccounts)
            .and_then(|_| {
                crate::resource_accounts::create_ftp_account(&owner, &form.username, &form.domain)
            });
    match result {
        Ok(ftp) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                format!(
                    "/databases?notice={}",
                    urlencoding_simple(&format!("Registered FTP account {}", ftp.username))
                ),
            ))
            .finish(),
        Err(error) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                format!("/databases?error={}", urlencoding_simple(&error)),
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
        return login_redirect(&http);
    };
    // Legacy Host packages tab: send bookmarks to unified Store Host category.
    if matches!(
        query.get("view").map(String::as_str),
        Some("host") | Some("apps")
    ) {
        let mut loc = String::from("/plugins?view=store&category=Host");
        for (key, value) in query.iter() {
            if key == "view" || key == "category" {
                continue;
            }
            if value.trim().is_empty() {
                continue;
            }
            loc.push('&');
            loc.push_str(key);
            loc.push('=');
            loc.push_str(&urlencoding_simple(value));
        }
        return HttpResponse::MovedPermanently()
            .append_header(("Location", loc))
            .finish();
    }
    let view = if query.contains_key("view-store")
        || query.get("view").map(String::as_str) == Some("store")
        || query.get("view").map(String::as_str) == Some("view-store")
    {
        "store".to_string()
    } else {
        query
            .get("view")
            .cloned()
            .unwrap_or_else(|| "installed".to_string())
    };
    let view = view.as_str();
    let layout = query.get("layout").map(String::as_str).unwrap_or("grid");
    let q = query.get("q").map(String::as_str).unwrap_or("");
    let category = query.get("category").map(String::as_str).unwrap_or("");
    let status = query.get("status").map(String::as_str).unwrap_or("");
    let domain = query.get("domain").map(String::as_str).unwrap_or("");
    let notice = query.get("notice").map(String::as_str);
    let error = query.get("error").map(String::as_str);
    let refresh = query.get("refresh").map(String::as_str) == Some("1");
    let mode = query.get("mode").map(String::as_str).unwrap_or("page");
    let page = crate::panel_plugins_spa::page_from_query(
        query.get("page").map(String::as_str).unwrap_or("1"),
    );
    let per_page = crate::panel_plugins_spa::per_page_from_query(
        query.get("per_page").map(String::as_str).unwrap_or("4"),
    );
    let partial = query.get("partial").map(String::as_str) == Some("1")
        || http
            .headers()
            .get("X-CPN-Partial")
            .and_then(|v| v.to_str().ok())
            == Some("1");
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
    let body = plugins_main(PluginsPageQuery {
        view,
        layout,
        q,
        category,
        status,
        domain,
        notice,
        error,
        refresh,
        mode,
        page,
        per_page,
        sites: &sites,
        username: &user,
    });
    if partial {
        return html_ok(body);
    }
    html_ok(panel_shell(&user, "plugins", "Plugins", &body))
}

#[derive(Debug, serde::Deserialize)]
pub struct PluginIdForm {
    #[serde(default)]
    id: String,
    #[serde(default)]
    domain: String,
    /// Must be `1` from the uninstall confirm dialog (ignored by other actions).
    #[serde(default)]
    confirm: String,
    /// When `installed`, prefer Installed hub after host uninstall.
    #[serde(default)]
    return_view: String,
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
        return login_redirect(&http);
    };
    // Host-scoped: never full-copy under a site when host install exists (or is required).
    if is_host_scoped_plugin(&form.id) {
        if crate::plugin_activation::host_plugin_installed(&form.id) {
            if let Err(error) = require_manage_site(&user, &form.domain, SitePerm::Enable) {
                return HttpResponse::SeeOther()
                    .append_header((
                        "Location",
                        plugins_redirect(&form.domain, "store", None, Some(&error)),
                    ))
                    .finish();
            }
            return match activate_host_plugin_for_domain(&form.domain, &form.id) {
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
                        plugins_redirect(&form.domain, "store", None, Some(&error)),
                    ))
                    .finish(),
            };
        }
        if !is_panel_admin(&user) {
            return HttpResponse::SeeOther()
                .append_header((
                    "Location",
                    plugins_redirect(
                        &form.domain,
                        "store",
                        None,
                        Some(
                            "This is a Host package. Ask the panel admin to Install on Host first.",
                        ),
                    ),
                ))
                .finish();
        }
        return match install_host_plugin(&form.id) {
            Ok(manifest) => {
                let notice = if form.domain.trim().is_empty() {
                    format!("Installed {} on Host", manifest.name)
                } else {
                    match activate_host_plugin_for_domain(&form.domain, &form.id) {
                        Ok(_) => format!(
                            "Installed {} on Host and activated for {}",
                            manifest.name,
                            form.domain.trim()
                        ),
                        Err(err) => format!(
                            "Installed {} on Host, but site activate failed: {}",
                            manifest.name, err
                        ),
                    }
                };
                HttpResponse::SeeOther()
                    .append_header((
                        "Location",
                        plugins_redirect(&form.domain, "store", Some(&notice), None),
                    ))
                    .finish()
            }
            Err(error) => HttpResponse::SeeOther()
                .append_header((
                    "Location",
                    plugins_redirect(&form.domain, "store", None, Some(&error)),
                ))
                .finish(),
        };
    }
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

#[post("/plugins/install-host")]
pub async fn plugins_install_host(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<PluginIdForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !is_panel_admin(&user) {
        return HttpResponse::SeeOther()
            .append_header((
                "Location",
                plugins_redirect(
                    &form.domain,
                    "store",
                    None,
                    Some("Only the panel admin can install Host plugins"),
                ),
            ))
            .finish();
    }
    match install_host_plugin(&form.id) {
        Ok(manifest) => {
            let notice = if form.domain.trim().is_empty() {
                format!("Installed {} on Host", manifest.name)
            } else if require_manage_site(&user, &form.domain, SitePerm::Enable).is_ok() {
                match activate_host_plugin_for_domain(&form.domain, &form.id) {
                    Ok(_) => format!(
                        "Installed {} on Host and activated for {}",
                        manifest.name,
                        form.domain.trim()
                    ),
                    Err(err) => format!(
                        "Installed {} on Host, but site activate failed: {}",
                        manifest.name, err
                    ),
                }
            } else {
                format!("Installed {} on Host", manifest.name)
            };
            HttpResponse::SeeOther()
                .append_header((
                    "Location",
                    plugins_redirect(&form.domain, "store", Some(&notice), None),
                ))
                .finish()
        }
        Err(error) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                plugins_redirect(&form.domain, "store", None, Some(&error)),
            ))
            .finish(),
    }
}

#[post("/plugins/activate-host")]
pub async fn plugins_activate_host(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<PluginIdForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Err(error) = require_manage_site(&user, &form.domain, SitePerm::Enable) {
        return HttpResponse::SeeOther()
            .append_header((
                "Location",
                plugins_redirect(&form.domain, "store", None, Some(&error)),
            ))
            .finish();
    }
    match activate_host_plugin_for_domain(&form.domain, &form.id) {
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
                plugins_redirect(&form.domain, "store", None, Some(&error)),
            ))
            .finish(),
    }
}

#[post("/plugins/deactivate-host")]
pub async fn plugins_deactivate_host(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<PluginIdForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Err(error) = require_manage_site(&user, &form.domain, SitePerm::Enable) {
        return HttpResponse::SeeOther()
            .append_header((
                "Location",
                plugins_redirect(&form.domain, "installed", None, Some(&error)),
            ))
            .finish();
    }
    match deactivate_host_plugin_for_domain(&form.domain, &form.id) {
        Ok(()) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                plugins_redirect(
                    &form.domain,
                    "installed",
                    Some(&format!("Deactivated {}", form.id.trim())),
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

#[post("/plugins/uninstall-host")]
pub async fn plugins_uninstall_host(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<PluginIdForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let view = if form.return_view.trim().eq_ignore_ascii_case("installed") {
        "installed"
    } else {
        "store"
    };
    if !is_panel_admin(&user) {
        return HttpResponse::SeeOther()
            .append_header((
                "Location",
                plugins_redirect(
                    &form.domain,
                    view,
                    None,
                    Some("Only the panel admin can uninstall Host plugins"),
                ),
            ))
            .finish();
    }
    if !crate::uninstall_confirm::confirm_accepted(&form.confirm) {
        return HttpResponse::SeeOther()
            .append_header((
                "Location",
                plugins_redirect(
                    &form.domain,
                    view,
                    None,
                    Some(crate::uninstall_confirm::CONFIRM_REQUIRED_MSG),
                ),
            ))
            .finish();
    }
    match uninstall_host_plugin(&form.id) {
        Ok(()) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                plugins_redirect(
                    &form.domain,
                    view,
                    Some(&format!("Uninstalled host plugin {}", form.id.trim())),
                    None,
                ),
            ))
            .finish(),
        Err(error) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                plugins_redirect(&form.domain, view, None, Some(&error)),
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
        return login_redirect(&http);
    };
    if is_host_owned_install(&form.domain, &form.id) && !is_panel_admin(&user) {
        return HttpResponse::SeeOther()
            .append_header((
                "Location",
                plugins_redirect(
                    &form.domain,
                    "installed",
                    None,
                    Some(
                        "This plugin is installed on the Host. Use Deactivate, or ask the admin to uninstall from Host.",
                    ),
                ),
            ))
            .finish();
    }
    if !crate::uninstall_confirm::confirm_accepted(&form.confirm) {
        return HttpResponse::SeeOther()
            .append_header((
                "Location",
                plugins_redirect(
                    &form.domain,
                    "installed",
                    None,
                    Some(crate::uninstall_confirm::CONFIRM_REQUIRED_MSG),
                ),
            ))
            .finish();
    }
    if is_host_owned_install(&form.domain, &form.id) && is_panel_admin(&user) {
        return match uninstall_host_plugin(&form.id) {
            Ok(()) => HttpResponse::SeeOther()
                .append_header((
                    "Location",
                    plugins_redirect(
                        &form.domain,
                        "store",
                        Some(&format!("Uninstalled host plugin {}", form.id.trim())),
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
        };
    }
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
        return login_redirect(&http);
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
        return login_redirect(&http);
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
        return login_redirect(&http);
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
        return login_redirect(&http);
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
    if crate::plugins_settings::is_webmail_plugin_id(id) {
        let mut cfg = crate::panel_webmail::load_webmail_config();
        if let Some(account) = settings.fields.get("auto_login_account") {
            cfg.auto_login_account = account.clone();
        }
        if let Some(path) = settings.fields.get("public_path")
            && !path.trim().is_empty()
        {
            cfg.public_path = path.clone();
        }
        let embed = settings
            .fields
            .get("internal_webmail")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true") || v == "on")
            .unwrap_or(false);
        cfg.internal_embed = embed;
        let _ = crate::panel_webmail::save_webmail_config(&cfg);
    }
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
        return login_redirect(&http);
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
