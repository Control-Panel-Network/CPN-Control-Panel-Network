//! Dashboard Activity Board POST routes (SSH security review snooze).

use crate::installer::AppState;
use crate::panel_admin::is_panel_admin;
use crate::panel_hub_http::{login_redirect, redirect, require_panel_user};
use crate::panel_user_prefs::{
    SSH_SECURITY_REVIEW_SNOOZE_DEFAULT_DAYS, clear_ssh_security_review_snooze,
    snooze_ssh_security_review,
};
use actix_web::{HttpRequest, HttpResponse, post, web};
use std::sync::Arc;

const DASH_SSH_LOGS: &str = "/dashboard?activity=ssh-logs";

#[derive(Debug, serde::Deserialize)]
pub struct SshReviewSnoozeForm {
    #[serde(default)]
    days: Option<u32>,
}

fn require_admin_user(state: &AppState, http: &HttpRequest) -> Result<String, HttpResponse> {
    let Some(user) = require_panel_user(state, http) else {
        return Err(login_redirect(http));
    };
    if !is_panel_admin(&user) {
        return Err(redirect(DASH_SSH_LOGS));
    }
    Ok(user)
}

#[post("/dashboard/activity/ssh-security-review/snooze")]
pub async fn dashboard_ssh_security_review_snooze(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<SshReviewSnoozeForm>,
) -> HttpResponse {
    let user = match require_admin_user(&state, &http) {
        Ok(u) => u,
        Err(resp) => return resp,
    };
    let days = form
        .days
        .unwrap_or(SSH_SECURITY_REVIEW_SNOOZE_DEFAULT_DAYS);
    let _ = snooze_ssh_security_review(&user, days);
    redirect(DASH_SSH_LOGS)
}

#[post("/dashboard/activity/ssh-security-review/show")]
pub async fn dashboard_ssh_security_review_show(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let user = match require_admin_user(&state, &http) {
        Ok(u) => u,
        Err(resp) => return resp,
    };
    let _ = clear_ssh_security_review_snooze(&user);
    redirect(DASH_SSH_LOGS)
}
