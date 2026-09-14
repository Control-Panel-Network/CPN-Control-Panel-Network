//! Shared admin + CSRF gate for Email tool POSTs.

use crate::panel_admin::is_panel_admin;
use crate::panel_hub_http::{redirect_flash, redirect_notice};
use crate::panel_ops_email_csrf::verify_email_csrf;
use crate::panel_site_tools_security::same_origin_ok;
use actix_web::{HttpRequest, HttpResponse};
use std::collections::HashMap;

fn redirect_tool(path: &str, notice: Option<&str>, error: Option<&str>) -> HttpResponse {
    // Prefer flash cookies for Change Password so the URL stays clean.
    if path == "/email/password" {
        redirect_flash(path, notice, error)
    } else {
        redirect_notice(path, notice, error)
    }
}

pub fn require_email_admin_csrf(
    http: &HttpRequest,
    user: &str,
    form: &HashMap<String, String>,
    redirect: &str,
) -> Option<HttpResponse> {
    if !is_panel_admin(user) {
        return Some(redirect_tool(
            redirect,
            None,
            Some("Only the panel admin can change email tools"),
        ));
    }
    if !same_origin_ok(http) {
        return Some(redirect_tool(
            redirect,
            None,
            Some("Rejected cross-origin form post"),
        ));
    }
    let csrf = form.get("csrf").map(String::as_str).unwrap_or("");
    if !verify_email_csrf(user, csrf) {
        return Some(redirect_tool(
            redirect,
            None,
            Some("Invalid or expired CSRF token"),
        ));
    }
    None
}
