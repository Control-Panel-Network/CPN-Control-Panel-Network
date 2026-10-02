//! Routes for owner-only System Repair (`/server/system-repair`).

use crate::installer::AppState;
use crate::panel_admin::is_panel_admin;
use crate::panel_hub_http::{
    html_ok, login_redirect, owner_only_html, redirect_notice, require_panel_user,
};
use crate::panel_hub_pages_system_repair::system_repair_page;
use crate::panel_pages::panel_shell;
use crate::system_repair::{PRODUCT_NAME, run_suite_ex};
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

/// Bound the JSON suite so a wedged probe group cannot hang the API forever.
const API_SUITE_BUDGET: Duration = Duration::from_secs(16);

fn wants_refresh(query: &HashMap<String, String>) -> bool {
    query
        .get("refresh")
        .map(|v| {
            let t = v.trim();
            t == "1" || t.eq_ignore_ascii_case("true") || t.eq_ignore_ascii_case("yes")
        })
        .unwrap_or(false)
}

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
    // Shell only: check cards load from `/server/system-repair/api` so hub render
    // never blocks on host probes (avoids the "gathering host status" 503 stub).
    let notice = query.get("notice").cloned();
    let error = query.get("error").cloned();
    html_ok(panel_shell(
        &user,
        "server",
        PRODUCT_NAME,
        &system_repair_page(notice.as_deref(), error.as_deref()),
    ))
}

#[get("/server/system-repair/api")]
pub async fn system_repair_api_route(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<HashMap<String, String>>,
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
    let refresh = wants_refresh(&query);
    let report = match tokio::time::timeout(
        API_SUITE_BUDGET,
        web::block(move || run_suite_ex(false, None, None, refresh)),
    )
    .await
    {
        Ok(Ok(report)) => report,
        Ok(Err(_)) => {
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "ok": false,
                "error": "System Repair worker failed"
            }));
        }
        Err(_) => {
            return HttpResponse::ServiceUnavailable()
                .insert_header(("Retry-After", "3"))
                .json(serde_json::json!({
                    "ok": false,
                    "error": "System Repair checks timed out. Retry; slow probe groups are skipped."
                }));
        }
    };
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
    let report = web::block(move || run_suite_ex(true, id.as_deref(), None, true))
        .await
        .unwrap_or_else(|_| run_suite_ex(true, None, None, true));
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
        redirect_notice("/server/system-repair?refresh=1", Some(&msg), None)
    } else {
        redirect_notice(
            "/server/system-repair?refresh=1",
            None,
            Some(&failed.join(" | ")),
        )
    }
}

/// Settings hub alias path (same page).
#[get("/settings/system-repair")]
pub async fn settings_system_repair_redirect() -> HttpResponse {
    HttpResponse::SeeOther()
        .append_header(("Location", "/server/system-repair"))
        .finish()
}
