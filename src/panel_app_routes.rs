//! Authenticated Apps panel routes.

use crate::apps::{AppId, install_app_on, reinstall_app_on, uninstall_app_on};
use crate::apps_control::{start_app, stop_app};
use crate::apps_site::{apply_site_scope, clear_site_scope, is_associable};
use crate::auth_api::panel_user_from_request;
use crate::installer::AppState;
use crate::login_next::login_redirect;
use crate::panel_admin::is_panel_admin;
use crate::site_acl::{SitePerm, require_manage_site};
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use std::sync::Arc;

fn require_panel_user(state: &AppState, http: &HttpRequest) -> Option<String> {
    panel_user_from_request(state, http)
}

fn urlencoding_simple(value: &str) -> String {
    let mut out = String::with_capacity(value.len() * 3);
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn apps_redirect_to(
    return_view: &str,
    domain: &str,
    notice: Option<&str>,
    error: Option<&str>,
) -> String {
    let mut url = if return_view.trim().eq_ignore_ascii_case("installed") {
        "/plugins?view=installed".to_string()
    } else {
        "/plugins?view=store&category=Host".to_string()
    };
    if !domain.trim().is_empty() {
        url.push_str(&format!("&domain={}", urlencoding_simple(domain.trim())));
    }
    if let Some(notice) = notice {
        url.push_str(&format!("&notice={}", urlencoding_simple(notice)));
    } else if let Some(error) = error {
        url.push_str(&format!("&error={}", urlencoding_simple(error)));
    }
    url
}

fn optional_domain_for_user(
    user: &str,
    domain: &str,
    perm: SitePerm,
) -> Result<Option<String>, String> {
    let domain = domain.trim();
    if domain.is_empty() {
        return Ok(None);
    }
    let site = require_manage_site(user, domain, perm)?;
    Ok(Some(site.domain))
}

#[get("/apps")]
pub async fn apps_page(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    // No domain: Store Host category. With domain: site Plugins Installed view.
    let domain = query
        .get("domain")
        .map(|s| s.trim())
        .filter(|d| !d.is_empty());
    let mut loc = if domain.is_some() {
        String::from("/plugins")
    } else {
        String::from("/plugins?view=store&category=Host")
    };
    if let Some(domain) = domain {
        let sep = if loc.contains('?') { '&' } else { '?' };
        loc.push(sep);
        loc.push_str(&format!("domain={}", urlencoding_simple(domain)));
    }
    if let Some(notice) = query.get("notice") {
        let sep = if loc.contains('?') { '&' } else { '?' };
        loc.push(sep);
        loc.push_str(&format!("notice={}", urlencoding_simple(notice)));
    } else if let Some(error) = query.get("error") {
        let sep = if loc.contains('?') { '&' } else { '?' };
        loc.push(sep);
        loc.push_str(&format!("error={}", urlencoding_simple(error)));
    }
    HttpResponse::MovedPermanently()
        .append_header(("Location", loc))
        .finish()
}

#[derive(Debug, serde::Deserialize)]
pub struct AppNameForm {
    #[serde(default)]
    name: String,
    #[serde(default)]
    domain: String,
    /// Must be `1` from the uninstall confirm dialog (ignored by other actions).
    #[serde(default)]
    confirm: String,
    /// When `installed`, POST redirects return to Installed instead of Store Host.
    #[serde(default)]
    return_view: String,
}

#[post("/apps/install")]
pub async fn apps_install(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<AppNameForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !is_panel_admin(&user) {
        return HttpResponse::SeeOther()
            .append_header((
                "Location",
                apps_redirect_to(
                    &form.return_view,
                    &form.domain,
                    None,
                    Some("Only the panel admin can install Host packages"),
                ),
            ))
            .finish();
    }
    let domain = match optional_domain_for_user(&user, &form.domain, SitePerm::Install) {
        Ok(v) => v,
        Err(error) => {
            return HttpResponse::SeeOther()
                .append_header((
                    "Location",
                    apps_redirect_to(&form.return_view, &form.domain, None, Some(&error)),
                ))
                .finish();
        }
    };
    match AppId::parse(&form.name).and_then(|id| install_app_on(id, domain.as_deref())) {
        Ok(message) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                apps_redirect_to(
                    &form.return_view,
                    domain.as_deref().unwrap_or(""),
                    Some(&message),
                    None,
                ),
            ))
            .finish(),
        Err(error) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                apps_redirect_to(
                    &form.return_view,
                    domain.as_deref().unwrap_or(""),
                    None,
                    Some(&error),
                ),
            ))
            .finish(),
    }
}

#[post("/apps/activate")]
pub async fn apps_activate(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<AppNameForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let parsed = match AppId::parse(&form.name) {
        Ok(id) => id,
        Err(error) => {
            return HttpResponse::SeeOther()
                .append_header((
                    "Location",
                    apps_redirect_to(&form.return_view, form.domain.trim(), None, Some(&error)),
                ))
                .finish();
        }
    };
    let is_webmail = matches!(
        parsed,
        AppId::Snappymail | AppId::Tachyon | AppId::Roundcube | AppId::Nextsnapmail
    );
    if is_webmail {
        return match crate::apps_webmail::activate_webmail_app(parsed) {
            Ok(message) => HttpResponse::SeeOther()
                .append_header((
                    "Location",
                    apps_redirect_to(&form.return_view, form.domain.trim(), Some(&message), None),
                ))
                .finish(),
            Err(error) => HttpResponse::SeeOther()
                .append_header((
                    "Location",
                    apps_redirect_to(&form.return_view, form.domain.trim(), None, Some(&error)),
                ))
                .finish(),
        };
    }
    let domain = match optional_domain_for_user(&user, &form.domain, SitePerm::Enable) {
        Ok(Some(d)) => d,
        Ok(None) => {
            return HttpResponse::SeeOther()
                .append_header((
                    "Location",
                    apps_redirect_to(
                        &form.return_view,
                        "",
                        None,
                        Some("Select a domain or subdomain to Activate"),
                    ),
                ))
                .finish();
        }
        Err(error) => {
            return HttpResponse::SeeOther()
                .append_header((
                    "Location",
                    apps_redirect_to(&form.return_view, &form.domain, None, Some(&error)),
                ))
                .finish();
        }
    };
    match (|| {
        if !is_associable(parsed) {
            return Err("This host package cannot be activated per site".into());
        }
        let status = crate::apps::detect_app(parsed);
        if status.state == crate::apps::AppStateKind::NotInstalled {
            return Err(
                "Host package is not installed. Ask the panel admin to Install on Host first."
                    .into(),
            );
        }
        apply_site_scope(parsed, &domain)
    })() {
        Ok(message) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                apps_redirect_to(&form.return_view, &domain, Some(&message), None),
            ))
            .finish(),
        Err(error) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                apps_redirect_to(&form.return_view, &domain, None, Some(&error)),
            ))
            .finish(),
    }
}

#[post("/apps/deactivate")]
pub async fn apps_deactivate(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<AppNameForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let domain = match optional_domain_for_user(&user, &form.domain, SitePerm::Enable) {
        Ok(Some(d)) => d,
        Ok(None) => {
            return HttpResponse::SeeOther()
                .append_header((
                    "Location",
                    apps_redirect_to(
                        &form.return_view,
                        "",
                        None,
                        Some("Select a domain to Deactivate"),
                    ),
                ))
                .finish();
        }
        Err(error) => {
            return HttpResponse::SeeOther()
                .append_header((
                    "Location",
                    apps_redirect_to(&form.return_view, &form.domain, None, Some(&error)),
                ))
                .finish();
        }
    };
    match AppId::parse(&form.name).and_then(|id| clear_site_scope(id, &domain)) {
        Ok(message) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                apps_redirect_to(&form.return_view, &domain, Some(&message), None),
            ))
            .finish(),
        Err(error) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                apps_redirect_to(&form.return_view, &domain, None, Some(&error)),
            ))
            .finish(),
    }
}

#[post("/apps/reinstall")]
pub async fn apps_reinstall(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<AppNameForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !is_panel_admin(&user) {
        return HttpResponse::SeeOther()
            .append_header((
                "Location",
                apps_redirect_to(
                    &form.return_view,
                    &form.domain,
                    None,
                    Some("Only the panel admin can reinstall Host packages"),
                ),
            ))
            .finish();
    }
    let domain = match optional_domain_for_user(&user, &form.domain, SitePerm::Install) {
        Ok(v) => v,
        Err(error) => {
            return HttpResponse::SeeOther()
                .append_header((
                    "Location",
                    apps_redirect_to(&form.return_view, &form.domain, None, Some(&error)),
                ))
                .finish();
        }
    };
    match AppId::parse(&form.name).and_then(|id| reinstall_app_on(id, domain.as_deref())) {
        Ok(message) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                apps_redirect_to(
                    &form.return_view,
                    domain.as_deref().unwrap_or(""),
                    Some(&message),
                    None,
                ),
            ))
            .finish(),
        Err(error) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                apps_redirect_to(
                    &form.return_view,
                    domain.as_deref().unwrap_or(""),
                    None,
                    Some(&error),
                ),
            ))
            .finish(),
    }
}

#[post("/apps/uninstall")]
pub async fn apps_uninstall(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<AppNameForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !is_panel_admin(&user) {
        return HttpResponse::SeeOther()
            .append_header((
                "Location",
                apps_redirect_to(&form.return_view, 
                    &form.domain,
                    None,
                    Some(
                        "Only the panel admin can uninstall Host packages. Use Deactivate for your site.",
                    ),
                ),
            ))
            .finish();
    }
    if !crate::uninstall_confirm::confirm_accepted(&form.confirm) {
        return HttpResponse::SeeOther()
            .append_header((
                "Location",
                apps_redirect_to(
                    &form.return_view,
                    &form.domain,
                    None,
                    Some(crate::uninstall_confirm::CONFIRM_REQUIRED_MSG),
                ),
            ))
            .finish();
    }
    // Admin host uninstall ignores site domain so packages are removed once.
    match AppId::parse(&form.name).and_then(|id| uninstall_app_on(id, None)) {
        Ok(message) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                apps_redirect_to(&form.return_view, form.domain.trim(), Some(&message), None),
            ))
            .finish(),
        Err(error) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                apps_redirect_to(&form.return_view, form.domain.trim(), None, Some(&error)),
            ))
            .finish(),
    }
}

#[post("/apps/start")]
pub async fn apps_start(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<AppNameForm>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    match AppId::parse(&form.name).and_then(start_app) {
        Ok(message) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                apps_redirect_to(&form.return_view, form.domain.trim(), Some(&message), None),
            ))
            .finish(),
        Err(error) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                apps_redirect_to(&form.return_view, form.domain.trim(), None, Some(&error)),
            ))
            .finish(),
    }
}

#[post("/apps/stop")]
pub async fn apps_stop(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<AppNameForm>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    match AppId::parse(&form.name).and_then(stop_app) {
        Ok(message) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                apps_redirect_to(&form.return_view, form.domain.trim(), Some(&message), None),
            ))
            .finish(),
        Err(error) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                apps_redirect_to(&form.return_view, form.domain.trim(), None, Some(&error)),
            ))
            .finish(),
    }
}
