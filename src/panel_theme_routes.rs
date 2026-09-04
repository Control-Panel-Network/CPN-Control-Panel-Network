//! JSON APIs for panel color mode and Design profiles.

use crate::auth_api::panel_user_from_request;
use crate::installer::AppState;
use crate::panel_admin::is_panel_admin;
use crate::panel_theme::{
    ColorMode, DesignPreset, DesignTokens, apply_design_preset, design_public_json,
    load_panel_design, load_user_color_mode, restore_default_design, save_custom_tokens,
    save_user_color_mode,
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
        403 => HttpResponse::Forbidden(),
        400 => HttpResponse::BadRequest(),
        _ => HttpResponse::InternalServerError(),
    };
    response
        .content_type("application/json; charset=utf-8")
        .body(body)
}

#[derive(Debug, Deserialize)]
pub struct ColorModeBody {
    color_mode: String,
}

#[get("/api/panel/color-mode")]
pub async fn panel_color_mode_get(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return json_err(401, "Login required");
    };
    let mode = load_user_color_mode(&user);
    json_ok(serde_json::json!({
        "ok": true,
        "color_mode": mode.as_str(),
        "username": user,
    }))
}

#[post("/api/panel/color-mode")]
pub async fn panel_color_mode_set(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<ColorModeBody>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return json_err(401, "Login required");
    };
    let Some(mode) = ColorMode::parse(&body.color_mode) else {
        return json_err(400, "color_mode must be light or dark");
    };
    match save_user_color_mode(&user, mode) {
        Ok(saved) => json_ok(serde_json::json!({
            "ok": true,
            "color_mode": saved.as_str(),
        })),
        Err(err) => json_err(500, &err),
    }
}

#[get("/api/panel/design")]
pub async fn panel_design_get(http: HttpRequest, state: web::Data<Arc<AppState>>) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return json_err(401, "Login required");
    };
    let design = load_panel_design();
    let mut payload = design_public_json(&design);
    if let Some(obj) = payload.as_object_mut() {
        obj.insert("ok".into(), serde_json::json!(true));
    }
    json_ok(payload)
}

#[derive(Debug, Deserialize)]
pub struct DesignSaveBody {
    tokens: DesignTokens,
}

#[post("/api/panel/design")]
pub async fn panel_design_save(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<DesignSaveBody>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return json_err(401, "Login required");
    };
    if !is_panel_admin(&user) {
        return json_err(403, "Only the panel admin can edit Design");
    }
    match save_custom_tokens(body.into_inner().tokens) {
        Ok(design) => {
            let mut payload = design_public_json(&design);
            if let Some(obj) = payload.as_object_mut() {
                obj.insert("ok".into(), serde_json::json!(true));
            }
            json_ok(payload)
        }
        Err(err) => json_err(400, &err),
    }
}

#[derive(Debug, Deserialize)]
pub struct DesignPresetBody {
    preset: String,
}

#[post("/api/panel/design/preset")]
pub async fn panel_design_preset(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<DesignPresetBody>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return json_err(401, "Login required");
    };
    if !is_panel_admin(&user) {
        return json_err(403, "Only the panel admin can edit Design");
    }
    let Some(preset) = DesignPreset::parse(&body.preset) else {
        return json_err(400, "preset must be default, light, dark, or custom");
    };
    match apply_design_preset(preset) {
        Ok(design) => {
            let mut payload = design_public_json(&design);
            if let Some(obj) = payload.as_object_mut() {
                obj.insert("ok".into(), serde_json::json!(true));
            }
            json_ok(payload)
        }
        Err(err) => json_err(500, &err),
    }
}

#[post("/api/panel/design/restore")]
pub async fn panel_design_restore(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return json_err(401, "Login required");
    };
    if !is_panel_admin(&user) {
        return json_err(403, "Only the panel admin can restore Design");
    }
    match restore_default_design() {
        Ok(design) => {
            let mut payload = design_public_json(&design);
            if let Some(obj) = payload.as_object_mut() {
                obj.insert("ok".into(), serde_json::json!(true));
                obj.insert(
                    "restored".into(),
                    serde_json::json!("Default (immutable baseline)"),
                );
            }
            json_ok(payload)
        }
        Err(err) => json_err(500, &err),
    }
}
