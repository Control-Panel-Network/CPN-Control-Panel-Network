//! JSON APIs for dashboard overview layout (per signed-in user).

use crate::auth_api::panel_user_from_request;
use crate::installer::AppState;
use crate::panel_dashboard_layout::{
    load_dashboard_widgets, restore_dashboard_layout, save_dashboard_layout, DEFAULT_DASH_WIDGETS,
};
use crate::panel_user_prefs::load_user_ui_prefs;
use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;
use std::sync::Arc;

fn json_ok(value: serde_json::Value) -> HttpResponse {
    HttpResponse::Ok()
        .content_type("application/json; charset=utf-8")
        .body(serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{\"ok\":false}".into()))
}

fn json_err(status: u16, message: &str) -> HttpResponse {
    let body = serde_json::to_string_pretty(&serde_json::json!({ "ok": false, "error": message }))
        .unwrap_or_else(|_| "{\"ok\":false}".into());
    let mut response = match status {
        401 => HttpResponse::Unauthorized(),
        400 => HttpResponse::BadRequest(),
        _ => HttpResponse::InternalServerError(),
    };
    response
        .content_type("application/json; charset=utf-8")
        .body(body)
}

fn layout_payload(username: &str) -> serde_json::Value {
    let prefs = load_user_ui_prefs(username);
    serde_json::json!({
        "ok": true,
        "widgets": load_dashboard_widgets(username),
        "default_widgets": DEFAULT_DASH_WIDGETS,
        "activity_board_open": prefs.activity_board_open,
    })
}

#[derive(Debug, Deserialize)]
pub struct DashboardLayoutBody {
    #[serde(default)]
    widgets: Vec<String>,
    #[serde(default)]
    activity_board_open: Option<bool>,
}

#[get("/api/panel/dashboard-layout")]
pub async fn panel_dashboard_layout_get(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(user) = panel_user_from_request(&state, &http) else {
        return json_err(401, "Login required");
    };
    json_ok(layout_payload(&user))
}

#[post("/api/panel/dashboard-layout")]
pub async fn panel_dashboard_layout_set(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<DashboardLayoutBody>,
) -> HttpResponse {
    let Some(user) = panel_user_from_request(&state, &http) else {
        return json_err(401, "Login required");
    };
    match save_dashboard_layout(&user, &body.widgets, body.activity_board_open) {
        Ok(_) => json_ok(layout_payload(&user)),
        Err(err) => json_err(500, &err),
    }
}

#[post("/api/panel/dashboard-layout/restore")]
pub async fn panel_dashboard_layout_restore(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(user) = panel_user_from_request(&state, &http) else {
        return json_err(401, "Login required");
    };
    match restore_dashboard_layout(&user) {
        Ok(_) => json_ok(layout_payload(&user)),
        Err(err) => json_err(500, &err),
    }
}
