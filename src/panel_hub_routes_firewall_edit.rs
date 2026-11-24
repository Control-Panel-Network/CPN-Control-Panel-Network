//! Edit routes for the firewall manager: rules, banned IPs, and SSH trusted IPs.

use crate::installer::AppState;
use crate::panel_hub_http::{login_redirect, redirect_notice, require_panel_user};
use crate::panel_hub_routes_firewall::{require_admin_csrf, tab_base};
use crate::panel_ops_firewall_rules::{update_ban_live, update_rule_live, update_trusted_live};
use actix_web::{HttpRequest, HttpResponse, post, web};
use std::collections::HashMap;
use std::sync::Arc;

fn field<'a>(form: &'a HashMap<String, String>, key: &str) -> &'a str {
    form.get(key).map(String::as_str).unwrap_or("")
}

fn finish(tab: &str, result: Result<String, String>) -> HttpResponse {
    match result {
        Ok(msg) => redirect_notice(&tab_base(tab), Some(&msg), None),
        Err(err) => redirect_notice(&tab_base(tab), None, Some(&err)),
    }
}

#[post("/security/firewall/rules/update")]
pub async fn security_firewall_rule_update(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Some(resp) = require_admin_csrf(&http, &user, &form, "rules") {
        return resp;
    }
    let id = field(&form, "id").to_string();
    let name = field(&form, "name").to_string();
    let protocol = field(&form, "protocol").to_string();
    let port = field(&form, "port").to_string();
    let source = field(&form, "source").to_string();
    let result = tokio::task::spawn_blocking(move || {
        update_rule_live(&id, &name, &protocol, &port, &source)
    })
    .await
    .unwrap_or_else(|_| Err("Rule update failed unexpectedly".into()));
    finish("rules", result)
}

#[post("/security/firewall/banned/update")]
pub async fn security_firewall_ban_update(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Some(resp) = require_admin_csrf(&http, &user, &form, "banned") {
        return resp;
    }
    let old_ip = field(&form, "old_ip").to_string();
    let ip = field(&form, "ip").to_string();
    let reason = field(&form, "reason").to_string();
    let duration = field(&form, "duration").to_string();
    let result =
        tokio::task::spawn_blocking(move || update_ban_live(&old_ip, &ip, &reason, &duration))
            .await
            .unwrap_or_else(|_| Err("Ban update failed unexpectedly".into()));
    finish("banned", result)
}

#[post("/security/firewall/trusted/update")]
pub async fn security_firewall_trusted_update(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Some(resp) = require_admin_csrf(&http, &user, &form, "trusted") {
        return resp;
    }
    let result = update_trusted_live(
        field(&form, "old_ip"),
        field(&form, "ip"),
        field(&form, "label"),
    );
    finish("trusted", result)
}
