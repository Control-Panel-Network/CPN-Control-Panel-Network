//! HTTP endpoints for installer maintenance (upgrade / repair / downgrade).

use crate::installer::AppState;
use crate::manifest::detect_existing_install;
use crate::model::{MaintenanceAction, MaintenanceInfo, MaintenanceRequest, TokenQuery};
use crate::releases;
use crate::upgrade::{build_plan, spawn_maintenance};
use actix_web::{HttpResponse, Responder, get, post, web};
use std::sync::Arc;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn authorized(state: &AppState, query: &TokenQuery) -> bool {
    state.token.as_bytes() == query.token.as_bytes()
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
    state: web::Data<Arc<AppState>>,
    query: web::Query<TokenQuery>,
) -> HttpResponse {
    if !authorized(&state, &query) {
        return HttpResponse::Unauthorized().finish();
    }
    let info = load_maintenance_info().await;
    let mut status = state.status.write().await;
    status.maintenance = Some(info.clone());
    HttpResponse::Ok().json(info)
}

#[get("/api/releases")]
pub async fn api_releases(
    state: web::Data<Arc<AppState>>,
    query: web::Query<TokenQuery>,
) -> HttpResponse {
    if !authorized(&state, &query) {
        return HttpResponse::Unauthorized().finish();
    }
    match releases::list_releases(20).await {
        Ok(list) => HttpResponse::Ok().json(list),
        Err(error) => HttpResponse::BadGateway().json(serde_json::json!({ "error": error })),
    }
}

#[post("/api/maintenance")]
pub async fn start_maintenance(
    state: web::Data<Arc<AppState>>,
    query: web::Query<TokenQuery>,
    request: web::Json<MaintenanceRequest>,
) -> impl Responder {
    if !authorized(&state, &query) {
        return HttpResponse::Unauthorized().finish();
    }
    #[cfg(unix)]
    if unsafe { libc::geteuid() } != 0 {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Run the installer as root (sudo cpn-installer)"
        }));
    }
    let mut current = state.status.write().await;
    if ["downloading", "installing", "testing"].contains(&current.phase) {
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
