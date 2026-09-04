//! Backups hub feature routes.

use crate::installer::AppState;
use crate::panel_backups::BackupsPageQuery;
use crate::panel_hub_http::{html_ok, login_redirect, redirect_notice, require_panel_user};
use crate::panel_hub_pages_backups::{
    backups_create_page, backups_destinations_page, backups_restore_page, backups_schedule_page,
    save_backup_destinations, save_backup_schedule,
};
use crate::panel_hub_pages_hosting::scaffold_feature;
use crate::panel_pages::panel_shell;
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use std::sync::Arc;

#[get("/backups/create")]
pub async fn backups_create_route(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    html_ok(panel_shell(
        &user,
        "backups",
        "Create Backup",
        &backups_create_page(BackupsPageQuery {
            notice: query.get("notice").map(String::as_str),
            error: query.get("error").map(String::as_str),
            scope: query.get("scope").map(String::as_str).unwrap_or("panel"),
            domain: query.get("domain").map(String::as_str).unwrap_or(""),
        }),
    ))
}

#[get("/backups/restore")]
pub async fn backups_restore_route(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    html_ok(panel_shell(
        &user,
        "backups",
        "Restore Backup",
        &backups_restore_page(
            query.get("scope").map(String::as_str).unwrap_or("panel"),
            query.get("domain").map(String::as_str).unwrap_or(""),
        ),
    ))
}

#[get("/backups/schedule")]
pub async fn backups_schedule_route(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    html_ok(panel_shell(
        &user,
        "backups",
        "Schedule Backup",
        &backups_schedule_page(
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}

#[derive(Debug, serde::Deserialize)]
pub struct ScheduleForm {
    #[serde(default)]
    enabled: String,
    #[serde(default)]
    cron: String,
    #[serde(default)]
    scope: String,
    #[serde(default)]
    domain: String,
}

#[post("/backups/schedule/save")]
pub async fn backups_schedule_save(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<ScheduleForm>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    let enabled = matches!(form.enabled.trim(), "1" | "true" | "on" | "yes");
    match save_backup_schedule(enabled, &form.cron, &form.scope, &form.domain) {
        Ok(msg) => redirect_notice("/backups/schedule", Some(&msg), None),
        Err(err) => redirect_notice("/backups/schedule", None, Some(&err)),
    }
}

#[get("/backups/destinations")]
pub async fn backups_destinations_route(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    html_ok(panel_shell(
        &user,
        "backups",
        "Destinations",
        &backups_destinations_page(
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}

#[derive(Debug, serde::Deserialize)]
pub struct DestinationsForm {
    #[serde(default)]
    local_enabled: String,
    #[serde(default)]
    google_drive_note: String,
    #[serde(default)]
    remote_note: String,
}

#[post("/backups/destinations/save")]
pub async fn backups_destinations_save(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<DestinationsForm>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    let local = matches!(form.local_enabled.trim(), "1" | "true" | "on" | "yes");
    match save_backup_destinations(local, &form.google_drive_note, &form.remote_note) {
        Ok(msg) => redirect_notice("/backups/destinations", Some(&msg), None),
        Err(err) => redirect_notice("/backups/destinations", None, Some(&err)),
    }
}

#[get("/backups/google-drive")]
pub async fn backups_gdrive_route(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    html_ok(panel_shell(
        &user,
        "backups",
        "Google Drive",
        &scaffold_feature(
            "Backups",
            "/backups",
            "Google Drive",
            "Backup to Drive",
            "Google Drive OAuth and sync are not configured yet.",
        ),
    ))
}

#[get("/backups/remote")]
pub async fn backups_remote_route(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    html_ok(panel_shell(
        &user,
        "backups",
        "Remote Backups",
        &scaffold_feature(
            "Backups",
            "/backups",
            "Remote Backups",
            "Transfer to another server",
            "Remote transfer targets are not configured yet.",
        ),
    ))
}
