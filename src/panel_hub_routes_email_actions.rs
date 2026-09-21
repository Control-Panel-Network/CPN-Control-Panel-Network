//! POST actions for Email tools (admin ACL + CSRF).

use crate::installer::AppState;
use crate::panel_hub_http::{login_redirect, redirect_flash, redirect_notice, require_panel_user};
use crate::panel_ops_email_acl::require_email_admin_csrf;
use crate::panel_ops_email_antispam::{enable_mailscanner, enable_rspamd, enable_spamassassin};
use crate::panel_ops_email_limits::{add_send_limit, remove_send_limit};
use crate::panel_ops_email_marketing::{add_recipient, create_list, send_campaign};
use crate::panel_ops_email_password::reset_mailbox_password;
use crate::panel_ops_email_pattern::{add_pattern_rule, apply_pattern_maps, remove_pattern_rule};
use crate::panel_ops_email_plus::set_plus_addressing;
use crate::panel_ops_email_queue::{delete_all_deferred, delete_queue_id, flush_queue};
use actix_web::{HttpRequest, HttpResponse, post, web};
use std::collections::HashMap;
use std::sync::Arc;

#[post("/email/pattern-forwarding/save")]
pub async fn email_pattern_fwd_save(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Some(resp) = require_email_admin_csrf(&http, &user, &form, "/email/pattern-forwarding") {
        return resp;
    }
    let kind = form.get("kind").map(String::as_str).unwrap_or("glob");
    let pattern = form.get("pattern").map(String::as_str).unwrap_or("");
    let destination = form.get("destination").map(String::as_str).unwrap_or("");
    match add_pattern_rule(kind, pattern, destination) {
        Ok(msg) => redirect_notice("/email/pattern-forwarding", Some(&msg), None),
        Err(err) => redirect_notice("/email/pattern-forwarding", None, Some(&err)),
    }
}

#[post("/email/pattern-forwarding/delete")]
pub async fn email_pattern_fwd_delete(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Some(resp) = require_email_admin_csrf(&http, &user, &form, "/email/pattern-forwarding") {
        return resp;
    }
    let id = form.get("id").map(String::as_str).unwrap_or("");
    match remove_pattern_rule(id) {
        Ok(msg) => redirect_notice("/email/pattern-forwarding", Some(&msg), None),
        Err(err) => redirect_notice("/email/pattern-forwarding", None, Some(&err)),
    }
}

#[post("/email/pattern-forwarding/apply")]
pub async fn email_pattern_fwd_apply(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Some(resp) = require_email_admin_csrf(&http, &user, &form, "/email/pattern-forwarding") {
        return resp;
    }
    match apply_pattern_maps() {
        Ok(msg) => redirect_notice("/email/pattern-forwarding", Some(&msg), None),
        Err(err) => redirect_notice("/email/pattern-forwarding", None, Some(&err)),
    }
}

#[post("/email/limits/save")]
pub async fn email_limits_save(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Some(resp) = require_email_admin_csrf(&http, &user, &form, "/email/limits") {
        return resp;
    }
    let scope = form.get("scope").map(String::as_str).unwrap_or("");
    let max = form
        .get("max_messages")
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(0);
    let window = form
        .get("window_minutes")
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(0);
    let note = form.get("note").map(String::as_str).unwrap_or("");
    match add_send_limit(scope, max, window, note) {
        Ok(msg) => redirect_notice("/email/limits", Some(&msg), None),
        Err(err) => redirect_notice("/email/limits", None, Some(&err)),
    }
}

#[post("/email/limits/delete")]
pub async fn email_limits_delete(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Some(resp) = require_email_admin_csrf(&http, &user, &form, "/email/limits") {
        return resp;
    }
    let id = form.get("id").map(String::as_str).unwrap_or("");
    match remove_send_limit(id) {
        Ok(msg) => redirect_notice("/email/limits", Some(&msg), None),
        Err(err) => redirect_notice("/email/limits", None, Some(&err)),
    }
}

#[post("/email/password/save")]
pub async fn email_password_save(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Some(resp) = require_email_admin_csrf(&http, &user, &form, "/email/password") {
        return resp;
    }
    let mailbox = form.get("mailbox").map(String::as_str).unwrap_or_default();
    // Read form fields without hard-coded empty-string defaults flowing into
    // password sinks (CodeQL rust/hard-coded-cryptographic-value).
    let Some(new_secret) = form.get("password").map(String::as_str) else {
        return redirect_flash("/email/password", None, Some("Password is required"));
    };
    let Some(new_secret2) = form.get("password2").map(String::as_str) else {
        return redirect_flash(
            "/email/password",
            None,
            Some("Password confirmation is required"),
        );
    };
    if new_secret != new_secret2 {
        return redirect_flash("/email/password", None, Some("Passwords do not match"));
    }
    match reset_mailbox_password(mailbox, new_secret) {
        Ok(msg) => redirect_flash("/email/password", Some(&msg), None),
        Err(err) => redirect_flash("/email/password", None, Some(&err)),
    }
}

#[post("/email/queue/flush")]
pub async fn email_queue_flush(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Some(resp) = require_email_admin_csrf(&http, &user, &form, "/email/queue") {
        return resp;
    }
    match flush_queue() {
        Ok(msg) => redirect_notice("/email/queue", Some(&msg), None),
        Err(err) => redirect_notice("/email/queue", None, Some(&err)),
    }
}

#[post("/email/queue/delete")]
pub async fn email_queue_delete(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Some(resp) = require_email_admin_csrf(&http, &user, &form, "/email/queue") {
        return resp;
    }
    let qid = form.get("qid").map(String::as_str).unwrap_or("");
    match delete_queue_id(qid) {
        Ok(msg) => redirect_notice("/email/queue", Some(&msg), None),
        Err(err) => redirect_notice("/email/queue", None, Some(&err)),
    }
}

#[post("/email/queue/delete-all")]
pub async fn email_queue_delete_all(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Some(resp) = require_email_admin_csrf(&http, &user, &form, "/email/queue") {
        return resp;
    }
    match delete_all_deferred() {
        Ok(msg) => redirect_notice("/email/queue", Some(&msg), None),
        Err(err) => redirect_notice("/email/queue", None, Some(&err)),
    }
}

#[post("/email/spamassassin/enable")]
pub async fn email_spamassassin_enable(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Some(resp) = require_email_admin_csrf(&http, &user, &form, "/email/spamassassin") {
        return resp;
    }
    match enable_spamassassin() {
        Ok(msg) => redirect_notice("/email/spamassassin", Some(&msg), None),
        Err(err) => redirect_notice("/email/spamassassin", None, Some(&err)),
    }
}

#[post("/email/rspamd/enable")]
pub async fn email_rspamd_enable(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Some(resp) = require_email_admin_csrf(&http, &user, &form, "/email/rspamd") {
        return resp;
    }
    match enable_rspamd() {
        Ok(msg) => redirect_notice("/email/rspamd", Some(&msg), None),
        Err(err) => redirect_notice("/email/rspamd", None, Some(&err)),
    }
}

#[post("/email/mailscanner/enable")]
pub async fn email_mailscanner_enable(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Some(resp) = require_email_admin_csrf(&http, &user, &form, "/email/mailscanner") {
        return resp;
    }
    match enable_mailscanner() {
        Ok(msg) => redirect_notice("/email/mailscanner", Some(&msg), None),
        Err(err) => redirect_notice("/email/mailscanner", None, Some(&err)),
    }
}

#[post("/email/marketing/list")]
pub async fn email_marketing_list(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Some(resp) = require_email_admin_csrf(&http, &user, &form, "/email/marketing") {
        return resp;
    }
    let name = form.get("name").map(String::as_str).unwrap_or("");
    match create_list(name) {
        Ok(msg) => redirect_notice("/email/marketing", Some(&msg), None),
        Err(err) => redirect_notice("/email/marketing", None, Some(&err)),
    }
}

#[post("/email/marketing/recipient")]
pub async fn email_marketing_recipient(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Some(resp) = require_email_admin_csrf(&http, &user, &form, "/email/marketing") {
        return resp;
    }
    let list_id = form.get("list_id").map(String::as_str).unwrap_or("");
    let email = form.get("email").map(String::as_str).unwrap_or("");
    match add_recipient(list_id, email) {
        Ok(msg) => redirect_notice("/email/marketing", Some(&msg), None),
        Err(err) => redirect_notice("/email/marketing", None, Some(&err)),
    }
}

#[post("/email/marketing/send")]
pub async fn email_marketing_send(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Some(resp) = require_email_admin_csrf(&http, &user, &form, "/email/marketing") {
        return resp;
    }
    let list_id = form.get("list_id").map(String::as_str).unwrap_or("");
    let subject = form.get("subject").map(String::as_str).unwrap_or("");
    let body = form.get("body").map(String::as_str).unwrap_or("");
    let from_address = form.get("from_address").map(String::as_str).unwrap_or("");
    match send_campaign(list_id, subject, body, from_address) {
        Ok(msg) => redirect_notice("/email/marketing", Some(&msg), None),
        Err(err) => redirect_notice("/email/marketing", None, Some(&err)),
    }
}

#[post("/email/plus-addressing/save")]
pub async fn email_plus_save(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Some(resp) = require_email_admin_csrf(&http, &user, &form, "/email/plus-addressing") {
        return resp;
    }
    let enabled = form
        .get("enabled")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("on") || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let delimiter = form.get("delimiter").map(String::as_str).unwrap_or("+");
    match set_plus_addressing(enabled, delimiter) {
        Ok(msg) => redirect_notice("/email/plus-addressing", Some(&msg), None),
        Err(err) => redirect_notice("/email/plus-addressing", None, Some(&err)),
    }
}
