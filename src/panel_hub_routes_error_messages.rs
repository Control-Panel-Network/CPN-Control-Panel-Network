//! Routes for owner-editable panel error messages.

use crate::installer::AppState;
use crate::panel_admin::is_panel_admin;
use crate::panel_error_messages::{
    PanelErrorMessages, load_messages, restore_all_builtins, restore_forbidden, restore_internal,
    restore_not_found, save_messages,
};
use crate::panel_hub_http::{html_ok, login_redirect, redirect_notice, require_panel_user};
use crate::panel_hub_pages_error_messages::error_messages_settings_page;
use crate::panel_markdown::render_safe_markdown;
use crate::panel_pages::panel_shell;
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use std::sync::Arc;

#[get("/settings/error-messages")]
pub async fn settings_error_messages_page(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !is_panel_admin(&user) {
        return HttpResponse::SeeOther()
            .append_header(("Location", "/settings?error=Owner%20only"))
            .finish();
    }
    html_ok(panel_shell(
        &user,
        "settings",
        "Error messages",
        &error_messages_settings_page(
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}

#[derive(Debug, serde::Deserialize)]
pub struct ErrorMessagesForm {
    #[serde(default)]
    forbidden_md: String,
    #[serde(default)]
    not_found_md: String,
    #[serde(default)]
    internal_md: String,
}

#[post("/settings/error-messages")]
pub async fn settings_error_messages_save(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<ErrorMessagesForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !is_panel_admin(&user) {
        return HttpResponse::SeeOther()
            .append_header(("Location", "/settings?error=Owner%20only"))
            .finish();
    }
    let mut messages = load_messages();
    messages.forbidden_md = form.forbidden_md.clone();
    messages.not_found_md = form.not_found_md.clone();
    messages.internal_md = form.internal_md.clone();
    match save_messages(&messages) {
        Ok(()) => redirect_notice(
            "/settings/error-messages",
            Some("Error messages saved"),
            None,
        ),
        Err(error) => redirect_notice("/settings/error-messages", None, Some(&error)),
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct MarkdownPreviewForm {
    #[serde(default)]
    markdown: String,
}

#[post("/settings/error-messages/preview")]
pub async fn settings_error_messages_preview(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<MarkdownPreviewForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return HttpResponse::Unauthorized().finish();
    };
    if !is_panel_admin(&user) {
        return HttpResponse::Forbidden().finish();
    }
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(render_safe_markdown(&form.markdown))
}

#[post("/settings/error-messages/restore-forbidden")]
pub async fn settings_error_messages_restore_forbidden(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    restore_one(&http, &state, restore_forbidden, "403 default restored")
}

#[post("/settings/error-messages/restore-not-found")]
pub async fn settings_error_messages_restore_not_found(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    restore_one(&http, &state, restore_not_found, "404 default restored")
}

#[post("/settings/error-messages/restore-internal")]
pub async fn settings_error_messages_restore_internal(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    restore_one(&http, &state, restore_internal, "500 default restored")
}

#[post("/settings/error-messages/restore-all")]
pub async fn settings_error_messages_restore_all(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    restore_one(
        &http,
        &state,
        restore_all_builtins,
        "All error defaults restored",
    )
}

fn restore_one(
    http: &HttpRequest,
    state: &web::Data<Arc<AppState>>,
    op: fn() -> Result<(), String>,
    ok: &str,
) -> HttpResponse {
    let Some(user) = require_panel_user(state, http) else {
        return login_redirect(http);
    };
    if !is_panel_admin(&user) {
        return HttpResponse::SeeOther()
            .append_header(("Location", "/settings?error=Owner%20only"))
            .finish();
    }
    match op() {
        Ok(()) => redirect_notice("/settings/error-messages", Some(ok), None),
        Err(error) => redirect_notice("/settings/error-messages", None, Some(&error)),
    }
}

#[allow(dead_code)]
fn default_messages_for_tests() -> PanelErrorMessages {
    PanelErrorMessages::default()
}
