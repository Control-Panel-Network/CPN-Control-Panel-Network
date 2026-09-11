//! Email hub feature routes.

use crate::installer::AppState;
use crate::panel_hub_http::{html_ok, login_redirect, redirect_notice, require_panel_user};
use crate::panel_hub_pages_email_auth::{
    email_bimi_page, email_mta_sts_page, push_bimi_cloudflare, push_mta_sts_cloudflare,
    save_bimi_form, save_mta_sts_form,
};
use crate::panel_hub_pages_hosting::{
    add_catchall, add_forward, email_accounts_page, email_catchall_page,
    email_create_redirect_hint, email_delivery_page, email_dkim_page, email_forwarding_page,
    ensure_dkim,
};
use crate::panel_hub_pages_webmail::{
    apply_regenerate_path, apply_webmail_settings_form, email_webmail_app_page, email_webmail_page,
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
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    html_ok(panel_shell(
        &user,
        "email",
        "Webmail",
        &email_webmail_page(
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}

#[get("/email/webmail/app")]
pub async fn email_webmail_app_route(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    html_ok(panel_shell(
        &user,
        "email",
        "Internal Webmail",
        &email_webmail_app_page(),
    ))
}

#[derive(Debug, serde::Deserialize)]
pub struct WebmailSettingsForm {
    #[serde(default)]
    auto_login_account: String,
    #[serde(default)]
    public_path: String,
    #[serde(default)]
    internal_embed: Option<String>,
}

#[post("/email/webmail/settings")]
pub async fn email_webmail_settings_save(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<WebmailSettingsForm>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    let embed = form.internal_embed.as_deref() == Some("1");
    match apply_webmail_settings_form(&form.auto_login_account, &form.public_path, embed) {
        Ok(msg) => redirect_notice("/email/webmail", Some(&msg), None),
        Err(err) => redirect_notice("/email/webmail", None, Some(&err)),
    }
}

#[post("/email/webmail/regenerate-path")]
pub async fn email_webmail_regenerate_path(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    match apply_regenerate_path() {
        Ok(msg) => redirect_notice("/email/webmail", Some(&msg), None),
        Err(err) => redirect_notice("/email/webmail", None, Some(&err)),
    }
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

#[get("/email/mta-sts")]
pub async fn email_mta_sts_route(
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
        "MTA-STS",
        &email_mta_sts_page(
            query.get("domain").map(String::as_str).unwrap_or(""),
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}

#[derive(Debug, serde::Deserialize)]
pub struct MtaStsForm {
    #[serde(default)]
    domain: String,
    #[serde(default)]
    enabled: Option<String>,
    #[serde(default)]
    mode: String,
    #[serde(default)]
    max_age: String,
    #[serde(default)]
    mx: String,
}

#[post("/email/mta-sts/save")]
pub async fn email_mta_sts_save(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<MtaStsForm>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    let enabled = form.enabled.as_deref() == Some("1");
    match save_mta_sts_form(&form.domain, enabled, &form.mode, &form.max_age, &form.mx) {
        Ok(msg) => redirect_notice(
            &format!("/email/mta-sts?domain={}", urlencoding_simple(&form.domain)),
            Some(&msg),
            None,
        ),
        Err(err) => redirect_notice(
            &format!("/email/mta-sts?domain={}", urlencoding_simple(&form.domain)),
            None,
            Some(&err),
        ),
    }
}

#[post("/email/mta-sts/push-cloudflare")]
pub async fn email_mta_sts_push_cf(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    let domain = form.get("domain").map(String::as_str).unwrap_or("");
    match push_mta_sts_cloudflare(domain) {
        Ok(msg) => redirect_notice(
            &format!("/email/mta-sts?domain={}", urlencoding_simple(domain)),
            Some(&msg),
            None,
        ),
        Err(err) => redirect_notice(
            &format!("/email/mta-sts?domain={}", urlencoding_simple(domain)),
            None,
            Some(&err),
        ),
    }
}

#[get("/email/bimi")]
pub async fn email_bimi_route(
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
        "BIMI",
        &email_bimi_page(
            query.get("domain").map(String::as_str).unwrap_or(""),
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}

#[derive(Debug, serde::Deserialize)]
pub struct BimiForm {
    #[serde(default)]
    domain: String,
    #[serde(default)]
    enabled: Option<String>,
    #[serde(default)]
    logo_svg_url: String,
    #[serde(default)]
    authority_url: String,
}

#[post("/email/bimi/save")]
pub async fn email_bimi_save(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<BimiForm>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    let enabled = form.enabled.as_deref() == Some("1");
    match save_bimi_form(
        &form.domain,
        enabled,
        &form.logo_svg_url,
        &form.authority_url,
    ) {
        Ok(msg) => redirect_notice(
            &format!("/email/bimi?domain={}", urlencoding_simple(&form.domain)),
            Some(&msg),
            None,
        ),
        Err(err) => redirect_notice(
            &format!("/email/bimi?domain={}", urlencoding_simple(&form.domain)),
            None,
            Some(&err),
        ),
    }
}

#[post("/email/bimi/push-cloudflare")]
pub async fn email_bimi_push_cf(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    let domain = form.get("domain").map(String::as_str).unwrap_or("");
    match push_bimi_cloudflare(domain) {
        Ok(msg) => redirect_notice(
            &format!("/email/bimi?domain={}", urlencoding_simple(domain)),
            Some(&msg),
            None,
        ),
        Err(err) => redirect_notice(
            &format!("/email/bimi?domain={}", urlencoding_simple(domain)),
            None,
            Some(&err),
        ),
    }
}

fn urlencoding_simple(value: &str) -> String {
    crate::panel_hub_http::urlencoding_simple(value)
}
