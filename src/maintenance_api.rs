//! HTTP endpoints for installer maintenance (upgrade / repair / downgrade).

use crate::auth_api::panel_user_from_request;
use crate::http_helpers::authorized_request;
use crate::installer::AppState;
use crate::manifest::detect_existing_install;
use crate::model::{MaintenanceAction, MaintenanceInfo, MaintenanceRequest, TokenQuery};
use crate::panel_admin::is_panel_admin;
use crate::releases;
use crate::upgrade::{build_plan, spawn_maintenance};
use actix_web::{HttpRequest, HttpResponse, Responder, get, post, web};
use std::sync::Arc;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn is_root() -> bool {
    #[cfg(unix)]
    {
        unsafe { libc::geteuid() == 0 }
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// Installer token/session, or signed-in panel admin.
fn maintenance_authorized(state: &AppState, query: &TokenQuery, http: &HttpRequest) -> bool {
    if authorized_request(state, query, http) {
        return true;
    }
    matches!(
        panel_user_from_request(state, http),
        Some(user) if is_panel_admin(&user)
    )
}

/// Installer token/session, or any signed-in panel user (read-only version info).
fn version_read_authorized(state: &AppState, query: &TokenQuery, http: &HttpRequest) -> bool {
    if authorized_request(state, query, http) {
        return true;
    }
    panel_user_from_request(state, http).is_some()
}

fn busy_phase(phase: &str) -> bool {
    matches!(
        phase,
        "configuring" | "downloading" | "installing" | "testing" | "verifying"
    )
}

pub async fn load_maintenance_info() -> MaintenanceInfo {
    let existing = detect_existing_install(VERSION);
    let check = releases::version_check(VERSION, &existing.package_version).await;
    let plan = Some(build_plan(
        MaintenanceAction::Repair,
        Some(&existing.package_version),
        &existing.package_version,
        false,
    ));
    MaintenanceInfo {
        existing_install: existing.detected,
        installed_version: existing.package_version,
        running_version: VERSION.into(),
        latest_version: check.latest_version.clone(),
        latest_tag: check.latest_tag.clone(),
        update_available: check.update_available,
        downgrade_possible: check.downgrade_possible,
        repo: check.repo,
        source: check.source,
        releases: check.releases,
        has_manifest: existing.has_manifest,
        has_bootstrap: existing.has_bootstrap,
        plan,
        check_error: check.error,
    }
}

#[get("/api/version-check")]
pub async fn api_version_check(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<TokenQuery>,
) -> HttpResponse {
    if !version_read_authorized(&state, &query, &http) {
        return HttpResponse::Unauthorized().finish();
    }
    let info = load_maintenance_info().await;
    let mut status = state.status.write().unwrap_or_else(|e| e.into_inner());
    status.maintenance = Some(info.clone());
    HttpResponse::Ok().json(info)
}

#[get("/api/releases")]
pub async fn api_releases(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<TokenQuery>,
) -> HttpResponse {
    if !version_read_authorized(&state, &query, &http) {
        return HttpResponse::Unauthorized().finish();
    }
    match releases::list_releases(20).await {
        Ok(list) => HttpResponse::Ok().json(list),
        Err(error) => HttpResponse::BadGateway().json(serde_json::json!({ "error": error })),
    }
}

#[get("/api/maintenance/status")]
pub async fn api_maintenance_status(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<TokenQuery>,
) -> HttpResponse {
    if !maintenance_authorized(&state, &query, &http) {
        return HttpResponse::Unauthorized().finish();
    }
    let status = state.status.read().unwrap_or_else(|e| e.into_inner());
    let busy = busy_phase(status.phase);
    HttpResponse::Ok().json(serde_json::json!({
        "phase": status.phase,
        "progress": status.progress,
        "message": status.message,
        "busy": busy,
        "error": status.error,
        "version": status.version,
    }))
}

#[post("/api/maintenance")]
pub async fn start_maintenance(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<TokenQuery>,
    request: web::Json<MaintenanceRequest>,
) -> impl Responder {
    if !maintenance_authorized(&state, &query, &http) {
        return HttpResponse::Unauthorized().finish();
    }
    if !is_root() {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Package upgrade/downgrade requires root. Run cpn-installer.service as root, or use: sudo cpn-installer --upgrade"
        }));
    }
    if !request.confirm_execute && !matches!(request.action, MaintenanceAction::ConfigOnly) {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "confirm_execute is required for upgrade, downgrade, and repair"
        }));
    }
    if matches!(request.action, MaintenanceAction::Downgrade) && !request.confirm_downgrade {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "confirm_downgrade is required for downgrade"
        }));
    }
    let mut current = state.status.write().unwrap_or_else(|e| e.into_inner());
    if busy_phase(current.phase) {
        return HttpResponse::Conflict()
            .json(serde_json::json!({ "error": "An operation is already in progress" }));
    }
    let existing = detect_existing_install(VERSION);
    let plan = build_plan(
        request.action,
        request.version.as_deref(),
        &existing.package_version,
        request.reset_data,
    );
    if let Some(info) = current.maintenance.as_mut() {
        info.plan = Some(plan.clone());
    } else {
        current.maintenance = Some(MaintenanceInfo {
            existing_install: existing.detected,
            installed_version: existing.package_version.clone(),
            running_version: VERSION.into(),
            latest_version: None,
            latest_tag: None,
            update_available: false,
            downgrade_possible: false,
            repo: releases::github_repo(),
            source: releases::package_source_label(),
            releases: Vec::new(),
            has_manifest: existing.has_manifest,
            has_bootstrap: existing.has_bootstrap,
            plan: Some(plan.clone()),
            check_error: None,
        });
    }
    current.phase = "downloading";
    current.progress = 1;
    current.error = None;
    current.message = plan.summary.clone();
    drop(current);
    tokio::spawn(spawn_maintenance(
        state.get_ref().clone(),
        request.into_inner(),
    ));
    HttpResponse::Accepted().json(plan)
}
