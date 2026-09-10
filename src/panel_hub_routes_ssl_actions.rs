//! Let's Encrypt / SSL provider POST actions (GET /security/ssl is in security routes).

use crate::installer::AppState;
use crate::panel_admin::is_panel_admin;
use crate::panel_hub_http::{login_redirect, redirect_notice, require_panel_user};
use crate::panel_ops_ssl_le::{
    issue_le_for_all_without_custom, issue_lets_encrypt, renew_lets_encrypt_all,
    restore_lets_encrypt, set_coverage_mode, set_custom_ssl, set_domain_provider,
    set_include_subdomains, upload_custom_ssl,
};
use crate::panel_ops_ssl_provider::{SslCoverageMode, SslProvider, save_ssl_defaults};
use actix_web::{HttpRequest, HttpResponse, post, web};
use serde::Deserialize;
use std::sync::Arc;

fn admin_gate(user: &str, back: &str) -> Option<HttpResponse> {
    if is_panel_admin(user) {
        None
    } else {
        Some(redirect_notice(back, None, Some("Admin only")))
    }
}

#[derive(Debug, Deserialize)]
pub struct SslDomainForm {
    pub domain: String,
    #[serde(default)]
    pub r#return: Option<String>,
}

fn ssl_back(form_return: Option<&str>) -> String {
    match form_return.map(str::trim).filter(|s| !s.is_empty()) {
        Some(r) if r.starts_with("/websites/manage") || r.starts_with("/security/ssl") => {
            r.to_string()
        }
        _ => "/security/ssl".into(),
    }
}

#[post("/security/ssl/issue")]
pub async fn security_ssl_issue(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<SslDomainForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    let back = ssl_back(form.r#return.as_deref());
    if let Some(resp) = admin_gate(&user, &back) {
        return resp;
    }
    match issue_lets_encrypt(&form.domain) {
        Ok(msg) => redirect_notice(&back, Some(&msg), None),
        Err(err) => redirect_notice(&back, None, Some(&err)),
    }
}

#[post("/security/ssl/issue-all")]
pub async fn security_ssl_issue_all(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    if let Some(resp) = admin_gate(&user, "/security/ssl") {
        return resp;
    }
    match issue_le_for_all_without_custom() {
        Ok(msg) => redirect_notice("/security/ssl", Some(&msg), None),
        Err(err) => redirect_notice("/security/ssl", None, Some(&err)),
    }
}

#[post("/security/ssl/renew")]
pub async fn security_ssl_renew(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    if let Some(resp) = admin_gate(&user, "/security/ssl") {
        return resp;
    }
    match renew_lets_encrypt_all() {
        Ok(msg) => redirect_notice("/security/ssl", Some(&msg), None),
        Err(err) => redirect_notice("/security/ssl", None, Some(&err)),
    }
}

#[post("/security/ssl/restore-le")]
pub async fn security_ssl_restore_le(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<SslDomainForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    let back = ssl_back(form.r#return.as_deref());
    if let Some(resp) = admin_gate(&user, &back) {
        return resp;
    }
    match restore_lets_encrypt(&form.domain) {
        Ok(msg) => redirect_notice(&back, Some(&msg), None),
        Err(err) => redirect_notice(&back, None, Some(&err)),
    }
}

#[post("/security/ssl/mark-custom")]
pub async fn security_ssl_mark_custom(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<SslDomainForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    let back = ssl_back(form.r#return.as_deref());
    if let Some(resp) = admin_gate(&user, &back) {
        return resp;
    }
    match set_custom_ssl(&form.domain) {
        Ok(msg) => redirect_notice(&back, Some(&msg), None),
        Err(err) => redirect_notice(&back, None, Some(&err)),
    }
}

#[derive(Debug, Deserialize)]
pub struct SslProviderForm {
    pub domain: String,
    pub provider: String,
    #[serde(default)]
    pub coverage_mode: Option<String>,
    #[serde(default)]
    pub include_subdomains: Option<String>,
    #[serde(default)]
    pub r#return: Option<String>,
}

#[post("/security/ssl/provider")]
pub async fn security_ssl_provider(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<SslProviderForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    let back = ssl_back(form.r#return.as_deref());
    if let Some(resp) = admin_gate(&user, &back) {
        return resp;
    }
    let provider = match SslProvider::parse(&form.provider) {
        Ok(p) => p,
        Err(err) => return redirect_notice(&back, None, Some(&err)),
    };
    let coverage = if let Some(raw) = form.coverage_mode.as_deref() {
        match SslCoverageMode::parse(raw) {
            Ok(m) => m,
            Err(err) => return redirect_notice(&back, None, Some(&err)),
        }
    } else if form
        .include_subdomains
        .as_deref()
        .map(|v| v == "1" || v.eq_ignore_ascii_case("on"))
        .unwrap_or(false)
    {
        SslCoverageMode::San
    } else {
        SslCoverageMode::Wildcard
    };
    match set_domain_provider(&form.domain, provider) {
        Ok(msg) => {
            let cov_msg = set_coverage_mode(&form.domain, coverage)
                .unwrap_or_else(|_| format!("coverage {}", coverage.label()));
            let _ = set_include_subdomains(&form.domain, matches!(coverage, SslCoverageMode::San));
            redirect_notice(&back, Some(&format!("{msg}. {cov_msg}")), None)
        }
        Err(err) => redirect_notice(&back, None, Some(&err)),
    }
}

#[derive(Debug, Deserialize)]
pub struct SslDefaultsForm {
    pub provider: String,
}

#[post("/security/ssl/defaults")]
pub async fn security_ssl_defaults(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<SslDefaultsForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    if let Some(resp) = admin_gate(&user, "/security/ssl") {
        return resp;
    }
    match SslProvider::parse(&form.provider).and_then(|p| {
        save_ssl_defaults(p)?;
        Ok(format!(
            "New-site SSL default set to {} (existing domains unchanged)",
            p.label()
        ))
    }) {
        Ok(msg) => redirect_notice("/security/ssl", Some(&msg), None),
        Err(err) => redirect_notice("/security/ssl", None, Some(&err)),
    }
}

#[derive(Debug, Deserialize)]
pub struct SslUploadForm {
    pub domain: String,
    pub cert_pem: String,
    pub key_pem: String,
    #[serde(default)]
    pub r#return: Option<String>,
}

#[post("/security/ssl/upload")]
pub async fn security_ssl_upload(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<SslUploadForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    let back = ssl_back(form.r#return.as_deref());
    if let Some(resp) = admin_gate(&user, &back) {
        return resp;
    }
    match upload_custom_ssl(&form.domain, &form.cert_pem, &form.key_pem) {
        Ok(msg) => redirect_notice(&back, Some(&msg), None),
        Err(err) => redirect_notice(&back, None, Some(&err)),
    }
}
