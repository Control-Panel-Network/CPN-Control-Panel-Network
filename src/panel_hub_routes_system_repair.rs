//! Routes for owner-only System Repair (`/server/system-repair`).

use crate::installer::AppState;
use crate::panel_admin::is_panel_admin;
use crate::panel_hub_http::{
    html_blocking, login_redirect, owner_only_html, redirect_notice, require_panel_user,
};
use crate::panel_hub_pages_system_repair::system_repair_page;
use crate::panel_pages::panel_shell;
use crate::system_repair::{PRODUCT_NAME, run_suite};
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use std::collections::HashMap;
use std::sync::Arc;

#[get("/server/system-repair")]
pub async fn system_repair_route(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !is_panel_admin(&user) {
        return owner_only_html(&user, "server", PRODUCT_NAME);
    }
    html_blocking(move || {
        panel_shell(
            &user,
            "server",
            PRODUCT_NAME,
            &system_repair_page(
                query.get("notice").map(String::as_str),
                query.get("error").map(String::as_str),
            ),
        )
    })
    .await
}

#[get("/server/system-repair/api")]
pub async fn system_repair_api_route(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !is_panel_admin(&user) {
        return HttpResponse::Forbidden().json(serde_json::json!({
            "ok": false,
            "error": "owner-only"
        }));
    }
    let report = web::block(|| run_suite(false, None, None))
        .await
        .unwrap_or_else(|_| run_suite(false, None, None));
    match report.to_json() {
        Ok(body) => HttpResponse::Ok()
            .content_type("application/json; charset=utf-8")
            .body(body),
        Err(err) => HttpResponse::InternalServerError().json(serde_json::json!({
            "ok": false,
            "error": err
        })),
    }
}

#[post("/server/system-repair/heal")]
pub async fn system_repair_heal_route(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !is_panel_admin(&user) {
        return owner_only_html(&user, "server", PRODUCT_NAME);
    }
    let id = form
        .get("id")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let report = web::block(move || run_suite(true, id.as_deref(), None))
        .await
        .unwrap_or_else(|_| run_suite(true, None, None));
    let failed: Vec<String> = report
        .heals
        .iter()
        .filter(|h| !h.ok)
        .map(|h| format!("{}: {}", h.heal_id, h.message))
        .collect();
    if failed.is_empty() {
        let msg = if report.heals.is_empty() {
            "No heal actions were needed.".to_string()
        } else {
            format!(
                "Heal finished ({} action(s)). Checks refreshed.",
                report.heals.len()
            )
        };
        redirect_notice("/server/system-repair", Some(&msg), None)
    } else {
        redirect_notice("/server/system-repair", None, Some(&failed.join(" | ")))
    }
}

/// Settings hub alias path (same page).
#[get("/settings/system-repair")]
pub async fn settings_system_repair_redirect() -> HttpResponse {
    HttpResponse::SeeOther()
        .append_header(("Location", "/server/system-repair"))
        .finish()
}
