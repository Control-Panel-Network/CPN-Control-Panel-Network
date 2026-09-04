//! Security hub routes.

use crate::installer::AppState;
use crate::panel_admin::is_panel_admin;
use crate::panel_hub_http::{html_ok, login_redirect, redirect_notice, require_panel_user};
use crate::panel_hub_pages_security::{
    fail2ban_page, firewall_page, hostname_ssl_page, mail_ssl_page, malware_scan_page,
    manage_ssl_page, modsec_page, modsec_rules_page, rule_packs_page, run_sshd_toggle,
    secure_ssh_page, security_hub_main,
};
use crate::panel_pages::panel_shell;
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use std::sync::Arc;

#[get("/security")]
pub async fn security_page(http: HttpRequest, state: web::Data<Arc<AppState>>) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    html_ok(panel_shell(
        &user,
        "security",
        "Security",
        &security_hub_main(),
    ))
}

#[get("/security/firewall")]
pub async fn security_firewall(http: HttpRequest, state: web::Data<Arc<AppState>>) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    html_ok(panel_shell(&user, "security", "Firewall", &firewall_page()))
}

#[get("/security/ssh")]
pub async fn security_ssh(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    html_ok(panel_shell(
        &user,
        "security",
        "Secure SSH",
        &secure_ssh_page(
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
            is_panel_admin(&user),
        ),
    ))
}

#[derive(Debug, serde::Deserialize)]
pub struct SshToggleForm {
    #[serde(default)]
    key: String,
    #[serde(default)]
    value: String,
}

#[post("/security/ssh/toggle")]
pub async fn security_ssh_toggle(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<SshToggleForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    match run_sshd_toggle(&user, form.key.trim(), form.value.trim()) {
        Ok(msg) => redirect_notice("/security/ssh", Some(&msg), None),
        Err(err) => redirect_notice("/security/ssh", None, Some(&err)),
    }
}

#[get("/security/fail2ban")]
pub async fn security_fail2ban(http: HttpRequest, state: web::Data<Arc<AppState>>) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    html_ok(panel_shell(&user, "security", "Fail2ban", &fail2ban_page()))
}

#[get("/security/modsecurity")]
pub async fn security_modsec(http: HttpRequest, state: web::Data<Arc<AppState>>) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    html_ok(panel_shell(
        &user,
        "security",
        "ModSecurity",
        &modsec_page(),
    ))
}

#[get("/security/modsec-rules")]
pub async fn security_modsec_rules(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    html_ok(panel_shell(
        &user,
        "security",
        "ModSec Rules",
        &modsec_rules_page(),
    ))
}

#[get("/security/rule-packs")]
pub async fn security_rule_packs(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    html_ok(panel_shell(
        &user,
        "security",
        "Rule Packs",
        &rule_packs_page(),
    ))
}

#[get("/security/malware-scan")]
pub async fn security_malware(http: HttpRequest, state: web::Data<Arc<AppState>>) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    html_ok(panel_shell(
        &user,
        "security",
        "Malware scan",
        &malware_scan_page(),
    ))
}

#[get("/security/ssl")]
pub async fn security_ssl(http: HttpRequest, state: web::Data<Arc<AppState>>) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    html_ok(panel_shell(
        &user,
        "security",
        "Manage SSL",
        &manage_ssl_page(),
    ))
}

#[get("/security/ssl/hostname")]
pub async fn security_ssl_hostname(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    html_ok(panel_shell(
        &user,
        "security",
        "Hostname SSL",
        &hostname_ssl_page(),
    ))
}

#[get("/security/ssl/mail")]
pub async fn security_ssl_mail(http: HttpRequest, state: web::Data<Arc<AppState>>) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    html_ok(panel_shell(
        &user,
        "security",
        "Mail Server SSL",
        &mail_ssl_page(),
    ))
}
