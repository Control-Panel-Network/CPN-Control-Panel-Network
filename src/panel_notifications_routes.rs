//! JSON APIs for panel notifications.

use crate::auth_api::panel_user_from_request;
use crate::installer::AppState;
use crate::panel_notifications::{
    load_notifications, mark_read, notifications_public_json, push_notification,
};
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use serde::Deserialize;
use std::sync::Arc;

fn require_panel_user(state: &AppState, http: &HttpRequest) -> Option<String> {
    panel_user_from_request(state, http)
}

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

#[get("/api/panel/notifications")]
pub async fn panel_notifications_get(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return json_err(401, "Login required");
    };
    let store = load_notifications(&user);
    json_ok(notifications_public_json(&store))
}

#[derive(Debug, Deserialize)]
pub struct MarkReadBody {
    #[serde(default)]
    ids: Vec<String>,
    #[serde(default)]
    all: bool,
}

#[post("/api/panel/notifications/mark-read")]
pub async fn panel_notifications_mark_read(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<MarkReadBody>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return json_err(401, "Login required");
    };
    if !body.all && body.ids.is_empty() {
        return json_err(400, "Provide ids or set all=true");
    }
    match mark_read(&user, &body.ids, body.all) {
        Ok(store) => json_ok(notifications_public_json(&store)),
        Err(err) => json_err(500, &err),
    }
}

#[derive(Debug, Deserialize)]
pub struct PushBody {
    title: String,
    #[serde(default)]
    body: String,
    #[serde(default)]
    category: String,
}

/// Authenticated self-service push (smoke / operator tooling). Category defaults to `panel`.
#[post("/api/panel/notifications")]
pub async fn panel_notifications_push(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<PushBody>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return json_err(401, "Login required");
    };
    let category = if body.category.trim().is_empty() {
        "panel"
    } else {
        body.category.trim()
    };
    match push_notification(&user, &body.title, &body.body, category) {
        Ok(item) => {
            let store = load_notifications(&user);
            let mut payload = notifications_public_json(&store);
            if let Some(obj) = payload.as_object_mut() {
                obj.insert(
                    "created".into(),
                    serde_json::to_value(item).unwrap_or_default(),
                );
            }
            json_ok(payload)
        }
        Err(err) => json_err(400, &err),
    }
}
