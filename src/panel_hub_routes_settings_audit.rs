//! Settings > Audit trail routes (policy save + export).

use crate::installer::AppState;
use crate::panel_admin::is_panel_admin;
use crate::panel_audit::apply_retention_now;
use crate::panel_audit_export::{export_csv, export_jsonl};
use crate::panel_audit_policy::{
    AuditPolicy, load_audit_policy, opt_outable_categories, save_audit_policy,
};
use crate::panel_hub_http::{
    html_ok, login_redirect, owner_only_html, redirect_flash, redirect_notice, require_panel_user,
};
use crate::panel_hub_pages_audit::audit_settings_page;
use crate::panel_pages::panel_shell;
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use std::collections::HashMap;
use std::sync::Arc;

#[get("/settings/audit")]
pub async fn settings_audit_page(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !is_panel_admin(&user) {
        return owner_only_html(&user, "settings", "Audit trail");
    }
    html_ok(panel_shell(
        &user,
        "settings",
        "Audit trail",
        &audit_settings_page(
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}

#[derive(Debug, serde::Deserialize)]
pub struct AuditPolicyForm {
    #[serde(default)]
    retention_days: String,
    #[serde(default)]
    webhook_url: String,
    #[serde(default)]
    alerts_enabled: String,
    #[serde(flatten)]
    extra: HashMap<String, String>,
}

#[post("/settings/audit")]
pub async fn settings_audit_save(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<AuditPolicyForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !is_panel_admin(&user) {
        return redirect_flash("/settings", None, Some("Admin only"));
    }
    let days = form
        .retention_days
        .trim()
        .parse::<u32>()
        .unwrap_or(crate::panel_audit_policy::DEFAULT_AUDIT_RETENTION_DAYS);
    let mut disabled = Vec::new();
    for cat in opt_outable_categories() {
        let key = format!("disable_{}", cat.as_str());
        if form.extra.get(&key).map(String::as_str) == Some("1") {
            disabled.push(cat.as_str().to_string());
        }
    }
    let prefs = AuditPolicy {
        retention_days: days,
        webhook_url: form.webhook_url.trim().to_string(),
        disabled_categories: disabled,
        alerts_enabled: form.alerts_enabled.trim() == "1",
    };
    match save_audit_policy(&prefs) {
        Ok(()) => {
            let _ = apply_retention_now();
            redirect_notice("/settings/audit", Some("Audit policy saved"), None)
        }
        Err(error) => redirect_notice("/settings/audit", None, Some(&error)),
    }
}

#[get("/settings/audit/export")]
pub async fn settings_audit_export(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !is_panel_admin(&user) {
        return redirect_flash("/settings", None, Some("Admin only"));
    }
    let format = query
        .get("format")
        .map(|s| s.as_str())
        .unwrap_or("jsonl")
        .to_ascii_lowercase();
    let category = query.get("category").map(String::as_str);
    let limit = query
        .get("limit")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(5000)
        .clamp(1, 50_000);
    let _ = load_audit_policy();
    if format == "csv" {
        let body = export_csv(limit, None, category);
        return HttpResponse::Ok()
            .content_type("text/csv; charset=utf-8")
            .insert_header((
                "Content-Disposition",
                "attachment; filename=\"cpn-audit.csv\"",
            ))
            .body(body);
    }
    let body = export_jsonl(limit, None, category);
    HttpResponse::Ok()
        .content_type("application/x-ndjson; charset=utf-8")
        .insert_header((
            "Content-Disposition",
            "attachment; filename=\"cpn-audit.jsonl\"",
        ))
        .body(body)
}
