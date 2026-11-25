//! Small HTTP helpers for recording audit events from Actix handlers.

use crate::panel_audit::record_full;
use crate::panel_audit_types::{AuditCategory, AuditSeverity};
use actix_web::HttpRequest;
use actix_web::http::header::USER_AGENT;

pub fn request_ip(http: &HttpRequest) -> String {
    http.headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| http.peer_addr().map(|a| a.ip().to_string()))
        .unwrap_or_default()
}

pub fn request_ua(http: &HttpRequest) -> String {
    http.headers()
        .get(USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .chars()
        .take(200)
        .collect()
}

/// Record an auth (or other) audit row with IP/UA from the request. Never panics.
#[allow(clippy::too_many_arguments)]
pub fn audit_from_request(
    http: &HttpRequest,
    actor: &str,
    action: &str,
    target: &str,
    ok: bool,
    message: &str,
    category: AuditCategory,
    severity: AuditSeverity,
) {
    let ip = request_ip(http);
    let ua = request_ua(http);
    if let Err(error) = record_full(
        actor,
        action,
        target,
        ok,
        message,
        &ip,
        &ua,
        "",
        Some(category),
        Some(severity),
    ) {
        eprintln!("panel-audit: {action}: {error}");
    }
}
