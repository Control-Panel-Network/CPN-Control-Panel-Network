//! GET routes for Email tools (formerly scaffolds).

use crate::installer::AppState;
use crate::panel_hub_http::{
    flash_messages, html_ok, html_ok_pop_flash, login_redirect, require_panel_user,
};
use crate::panel_hub_pages_email_deliver::{
    email_debugger_page, email_marketing_page, email_queue_page, mailscanner_page, rspamd_page,
    spamassassin_page,
};
use crate::panel_hub_pages_email_tools::{
    email_limits_page, email_password_page, pattern_forwarding_page, plus_addressing_page,
};
use crate::panel_pages::panel_shell;
use actix_web::{HttpRequest, HttpResponse, get, web};
use std::collections::HashMap;
use std::sync::Arc;

#[get("/email/pattern-forwarding")]
pub async fn email_pattern_fwd(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    html_ok(panel_shell(
        &user,
        "email",
        "Pattern Forwarding",
        &pattern_forwarding_page(
            &user,
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}

#[get("/email/limits")]
pub async fn email_limits(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    html_ok(panel_shell(
        &user,
        "email",
        "Email Limits",
        &email_limits_page(
            &user,
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}

#[get("/email/password")]
pub async fn email_password(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let (notice, error) = flash_messages(
        &http,
        query.get("notice").map(String::as_str),
        query.get("error").map(String::as_str),
    );
    html_ok_pop_flash(
        &http,
        panel_shell(
            &user,
            "email",
            "Change Password",
            &email_password_page(
                &user,
                notice.as_deref(),
                error.as_deref(),
            ),
        ),
    )
}

#[get("/email/debugger")]
pub async fn email_debugger(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    html_ok(panel_shell(
        &user,
        "email",
        "Email Debugger",
        &email_debugger_page(
            &user,
            query.get("domain").map(String::as_str),
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}

#[get("/email/queue")]
pub async fn email_queue(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    html_ok(panel_shell(
        &user,
        "email",
        "Mail Queue",
        &email_queue_page(
            &user,
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}

#[get("/email/spamassassin")]
pub async fn email_spamassassin(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    html_ok(panel_shell(
        &user,
        "email",
        "SpamAssassin",
        &spamassassin_page(
            &user,
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}

#[get("/email/rspamd")]
pub async fn email_rspamd(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    html_ok(panel_shell(
        &user,
        "email",
        "Rspamd",
        &rspamd_page(
            &user,
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}

#[get("/email/mailscanner")]
pub async fn email_mailscanner(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    html_ok(panel_shell(
        &user,
        "email",
        "MailScanner",
        &mailscanner_page(
            &user,
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}

#[get("/email/marketing")]
pub async fn email_marketing(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    html_ok(panel_shell(
        &user,
        "email",
        "Email Marketing",
        &email_marketing_page(
            &user,
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}

#[get("/email/plus-addressing")]
pub async fn email_plus(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    html_ok(panel_shell(
        &user,
        "email",
        "Plus-Addressing",
        &plus_addressing_page(
            &user,
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}
