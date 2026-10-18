//! Owner routes for `/server/panel-migrate`.

use crate::foreign_panel::apply::ApplyOptions;
use crate::foreign_panel::{apply_panel, host_root, wipe_panel};
use crate::installer::AppState;
use crate::panel_admin::is_panel_admin;
use crate::panel_csrf::require_panel_csrf;
use crate::panel_hub_http::{
    html_ok, login_redirect, owner_only_html, redirect_notice, require_panel_user,
};
use crate::panel_hub_pages_migrate::panel_migrate_page;
use crate::panel_pages::panel_shell;
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use serde::Deserialize;
use std::sync::Arc;
use std::time::Duration;

const MIGRATE_BUDGET: Duration = Duration::from_secs(3600);

#[derive(Debug, Deserialize)]
pub struct MigrateForm {
    pub id: String,
    pub confirm: Option<String>,
    pub port_ok: Option<String>,
    pub csrf: Option<String>,
}

fn gate(http: &HttpRequest, state: &AppState) -> Result<String, HttpResponse> {
    let Some(user) = require_panel_user(state, http) else {
        return Err(login_redirect(http));
    };
    if !is_panel_admin(&user) {
        return Err(owner_only_html(&user, "server", "Migrate another panel"));
    }
    Ok(user)
}

#[get("/server/panel-migrate")]
pub async fn panel_migrate_route(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let user = match gate(&http, &state) {
        Ok(user) => user,
        Err(response) => return response,
    };
    let notice = query.get("notice").cloned();
    let error = query.get("error").cloned();
    html_ok(panel_shell(
        &user,
        "server",
        "Migrate another panel",
        &panel_migrate_page(notice.as_deref(), error.as_deref()),
    ))
}

#[post("/server/panel-migrate/apply")]
pub async fn panel_migrate_apply(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<MigrateForm>,
) -> HttpResponse {
    let user = match gate(&http, &state) {
        Ok(user) => user,
        Err(response) => return response,
    };
    let token = form.csrf.clone().unwrap_or_default();
    if let Err(error) = require_panel_csrf(&http, &user, &token) {
        return redirect_notice("/server/panel-migrate", None, Some(&error));
    }
    if form.confirm.as_deref().unwrap_or("").trim() != "MIGRATE" {
        return redirect_notice(
            "/server/panel-migrate",
            None,
            Some("Type MIGRATE to copy this panel into CPN."),
        );
    }
    let id = form.id.trim().to_string();
    let port_ok = form.port_ok.as_deref() == Some("1");
    let work = tokio::task::spawn_blocking(move || {
        apply_panel(
            &host_root(),
            &id,
            &ApplyOptions {
                port_ok,
                register_sites: true,
            },
        )
    });
    match tokio::time::timeout(MIGRATE_BUDGET, work).await {
        Ok(Ok(Ok(report))) => {
            let notice = format!(
                "Copied {} site(s) from {}. Mail paths {}, DNS zones {}, users listed {}.",
                report.sites.len(),
                report.panel_name,
                report.mail_copied.len(),
                report.zones_copied.len(),
                report.users.len()
            );
            let error = if report.verified || report.warnings.is_empty() {
                None
            } else {
                Some(truncate(&report.warnings.join(" ")))
            };
            redirect_notice("/server/panel-migrate", Some(&notice), error.as_deref())
        }
        Ok(Ok(Err(error))) => {
            redirect_notice("/server/panel-migrate", None, Some(&truncate(&error)))
        }
        _ => redirect_notice(
            "/server/panel-migrate",
            None,
            Some("Copy did not finish. Run cpn panel-migrate apply on the server."),
        ),
    }
}

#[post("/server/panel-migrate/wipe")]
pub async fn panel_migrate_wipe(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<MigrateForm>,
) -> HttpResponse {
    let user = match gate(&http, &state) {
        Ok(user) => user,
        Err(response) => return response,
    };
    let token = form.csrf.clone().unwrap_or_default();
    if let Err(error) = require_panel_csrf(&http, &user, &token) {
        return redirect_notice("/server/panel-migrate", None, Some(&error));
    }
    if form.confirm.as_deref().unwrap_or("").trim() != "REMOVE" {
        return redirect_notice(
            "/server/panel-migrate",
            None,
            Some("Type REMOVE to delete the other panel and its duplicate files."),
        );
    }
    let id = form.id.trim().to_string();
    let work = tokio::task::spawn_blocking(move || wipe_panel(&host_root(), &id));
    match tokio::time::timeout(MIGRATE_BUDGET, work).await {
        Ok(Ok(Ok(paths))) => redirect_notice(
            "/server/panel-migrate",
            Some(&format!("Removed {} path(s).", paths.len())),
            None,
        ),
        Ok(Ok(Err(error))) => {
            redirect_notice("/server/panel-migrate", None, Some(&truncate(&error)))
        }
        _ => redirect_notice(
            "/server/panel-migrate",
            None,
            Some("Remove did not finish. Run cpn panel-migrate wipe on the server."),
        ),
    }
}

fn truncate(value: &str) -> String {
    let mut chars = value.chars();
    let short: String = chars.by_ref().take(240).collect();
    if chars.next().is_some() {
        format!("{short}...")
    } else {
        short
    }
}
