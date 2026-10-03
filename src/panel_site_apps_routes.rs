//! GET/POST routes for website manage Apps (`tab=apps`).

use crate::apps::{AppId, AppStateKind, detect_app, install_app};
use crate::apps_site::{apply_site_scope, clear_site_scope};
use crate::installer::AppState;
use crate::panel_admin::is_panel_admin;
use crate::panel_hub_http::{
    html_blocking, login_redirect, redirect_notice, require_panel_user, urlencoding_simple,
};
use crate::panel_pages::panel_shell;
use crate::panel_site_apps_cmsms::{install_cmsms, remove_cmsms_installer};
use crate::panel_site_apps_runtime::{
    RuntimeKind, ensure_python_venv, save_runtime, start_runtime, stop_runtime,
};
use crate::panel_site_tools_security::{
    check_tools_rate_limit, same_origin_ok, verify_site_tools_csrf,
};
use crate::panel_website_manage::website_manage_main;
use crate::site_acl::{SitePerm, require_manage_site};
use actix_web::{HttpRequest, HttpResponse, post, route, web};
use std::sync::Arc;

fn manage_apps_redirect(domain: &str, notice: Option<&str>, error: Option<&str>) -> HttpResponse {
    let base = format!(
        "/websites/manage?domain={}&tab=apps",
        urlencoding_simple(domain)
    );
    redirect_notice(&base, notice, error)
}

#[route("/websites/apps", method = "GET", method = "HEAD")]
pub async fn websites_apps_get(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let domain = query.get("domain").map(String::as_str).unwrap_or("");
    let notice = query.get("notice").cloned();
    let error = query.get("error").cloned();
    match require_manage_site(&user, domain, SitePerm::Enable) {
        Ok(site) => {
            let user_owned = user.clone();
            html_blocking(move || {
                panel_shell(
                    &user_owned,
                    "websites",
                    &format!("Manage {}", site.domain),
                    &website_manage_main(
                        &site,
                        &user_owned,
                        Some("apps"),
                        notice.as_deref(),
                        error.as_deref(),
                    ),
                )
            })
            .await
        }
        Err(err) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                format!("/websites?error={}", urlencoding_simple(&err)),
            ))
            .finish(),
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct SiteAppsForm {
    #[serde(default)]
    domain: String,
    #[serde(default)]
    csrf: String,
    #[serde(default)]
    action: String,
    #[serde(default)]
    version_bin: String,
    #[serde(default)]
    app_rel: String,
    #[serde(default)]
    entry: String,
    #[serde(default)]
    app: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    backup: String,
}

#[post("/websites/apps")]
pub async fn websites_apps_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<SiteAppsForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !same_origin_ok(&http) {
        return manage_apps_redirect(&form.domain, None, Some("Same-origin check failed"));
    }
    if !verify_site_tools_csrf(&user, &form.domain, &form.csrf) {
        return manage_apps_redirect(&form.domain, None, Some("Invalid CSRF token"));
    }
    if let Err(err) = check_tools_rate_limit(&user) {
        return manage_apps_redirect(&form.domain, None, Some(&err));
    }
    let site = match require_manage_site(&user, &form.domain, SitePerm::Enable) {
        Ok(s) => s,
        Err(err) => {
            return HttpResponse::SeeOther()
                .append_header((
                    "Location",
                    format!("/websites?error={}", urlencoding_simple(&err)),
                ))
                .finish();
        }
    };
    let action = form.action.trim().to_ascii_lowercase();
    let admin = is_panel_admin(&user);
    let domain = site.domain.clone();
    let version_bin = form.version_bin.clone();
    let app_rel = form.app_rel.clone();
    let entry = form.entry.clone();
    let app = form.app.clone();
    let version = form.version.clone();
    let backup = form.backup.clone();

    let result = tokio::task::spawn_blocking(move || match action.as_str() {
        "cmsms_install" => install_cmsms(&site),
        "cmsms_remove_installer" => remove_cmsms_installer(&site),
        "redis_activate" => {
            let st = detect_app(AppId::Redis);
            if st.state == AppStateKind::NotInstalled {
                return Err(
                    "Redis is not installed on the host. Install the host package first, then Activate."
                        .into(),
                );
            }
            apply_site_scope(AppId::Redis, &site.domain)
        }
        "redis_detach" => clear_site_scope(AppId::Redis, &site.domain),
        "redis_install_host" => {
            if !admin {
                return Err("Only the panel owner can install Redis on the host.".into());
            }
            let msg = install_app(AppId::Redis)?;
            let attach = apply_site_scope(AppId::Redis, &site.domain)?;
            Ok(format!("{msg} {attach}"))
        }
        "node_save" => save_runtime(&site, RuntimeKind::Node, &version_bin, &app_rel, &entry),
        "python_save" => save_runtime(&site, RuntimeKind::Python, &version_bin, &app_rel, &entry),
        "node_start" => start_runtime(&site, RuntimeKind::Node),
        "node_stop" => stop_runtime(&site, RuntimeKind::Node),
        "python_start" => start_runtime(&site, RuntimeKind::Python),
        "python_stop" => stop_runtime(&site, RuntimeKind::Python),
        "python_venv" => ensure_python_venv(&site),
        "app_install" => {
            let app = crate::site_app_lifecycle::SiteAppId::parse(&app)?;
            if matches!(
                app,
                crate::site_app_lifecycle::SiteAppId::Redis
                    | crate::site_app_lifecycle::SiteAppId::Node
                    | crate::site_app_lifecycle::SiteAppId::Python
            ) && !admin
            {
                return Err("Only the panel owner can install host runtimes.".into());
            }
            crate::site_app_lifecycle::install(&site, app, Some(&version))
        }
        "app_update" | "app_downgrade" => {
            let app = crate::site_app_lifecycle::SiteAppId::parse(&app)?;
            if matches!(
                app,
                crate::site_app_lifecycle::SiteAppId::Redis
                    | crate::site_app_lifecycle::SiteAppId::Node
                    | crate::site_app_lifecycle::SiteAppId::Python
            ) && !admin
            {
                return Err("Only the panel owner can change host runtime versions.".into());
            }
            if action == "app_downgrade" {
                crate::site_app_lifecycle::downgrade(&site, app, &version)
            } else {
                crate::site_app_lifecycle::update(&site, app, Some(&version))
            }
        }
        "app_restore" => {
            let app = crate::site_app_lifecycle::SiteAppId::parse(&app)?;
            if matches!(app, crate::site_app_lifecycle::SiteAppId::Redis) && !admin {
                return Err("Only the panel owner can restore a host runtime.".into());
            }
            crate::site_app_lifecycle::restore(&site, app, Some(&backup))
        }
        _ => Err("Unknown Apps action".into()),
    })
    .await;

    match result {
        Ok(Ok(msg)) => manage_apps_redirect(&domain, Some(&msg), None),
        Ok(Err(err)) => manage_apps_redirect(&domain, None, Some(&err)),
        Err(_) => manage_apps_redirect(&domain, None, Some("Apps action failed to run")),
    }
}
