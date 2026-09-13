//! Settings > Log retention routes.

use crate::installer::AppState;
use crate::panel_admin::is_panel_admin;
use crate::panel_hub_http::{html_ok, login_redirect, redirect_notice, require_panel_user};
use crate::panel_hub_pages_log_retention::log_retention_settings_page;
use crate::panel_log_retention::{LogRetentionPrefs, save_log_retention};
use crate::panel_pages::panel_shell;
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use std::sync::Arc;

#[get("/settings/logs")]
pub async fn settings_logs_page(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !is_panel_admin(&user) {
        return HttpResponse::SeeOther()
            .append_header(("Location", "/settings?error=Admin%20only"))
            .finish();
    }
    html_ok(panel_shell(
        &user,
        "settings",
        "Log retention",
        &log_retention_settings_page(
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}

#[derive(Debug, serde::Deserialize)]
pub struct LogRetentionForm {
    #[serde(default)]
    retention_days: String,
    #[serde(default)]
    max_size_mb: String,
}

#[post("/settings/logs")]
pub async fn settings_logs_save(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<LogRetentionForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !is_panel_admin(&user) {
        return HttpResponse::SeeOther()
            .append_header(("Location", "/settings?error=Admin%20only"))
            .finish();
    }
    let days = form
        .retention_days
        .trim()
        .parse::<u32>()
        .unwrap_or(crate::panel_log_retention::DEFAULT_RETENTION_DAYS);
    let max_size_mb = {
        let t = form.max_size_mb.trim();
        if t.is_empty() {
            None
        } else {
            t.parse::<u32>().ok().filter(|n| *n > 0)
        }
    };
    let prefs = LogRetentionPrefs {
        retention_days: days,
        max_size_mb,
    };
    match save_log_retention(&prefs) {
        Ok(()) => redirect_notice(
            "/settings/logs",
            Some("Log retention saved and applied"),
            None,
        ),
        Err(error) => redirect_notice("/settings/logs", None, Some(&error)),
    }
}
