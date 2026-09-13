//! HTTP routes for site git, clone/staging, and terminal WebSocket.

use crate::installer::AppState;
use crate::panel_hub_http::{redirect_notice, require_panel_user, urlencoding_simple};
use crate::panel_site_clone::clone_site_files;
use crate::panel_site_git::{GitAction, run_git_action};
use crate::panel_site_tools_security::{
    check_tools_rate_limit, same_origin_ok, site_tools_csrf_token, verify_site_tools_csrf,
};
use crate::site_acl::{SitePerm, require_manage_site};
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use std::sync::Arc;

pub use crate::panel_site_terminal::websites_terminal_ws;

fn manage_redirect(
    domain: &str,
    tab: &str,
    notice: Option<&str>,
    error: Option<&str>,
) -> HttpResponse {
    let base = format!(
        "/websites/manage?domain={}&tab={}",
        urlencoding_simple(domain),
        urlencoding_simple(tab)
    );
    redirect_notice(&base, notice, error)
}

#[derive(Debug, serde::Deserialize)]
pub struct GitForm {
    #[serde(default)]
    domain: String,
    #[serde(default)]
    csrf: String,
    #[serde(default)]
    action: String,
    #[serde(default)]
    message: String,
    #[serde(default)]
    remote_url: String,
}

#[post("/websites/git")]
pub async fn websites_git_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<GitForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return crate::panel_hub_http::login_redirect(&http);
    };
    if !same_origin_ok(&http) {
        return manage_redirect(&form.domain, "git", None, Some("Same-origin check failed"));
    }
    if !verify_site_tools_csrf(&user, &form.domain, &form.csrf) {
        return manage_redirect(&form.domain, "git", None, Some("Invalid CSRF token"));
    }
    if let Err(err) = check_tools_rate_limit(&user) {
        return manage_redirect(&form.domain, "git", None, Some(&err));
    }
    let site = match require_manage_site(&user, &form.domain, SitePerm::Enable) {
        Ok(s) => s,
        Err(err) => {
            return HttpResponse::SeeOther()
                .append_header((
                    "Location",
                    format!("/websites?error={}", urlencoding_simple(&err)),
                ))
                .finish();
        }
    };
    let action = match GitAction::parse(&form.action) {
        Ok(a) => a,
        Err(err) => return manage_redirect(&site.domain, "git", None, Some(&err)),
    };
    match run_git_action(
        &site,
        action,
        Some(form.message.as_str()),
        Some(form.remote_url.as_str()),
    ) {
        Ok(out) => {
            let notice = if out.len() > 400 {
                format!("Git {}: {}", form.action.trim(), &out[..400])
            } else {
                format!("Git {}: {out}", form.action.trim())
            };
            manage_redirect(&site.domain, "git", Some(&notice), None)
        }
        Err(err) => manage_redirect(&site.domain, "git", None, Some(&err)),
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct CloneForm {
    #[serde(default)]
    domain: String,
    #[serde(default)]
    csrf: String,
    #[serde(default)]
    target: String,
    #[serde(default)]
    staging: String,
}

#[post("/websites/clone")]
pub async fn websites_clone_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<CloneForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return crate::panel_hub_http::login_redirect(&http);
    };
    if !same_origin_ok(&http) {
        return manage_redirect(
            &form.domain,
            "clone",
            None,
            Some("Same-origin check failed"),
        );
    }
    if !verify_site_tools_csrf(&user, &form.domain, &form.csrf) {
        return manage_redirect(&form.domain, "clone", None, Some("Invalid CSRF token"));
    }
    if let Err(err) = check_tools_rate_limit(&user) {
        return manage_redirect(&form.domain, "clone", None, Some(&err));
    }
    let site = match require_manage_site(&user, &form.domain, SitePerm::Install) {
        Ok(s) => s,
        Err(err) => {
            return HttpResponse::SeeOther()
                .append_header((
                    "Location",
                    format!("/websites?error={}", urlencoding_simple(&err)),
                ))
                .finish();
        }
    };
    let use_staging = matches!(
        form.staging.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "on" | "yes"
    );
    match clone_site_files(&site, &user, &form.target, use_staging) {
        Ok(result) => {
            let notice = format!(
                "Cloned files to {} ({} files). {}",
                result.domain, result.files_copied, result.note
            );
            manage_redirect(&result.domain, "overview", Some(&notice), None)
        }
        Err(err) => manage_redirect(&site.domain, "clone", None, Some(&err)),
    }
}

/// Expose CSRF mint for templates (git/clone tabs).
#[get("/api/websites/tools/csrf")]
pub async fn websites_tools_csrf_get(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return HttpResponse::Unauthorized().finish();
    };
    let domain = query.get("domain").map(String::as_str).unwrap_or("");
    if require_manage_site(&user, domain, SitePerm::Enable).is_err() {
        return HttpResponse::Forbidden().finish();
    }
    let token = site_tools_csrf_token(&user, domain);
    HttpResponse::Ok()
        .content_type("application/json; charset=utf-8")
        .body(
            serde_json::to_string_pretty(&serde_json::json!({
                "ok": true,
                "csrf": token,
                "domain": domain,
            }))
            .unwrap_or_else(|_| "{\"ok\":false}".into()),
        )
}
