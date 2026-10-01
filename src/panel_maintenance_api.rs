//! Public HTTP endpoints for panel upgrade maintenance status / page.

use crate::panel_maintenance_mode::{self, MaintenanceFlag};
use crate::panel_maintenance_page::{self, witty_for_phase};
use actix_web::{HttpResponse, get};

fn public_json(flag: Option<&MaintenanceFlag>) -> serde_json::Value {
    match flag {
        Some(flag) => serde_json::json!({
            "active": true,
            "phase": flag.phase,
            "progress": flag.progress,
            "message": flag.message,
            "title": flag.title,
            "target": flag.target,
            "source": flag.source,
            "started_at": panel_maintenance_mode::format_nb_datetime(flag.started_at_unix),
            "updated_at": panel_maintenance_mode::format_nb_datetime(flag.updated_at_unix),
            "flavor": witty_for_phase(&flag.phase),
        }),
        None => serde_json::json!({
            "active": false,
            "phase": "",
            "progress": 100,
            "message": "Panel is online",
            "title": "CPN Panel",
            "target": serde_json::Value::Null,
            "source": "",
            "started_at": "",
            "updated_at": "",
            "flavor": "",
        }),
    }
}

/// Public poll endpoint used by the maintenance page auto-retry (no auth).
#[get("/api/panel-maintenance")]
pub async fn api_panel_maintenance() -> HttpResponse {
    let flag = panel_maintenance_mode::load_active();
    let mut response = HttpResponse::Ok();
    response.insert_header(("Cache-Control", "no-store"));
    if flag.is_some() {
        response.insert_header(("Retry-After", "5"));
    }
    response.json(public_json(flag.as_ref()))
}

/// Dedicated maintenance page (always reachable; shows live status when active).
#[get("/maintenance")]
pub async fn maintenance_page() -> HttpResponse {
    if let Some(flag) = panel_maintenance_mode::load_active() {
        return HttpResponse::ServiceUnavailable()
            .content_type("text/html; charset=utf-8")
            .insert_header(("Cache-Control", "no-store"))
            .insert_header(("Retry-After", "5"))
            .body(panel_maintenance_page::render_html(&flag));
    }
    HttpResponse::SeeOther()
        .insert_header(("Location", "/"))
        .finish()
}
