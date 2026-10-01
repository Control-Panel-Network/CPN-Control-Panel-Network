//! JSON APIs for panel color mode and Design profiles.

use crate::auth_api::panel_user_from_request;
use crate::installer::AppState;
use crate::panel_admin::is_panel_admin;
use crate::panel_theme::{
    ColorMode, DesignPreset, DesignTokens, apply_design_preset, design_public_json,
    load_panel_design, restore_default_design, save_custom_tokens, save_custom_tokens_with_theme,
};
use crate::panel_user_prefs::{
    load_user_color_mode, load_user_minimalist_mode, save_user_color_mode,
    save_user_minimalist_mode,
};
use crate::plugins::format_unix_local;
use crate::themes_catalog::{
    fetch_themes_catalog, themes_next_refresh_unix, themes_repo_slug, themes_repo_url,
};
use crate::themes_install::{
    enrich_catalog_for_store, install_theme, list_installed_themes, load_installed_theme,
    theme_is_installed, uninstall_theme,
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
        404 => HttpResponse::NotFound(),
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

#[derive(Debug, Deserialize)]
pub struct MinimalistModeBody {
    #[serde(default)]
    minimalist_mode: bool,
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

#[get("/api/panel/minimalist-mode")]
pub async fn panel_minimalist_mode_get(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return json_err(401, "Login required");
    };
    let enabled = load_user_minimalist_mode(&user);
    json_ok(serde_json::json!({
        "ok": true,
        "minimalist_mode": enabled,
        "username": user,
    }))
}

#[post("/api/panel/minimalist-mode")]
pub async fn panel_minimalist_mode_set(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<MinimalistModeBody>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return json_err(401, "Login required");
    };
    match save_user_minimalist_mode(&user, body.minimalist_mode) {
        Ok(saved) => json_ok(serde_json::json!({
            "ok": true,
            "minimalist_mode": saved,
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
            // Light/Dark design presets also set this operator's color mode so the
            // chrome surfaces change visibly (not only accent tokens).
            let synced_color = match preset {
                DesignPreset::Light => save_user_color_mode(&user, ColorMode::Light).ok(),
                DesignPreset::Dark => save_user_color_mode(&user, ColorMode::Dark).ok(),
                DesignPreset::Default | DesignPreset::Custom => None,
            };
            let mut payload = design_public_json(&design);
            if let Some(obj) = payload.as_object_mut() {
                obj.insert("ok".into(), serde_json::json!(true));
                if let Some(mode) = synced_color {
                    obj.insert("color_mode".into(), serde_json::json!(mode.as_str()));
                }
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

#[derive(Debug, Deserialize)]
pub struct ThemesCatalogQuery {
    #[serde(default)]
    refresh: Option<String>,
}

#[get("/api/panel/themes/catalog")]
pub async fn panel_themes_catalog(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<ThemesCatalogQuery>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return json_err(401, "Login required");
    };
    let force = query
        .refresh
        .as_deref()
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    match fetch_themes_catalog(force) {
        Ok((entries, fetched_at)) => {
            let design = load_panel_design();
            let active = design.active_theme_id.as_deref();
            let themes = enrich_catalog_for_store(&entries, active);
            let installed = list_installed_themes();
            let new_count = themes.iter().filter(|t| !t.installed).count();
            json_ok(serde_json::json!({
                "ok": true,
                "repo": themes_repo_slug(),
                "repo_url": themes_repo_url(),
                "fetched_at_unix": fetched_at,
                "next_refresh_unix": themes_next_refresh_unix(fetched_at),
                "fetched_at_local": format_unix_local(fetched_at),
                "active_theme_id": design.active_theme_id,
                "installed_count": installed.len(),
                "available_count": new_count,
                "themes": themes,
            }))
        }
        Err(err) => json_err(500, &err),
    }
}

#[derive(Debug, Deserialize)]
pub struct ThemeIdBody {
    id: String,
}

#[post("/api/panel/themes/install")]
pub async fn panel_themes_install(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<ThemeIdBody>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return json_err(401, "Login required");
    };
    if !is_panel_admin(&user) {
        return json_err(403, "Only the panel admin can install Design themes");
    }
    match install_theme(&body.id) {
        Ok(manifest) => json_ok(serde_json::json!({
            "ok": true,
            "installed": true,
            "theme": manifest,
        })),
        Err(err) => json_err(400, &err),
    }
}

#[post("/api/panel/themes/uninstall")]
pub async fn panel_themes_uninstall(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<ThemeIdBody>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return json_err(401, "Login required");
    };
    if !is_panel_admin(&user) {
        return json_err(403, "Only the panel admin can uninstall Design themes");
    }
    let id = body.id.trim();
    let design = load_panel_design();
    let was_active = design
        .active_theme_id
        .as_deref()
        .map(|a| a.eq_ignore_ascii_case(id))
        .unwrap_or(false);
    match uninstall_theme(id) {
        Ok(()) => {
            if was_active {
                let mut cleared = design;
                cleared.active_theme_id = None;
                let _ = crate::panel_theme::save_panel_design(&cleared);
            }
            json_ok(serde_json::json!({
                "ok": true,
                "uninstalled": true,
                "id": id,
                "cleared_active": was_active,
            }))
        }
        Err(err) => json_err(404, &err),
    }
}

#[post("/api/panel/themes/apply")]
pub async fn panel_themes_apply(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<ThemeIdBody>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return json_err(401, "Login required");
    };
    if !is_panel_admin(&user) {
        return json_err(403, "Only the panel admin can apply Design themes");
    }
    let id = body.id.trim();
    // Prefer installed package; auto-install from catalog when missing (Plugin Store parity).
    let theme = if theme_is_installed(id) {
        match load_installed_theme(id) {
            Ok(m) => m,
            Err(err) => return json_err(404, &err),
        }
    } else {
        match install_theme(id) {
            Ok(m) => m,
            Err(err) => return json_err(404, &err),
        }
    };
    match save_custom_tokens_with_theme(theme.tokens.clone(), Some(&theme.id)) {
        Ok(design) => {
            let mut payload = design_public_json(&design);
            if let Some(obj) = payload.as_object_mut() {
                obj.insert("ok".into(), serde_json::json!(true));
                obj.insert("applied_theme".into(), serde_json::json!(theme.id));
                obj.insert("applied_theme_name".into(), serde_json::json!(theme.name));
                obj.insert("installed".into(), serde_json::json!(true));
            }
            json_ok(payload)
        }
        Err(err) => json_err(400, &err),
    }
}
