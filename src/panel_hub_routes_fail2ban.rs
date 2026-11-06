//! Fail2Ban list/unban API and page wiring (issue #627).

use crate::installer::AppState;
use crate::panel_action_log;
use crate::panel_admin::is_panel_admin;
use crate::panel_hub_http::{html_ok, login_redirect, require_panel_user};
use crate::panel_hub_pages_fail2ban::fail2ban_manager_page;
use crate::panel_ops_fail2ban::{list_banned, unban_ip, verify_fail2ban_csrf};
use crate::panel_pages::panel_shell;
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;

fn same_origin_ok(http: &HttpRequest) -> bool {
    let host = http
        .headers()
        .get("host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if host.is_empty() {
        return true;
    }
    if let Some(origin) = http.headers().get("origin").and_then(|v| v.to_str().ok()) {
        return origin.contains(host);
    }
    if let Some(referer) = http.headers().get("referer").and_then(|v| v.to_str().ok()) {
        return referer.contains(host);
    }
    true
}

fn json_err(status: actix_web::http::StatusCode, msg: &str) -> HttpResponse {
    HttpResponse::build(status)
        .content_type("application/json; charset=utf-8")
        .body(serde_json::json!({ "ok": false, "error": msg }).to_string())
}

#[get("/security/fail2ban")]
pub async fn security_fail2ban(http: HttpRequest, state: web::Data<Arc<AppState>>) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    html_ok(panel_shell(
        &user,
        "security",
        "Fail2ban",
        &fail2ban_manager_page(&user, is_panel_admin(&user)),
    ))
}

#[derive(Debug, Deserialize)]
pub struct BansQuery {
    #[serde(default)]
    jail: String,
}

#[get("/api/security/fail2ban/bans")]
pub async fn security_fail2ban_bans(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<BansQuery>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return json_err(
            actix_web::http::StatusCode::UNAUTHORIZED,
            "Sign in required",
        );
    };
    let jail_owned = query.jail.trim().to_string();
    let filter = if jail_owned.is_empty() {
        None
    } else {
        Some(jail_owned)
    };
    let inv = web::block(move || list_banned(filter.as_deref()))
        .await
        .unwrap_or_else(|_| list_banned(None));
    HttpResponse::Ok()
        .content_type("application/json; charset=utf-8")
        .body(
            serde_json::to_string(&serde_json::json!({
                "ok": true,
                "installed": inv.installed,
                "active": inv.active,
                "enabled": inv.enabled,
                "available": inv.available,
                "detail": inv.detail,
                "jails": inv.jails,
                "bans": inv.bans,
            }))
            .unwrap_or_else(|_| r#"{"ok":false,"error":"encode failed"}"#.into()),
        )
}

#[post("/api/security/fail2ban/unban")]
pub async fn security_fail2ban_unban(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return json_err(
            actix_web::http::StatusCode::UNAUTHORIZED,
            "Sign in required",
        );
    };
    if !is_panel_admin(&user) {
        let _ = panel_action_log::record(
            &user,
            "security.fail2ban.unban",
            "",
            false,
            "Denied: admin only",
        );
        return json_err(
            actix_web::http::StatusCode::FORBIDDEN,
            "Only the panel admin can unban IPs",
        );
    }
    if !same_origin_ok(&http) {
        return json_err(
            actix_web::http::StatusCode::FORBIDDEN,
            "Rejected cross-origin form post",
        );
    }
    let csrf = form.get("csrf").map(String::as_str).unwrap_or("");
    if !verify_fail2ban_csrf(&user, csrf) {
        return json_err(
            actix_web::http::StatusCode::FORBIDDEN,
            "Invalid or expired CSRF token",
        );
    }
    let ip = form.get("ip").cloned().unwrap_or_default();
    let jail = form.get("jail").cloned().unwrap_or_default();
    let target = format!("{ip}@{jail}");
    let actor = user.clone();
    let ip_run = ip.clone();
    let jail_run = jail.clone();
    let result = web::block(move || unban_ip(&jail_run, &ip_run))
        .await
        .unwrap_or_else(|e| Err(format!("Unban task failed: {e}")));
    match result {
        Ok(msg) => {
            let _ =
                panel_action_log::record(&actor, "security.fail2ban.unban", &target, true, &msg);
            HttpResponse::Ok()
                .content_type("application/json; charset=utf-8")
                .body(serde_json::json!({ "ok": true, "message": msg }).to_string())
        }
        Err(err) => {
            let _ =
                panel_action_log::record(&actor, "security.fail2ban.unban", &target, false, &err);
            json_err(actix_web::http::StatusCode::BAD_REQUEST, &err)
        }
    }
}
