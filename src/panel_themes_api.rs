//! Theme Store JSON APIs (catalog, install, update-all, activity log).

use crate::auth_api::panel_user_from_request;
use crate::installer::AppState;
use crate::panel_action_log;
use crate::panel_admin::is_panel_admin;
use crate::panel_theme::{
    ColorMode, design_public_json, load_panel_design, save_custom_tokens_with_theme,
};
use crate::panel_user_prefs::save_user_color_mode;
use crate::plugins::format_unix_local;
use crate::themes_catalog::{
    fetch_themes_catalog, merge_with_paid_catalog, themes_next_refresh_unix, themes_repo_slug,
    themes_repo_url,
};
use crate::themes_install::{
    enrich_catalog_for_store, install_theme, list_installed_themes, load_installed_theme,
    load_installed_theme_extra_css, resolve_theme_asset_file, theme_asset_content_type,
    theme_is_installed, uninstall_theme,
};
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use serde::Deserialize;
use std::sync::Arc;

fn require_panel_user(state: &AppState, http: &HttpRequest) -> Option<String> {
    panel_user_from_request(state, http)
}

fn log_theme_action(actor: &str, action: &str, theme_id: &str, ok: bool, message: &str) {
    if let Err(error) = panel_action_log::record(actor, action, theme_id, ok, message) {
        eprintln!("panel-action-log: could not record {action}: {error}");
    }
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
            let merged = merge_with_paid_catalog(entries);
            let themes = enrich_catalog_for_store(&merged, active);
            let installed = list_installed_themes();
            let new_count = themes.iter().filter(|t| !t.installed).count();
            let paid_count = themes
                .iter()
                .filter(|t| crate::theme_entitlements::theme_is_paid(&t.entry.pricing))
                .count();
            json_ok(serde_json::json!({
                "ok": true,
                "repo": themes_repo_slug(),
                "repo_url": themes_repo_url(),
                "shop_catalog_url": crate::theme_entitlements::shop_purchase_url(),
                "store_category_url": crate::theme_entitlements::store_category_url(),
                "fetched_at_unix": fetched_at,
                "next_refresh_unix": themes_next_refresh_unix(fetched_at),
                "fetched_at_local": format_unix_local(fetched_at),
                "active_theme_id": design.active_theme_id,
                "installed_count": installed.len(),
                "available_count": new_count,
                "paid_count": paid_count,
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

#[derive(Debug, Deserialize)]
pub struct ThemeRedeemBody {
    id: String,
    #[serde(default)]
    activation_key: String,
    #[serde(default)]
    license_email: String,
    #[serde(default)]
    grant_plugin: String,
}

#[post("/api/panel/themes/redeem")]
pub async fn panel_themes_redeem(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<ThemeRedeemBody>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return json_err(401, "Login required");
    };
    if !is_panel_admin(&user) {
        return json_err(403, "Only the panel admin can redeem theme licenses");
    }
    let id = body.id.trim().to_ascii_lowercase();
    if id.is_empty() {
        return json_err(400, "theme id is required");
    }
    let grant = if body.grant_plugin.trim().is_empty() {
        // Prefer catalog grant_plugin when omitted.
        crate::theme_entitlements::fetch_paid_themes_catalog()
            .ok()
            .and_then(|paid| {
                paid.into_iter()
                    .find(|p| p.id.eq_ignore_ascii_case(&id))
                    .map(|p| p.grant_plugin)
            })
            .unwrap_or_else(|| format!("cpn-theme-{id}"))
    } else {
        body.grant_plugin.trim().to_string()
    };
    match crate::theme_entitlements::redeem_theme_activation_key(
        &id,
        &grant,
        &body.activation_key,
        &body.license_email,
    ) {
        Ok(entry) => {
            let msg = format!("Unlocked {} ({})", entry.theme_id, entry.via);
            log_theme_action(&user, "theme.redeem", &id, true, &msg);
            json_ok(serde_json::json!({
                "ok": true,
                "message": msg,
                "theme_id": entry.theme_id,
                "entitled": entry.entitled,
                "via": entry.via,
            }))
        }
        Err(err) => {
            log_theme_action(&user, "theme.redeem", &id, false, &err);
            json_err(400, &err)
        }
    }
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
    let id = body.id.trim();
    let was_installed = theme_is_installed(id);
    let action = if was_installed {
        "theme.update"
    } else {
        "theme.install"
    };
    match install_theme(id) {
        Ok(manifest) => {
            let msg = if was_installed {
                format!("Updated {} to v{}", manifest.id, manifest.version)
            } else {
                format!("Installed {} v{}", manifest.id, manifest.version)
            };
            log_theme_action(&user, action, &manifest.id, true, &msg);
            json_ok(serde_json::json!({
                "ok": true,
                "installed": true,
                "updated": was_installed,
                "theme": manifest,
            }))
        }
        Err(err) => {
            log_theme_action(&user, action, id, false, &err);
            json_err(400, &err)
        }
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
                cleared.active_theme_background = None;
                cleared.active_theme_extra_css = None;
                let _ = crate::panel_theme::save_panel_design(&cleared);
            }
            let msg = if was_active {
                format!("Uninstalled {id} (cleared active theme)")
            } else {
                format!("Uninstalled {id}")
            };
            log_theme_action(&user, "theme.uninstall", id, true, &msg);
            json_ok(serde_json::json!({
                "ok": true,
                "uninstalled": true,
                "id": id,
                "cleared_active": was_active,
            }))
        }
        Err(err) => {
            log_theme_action(&user, "theme.uninstall", id, false, &err);
            json_err(404, &err)
        }
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
            Err(err) => {
                log_theme_action(&user, "theme.apply", id, false, &err);
                return json_err(404, &err);
            }
        }
    } else {
        match install_theme(id) {
            Ok(m) => {
                log_theme_action(
                    &user,
                    "theme.install",
                    &m.id,
                    true,
                    &format!("Installed {} v{} (during apply)", m.id, m.version),
                );
                m
            }
            Err(err) => {
                log_theme_action(&user, "theme.apply", id, false, &err);
                return json_err(404, &err);
            }
        }
    };
    let extra_css = load_installed_theme_extra_css(&theme.id);
    match save_custom_tokens_with_theme(
        theme.tokens.clone(),
        Some(&theme.id),
        theme.background.clone(),
        extra_css,
    ) {
        Ok(design) => {
            // Prefer the theme's intended light/dark surfaces so backgrounds read correctly.
            let synced_color = theme
                .background
                .as_ref()
                .and_then(|bg| bg.color_mode.as_deref())
                .and_then(ColorMode::parse)
                .and_then(|mode| save_user_color_mode(&user, mode).ok());
            log_theme_action(
                &user,
                "theme.apply",
                &theme.id,
                true,
                &format!("Applied {}", theme.name),
            );
            let mut payload = design_public_json(&design);
            if let Some(obj) = payload.as_object_mut() {
                obj.insert("ok".into(), serde_json::json!(true));
                obj.insert("applied_theme".into(), serde_json::json!(theme.id));
                obj.insert("applied_theme_name".into(), serde_json::json!(theme.name));
                obj.insert("installed".into(), serde_json::json!(true));
                obj.insert(
                    "has_background".into(),
                    serde_json::json!(theme.background.is_some()),
                );
                if let Some(mode) = synced_color {
                    obj.insert("color_mode".into(), serde_json::json!(mode.as_str()));
                }
            }
            json_ok(payload)
        }
        Err(err) => {
            log_theme_action(&user, "theme.apply", &theme.id, false, &err);
            json_err(400, &err)
        }
    }
}

/// Update every installed theme that has a newer catalog version.
#[post("/api/panel/themes/update-all")]
pub async fn panel_themes_update_all(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return json_err(401, "Login required");
    };
    if !is_panel_admin(&user) {
        return json_err(403, "Only the panel admin can update Design themes");
    }
    let (entries, _) = match fetch_themes_catalog(true) {
        Ok(v) => v,
        Err(err) => {
            log_theme_action(&user, "theme.update-all", "", false, &err);
            return json_err(500, &err);
        }
    };
    let design = load_panel_design();
    let merged = merge_with_paid_catalog(entries);
    let rows = enrich_catalog_for_store(&merged, design.active_theme_id.as_deref());
    let pending: Vec<_> = rows
        .into_iter()
        .filter(|row| row.update_available && !row.locked)
        .collect();
    if pending.is_empty() {
        log_theme_action(
            &user,
            "theme.update-all",
            "",
            true,
            "No theme updates available",
        );
        return json_ok(serde_json::json!({
            "ok": true,
            "updated": [],
            "failed": [],
            "count": 0,
            "message": "No theme updates available",
        }));
    }
    let mut updated = Vec::new();
    let mut failed = Vec::new();
    for row in &pending {
        let id = row.entry.id.as_str();
        match install_theme(id) {
            Ok(manifest) => {
                let msg = format!("Updated {} to v{}", manifest.id, manifest.version);
                log_theme_action(&user, "theme.update", &manifest.id, true, &msg);
                updated.push(serde_json::json!({
                    "id": manifest.id,
                    "version": manifest.version,
                }));
            }
            Err(err) => {
                log_theme_action(&user, "theme.update", id, false, &err);
                failed.push(serde_json::json!({ "id": id, "error": err }));
            }
        }
    }
    let ok = failed.is_empty();
    let summary = format!(
        "Update all: {} updated, {} failed (of {} pending)",
        updated.len(),
        failed.len(),
        pending.len()
    );
    log_theme_action(&user, "theme.update-all", "", ok, &summary);
    json_ok(serde_json::json!({
        "ok": ok,
        "updated": updated,
        "failed": failed,
        "count": pending.len(),
        "message": summary,
    }))
}

/// Recent theme install/update/uninstall/apply actions (panel action log).
#[get("/api/panel/themes/actions")]
pub async fn panel_themes_actions(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return json_err(401, "Login required");
    };
    let filter = if is_panel_admin(&user) {
        None
    } else {
        Some(user.as_str())
    };
    let rows: Vec<_> = panel_action_log::recent(100, filter)
        .into_iter()
        .filter(|row| row.action.starts_with("theme."))
        .take(25)
        .map(|row| {
            serde_json::json!({
                "ts": row.ts,
                "actor": row.actor,
                "action": row.action,
                "label": panel_action_log::action_label(&row.action),
                "theme_id": row.target,
                "ok": row.ok,
                "message": row.message,
            })
        })
        .collect();
    json_ok(serde_json::json!({
        "ok": true,
        "actions": rows,
        "logs_url": "/settings/logs",
    }))
}

#[get("/api/panel/themes/assets/{id}/{file}")]
pub async fn panel_themes_asset(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    path: web::Path<(String, String)>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return json_err(401, "Login required");
    };
    let (id, file) = path.into_inner();
    match resolve_theme_asset_file(&id, &file) {
        Ok(asset_path) => match std::fs::read(&asset_path) {
            Ok(bytes) => HttpResponse::Ok()
                .insert_header((
                    actix_web::http::header::CACHE_CONTROL,
                    "private, max-age=3600",
                ))
                .content_type(theme_asset_content_type(&asset_path))
                .body(bytes),
            Err(_) => json_err(404, "Theme asset could not be read"),
        },
        Err(err) => json_err(404, &err),
    }
}
