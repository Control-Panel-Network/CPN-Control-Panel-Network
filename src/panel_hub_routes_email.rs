//! Email hub feature routes.

use crate::installer::AppState;
use crate::panel_hub_http::{html_ok, login_redirect, redirect_notice, require_panel_user};
use crate::panel_hub_pages_hosting::{
    add_catchall, add_forward, email_accounts_page, email_catchall_page,
    email_create_redirect_hint, email_delivery_page, email_dkim_page, email_forwarding_page,
    email_webmail_page, ensure_dkim, scaffold_feature,
};
use crate::panel_pages::panel_shell;
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use std::sync::Arc;

#[get("/email/accounts")]
pub async fn email_accounts_route(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    let status = state
        .status
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    html_ok(panel_shell(
        &user,
        "email",
        "Email Accounts",
        &email_accounts_page(
            status.selected_mail,
            status.mail_client_ready,
            status.mail_backend_ready,
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}

#[get("/email/create")]
pub async fn email_create_route(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    html_ok(panel_shell(
        &user,
        "email",
        "Create Email",
        &email_create_redirect_hint(),
    ))
}

#[get("/email/forwarding")]
pub async fn email_forwarding_route(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    html_ok(panel_shell(
        &user,
        "email",
        "Forwarding",
        &email_forwarding_page(
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}

#[derive(Debug, serde::Deserialize)]
pub struct ForwardForm {
    #[serde(default)]
    from: String,
    #[serde(default)]
    to: String,
}

#[post("/email/forwarding/save")]
pub async fn email_forwarding_save(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<ForwardForm>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    match add_forward(&form.from, &form.to) {
        Ok(msg) => redirect_notice("/email/forwarding", Some(&msg), None),
        Err(err) => redirect_notice("/email/forwarding", None, Some(&err)),
    }
}

#[get("/email/catchall")]
pub async fn email_catchall_route(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    html_ok(panel_shell(
        &user,
        "email",
        "Catch-All",
        &email_catchall_page(
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}

#[derive(Debug, serde::Deserialize)]
pub struct CatchAllForm {
    #[serde(default)]
    domain: String,
    #[serde(default)]
    target: String,
}

#[post("/email/catchall/save")]
pub async fn email_catchall_save(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<CatchAllForm>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    match add_catchall(&form.domain, &form.target) {
        Ok(msg) => redirect_notice("/email/catchall", Some(&msg), None),
        Err(err) => redirect_notice("/email/catchall", None, Some(&err)),
    }
}

#[get("/email/dkim")]
pub async fn email_dkim_route(http: HttpRequest, state: web::Data<Arc<AppState>>) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    html_ok(panel_shell(
        &user,
        "email",
        "DKIM Manager",
        &email_dkim_page(),
    ))
}

#[post("/email/dkim/ensure")]
pub async fn email_dkim_ensure(http: HttpRequest, state: web::Data<Arc<AppState>>) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    match ensure_dkim() {
        Ok(msg) => redirect_notice("/email/dkim", Some(&msg), None),
        Err(err) => redirect_notice("/email/dkim", None, Some(&err)),
    }
}

#[get("/email/webmail")]
pub async fn email_webmail_route(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    let status = state
        .status
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    html_ok(panel_shell(
        &user,
        "email",
        "Webmail",
        &email_webmail_page(status.selected_mail, status.mail_client_ready),
    ))
}

#[get("/email/delivery")]
pub async fn email_delivery_route(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    html_ok(panel_shell(
        &user,
        "email",
        "Email Delivery",
        &email_delivery_page(),
    ))
}

macro_rules! email_scaffold {
    ($name:ident, $path:literal, $title:literal, $sub:literal, $detail:literal) => {
        #[get($path)]
        pub async fn $name(http: HttpRequest, state: web::Data<Arc<AppState>>) -> HttpResponse {
            let Some(user) = require_panel_user(&state, &http) else {
                return login_redirect();
            };
            html_ok(panel_shell(
                &user,
                "email",
                $title,
                &scaffold_feature("Email", "/email", $title, $sub, $detail),
            ))
        }
    };
}

email_scaffold!(
    email_pattern_fwd,
    "/email/pattern-forwarding",
    "Pattern Forwarding",
    "Rule-based forwarding",
    "Pattern rules are not wired yet."
);
email_scaffold!(
    email_limits,
    "/email/limits",
    "Email Limits",
    "Sending limits",
    "Per-mailbox send limits are not configured yet."
);
email_scaffold!(
    email_password,
    "/email/password",
    "Change Password",
    "Reset mailbox password",
    "Mailbox password reset UI is not wired yet."
);
email_scaffold!(
    email_debugger,
    "/email/debugger",
    "Email Debugger",
    "Diagnose mail issues",
    "Mail debugger is not configured yet."
);
email_scaffold!(
    email_queue,
    "/email/queue",
    "Mail Queue",
    "Inspect the queue",
    "Mail queue inspection is not configured yet."
);
email_scaffold!(
    email_spamassassin,
    "/email/spamassassin",
    "SpamAssassin",
    "Spam filtering",
    "SpamAssassin is not installed or not configured."
);
email_scaffold!(
    email_rspamd,
    "/email/rspamd",
    "Rspamd",
    "Spam filtering",
    "Rspamd is not installed or not configured."
);
email_scaffold!(
    email_mailscanner,
    "/email/mailscanner",
    "MailScanner",
    "Mail scanning",
    "MailScanner is not installed or not configured."
);
email_scaffold!(
    email_marketing,
    "/email/marketing",
    "Email Marketing",
    "Campaigns and lists",
    "Email marketing is not configured yet."
);
email_scaffold!(
    email_plus,
    "/email/plus-addressing",
    "Plus-Addressing",
    "user+tag addressing",
    "Plus-addressing controls are not configured yet."
);
