//! Reseller Center routes (GET page + CSRF-protected POSTs).

use crate::account::default_password_policy;
use crate::account_mgmt::create_account;
use crate::installer::AppState;
use crate::packages::is_panel_admin;
use crate::panel_hub_admin_gate::{ADMIN_ONLY_CODE, admin_only_html, decode_notice_code};
use crate::panel_hub_http::{html_ok, login_redirect, redirect_notice, require_panel_user};
use crate::panel_hub_pages_reseller::users_reseller_page;
use crate::panel_hub_pages_reseller_tabs::normalize_reseller_tab;
use crate::panel_pages::panel_shell;
use crate::panel_reseller::{
    ResellerBranding, assign_user_to_reseller, can_access_reseller_center, demote_reseller,
    promote_reseller, quotas_from_fields, unassign_user, update_reseller_branding,
    update_reseller_quotas,
};
use crate::panel_reseller_csrf::verify_reseller_csrf;
use crate::panel_site_tools_security::same_origin_ok;
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use std::sync::Arc;

fn forbid_reseller(viewer: &str) -> HttpResponse {
    admin_only_html(viewer, "Reseller Center")
}

fn require_reseller_csrf(http: &HttpRequest, user: &str, token: &str) -> Result<(), String> {
    if !same_origin_ok(http) {
        return Err("Request origin was rejected".into());
    }
    if !verify_reseller_csrf(user, token) {
        return Err("CSRF check failed; reload the page and try again".into());
    }
    Ok(())
}

fn redirect_reseller(tab: &str, notice: Option<&str>, error: Option<&str>) -> HttpResponse {
    let tab = normalize_reseller_tab(tab);
    redirect_notice(
        &format!("/account/users/reseller?tab={tab}"),
        notice,
        error,
    )
}

#[derive(Debug, serde::Deserialize)]
pub struct ResellerCsrfForm {
    #[serde(default)]
    csrf: String,
    #[serde(default)]
    username: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct ResellerPromoteForm {
    #[serde(default)]
    csrf: String,
    #[serde(default)]
    username: String,
    #[serde(default)]
    q_websites: String,
    #[serde(default)]
    q_mailboxes: String,
    #[serde(default)]
    q_databases: String,
    #[serde(default)]
    q_ftp: String,
    #[serde(default)]
    q_storage: String,
    #[serde(default)]
    q_bandwidth: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct ResellerAssignForm {
    #[serde(default)]
    csrf: String,
    #[serde(default)]
    child: String,
    #[serde(default)]
    reseller: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct ResellerBrandingForm {
    #[serde(default)]
    csrf: String,
    #[serde(default)]
    username: String,
    #[serde(default)]
    display_name: String,
    #[serde(default)]
    tagline: String,
    #[serde(default)]
    logo_url: String,
    #[serde(default)]
    primary_color: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct ResellerCreateUserForm {
    #[serde(default)]
    csrf: String,
    #[serde(default)]
    reseller: String,
    #[serde(default)]
    username: String,
    #[serde(default)]
    recovery_email: String,
    #[serde(default)]
    password: String,
    #[serde(default)]
    generate: String,
}

#[get("/account/users/reseller")]
pub async fn users_reseller_route(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !can_access_reseller_center(&user) {
        return forbid_reseller(&user);
    }
    let notice = query.get("notice").map(|s| decode_notice_code(s));
    let error = query.get("error").map(|s| decode_notice_code(s));
    let tab = query
        .get("tab")
        .map(|s| s.as_str())
        .unwrap_or("overview");
    html_ok(panel_shell(
        &user,
        "users",
        "Reseller Center",
        &users_reseller_page(&user, notice.as_deref(), error.as_deref(), tab),
    ))
}

#[post("/account/users/reseller/promote")]
pub async fn users_reseller_promote_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<ResellerPromoteForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !is_panel_admin(&user) {
        return redirect_reseller("resellers", None, Some(ADMIN_ONLY_CODE));
    }
    if let Err(err) = require_reseller_csrf(&http, &user, &form.csrf) {
        return redirect_reseller("resellers", None, Some(&err));
    }
    let quotas = match quotas_from_fields(
        &form.q_websites,
        &form.q_mailboxes,
        &form.q_databases,
        &form.q_ftp,
        &form.q_storage,
        &form.q_bandwidth,
    ) {
        Ok(q) => q,
        Err(err) => return redirect_reseller("resellers", None, Some(&err)),
    };
    match promote_reseller(&form.username, quotas) {
        Ok(row) => redirect_reseller(
            "resellers",
            Some(&format!("Promoted {} to reseller", row.username)),
            None,
        ),
        Err(err) => redirect_reseller("resellers", None, Some(&err)),
    }
}

#[post("/account/users/reseller/demote")]
pub async fn users_reseller_demote_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<ResellerCsrfForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !is_panel_admin(&user) {
        return redirect_reseller("resellers", None, Some(ADMIN_ONLY_CODE));
    }
    if let Err(err) = require_reseller_csrf(&http, &user, &form.csrf) {
        return redirect_reseller("resellers", None, Some(&err));
    }
    match demote_reseller(&form.username) {
        Ok(()) => redirect_reseller("resellers", Some("Reseller demoted"), None),
        Err(err) => redirect_reseller("resellers", None, Some(&err)),
    }
}

#[post("/account/users/reseller/quotas")]
pub async fn users_reseller_quotas_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<ResellerPromoteForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !is_panel_admin(&user) {
        return redirect_reseller("quotas", None, Some(ADMIN_ONLY_CODE));
    }
    if let Err(err) = require_reseller_csrf(&http, &user, &form.csrf) {
        return redirect_reseller("quotas", None, Some(&err));
    }
    let quotas = match quotas_from_fields(
        &form.q_websites,
        &form.q_mailboxes,
        &form.q_databases,
        &form.q_ftp,
        &form.q_storage,
        &form.q_bandwidth,
    ) {
        Ok(q) => q,
        Err(err) => return redirect_reseller("quotas", None, Some(&err)),
    };
    match update_reseller_quotas(&form.username, quotas) {
        Ok(()) => redirect_reseller("quotas", Some("Reseller quotas updated"), None),
        Err(err) => redirect_reseller("quotas", None, Some(&err)),
    }
}

#[post("/account/users/reseller/branding")]
pub async fn users_reseller_branding_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<ResellerBrandingForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !can_access_reseller_center(&user) {
        return forbid_reseller(&user);
    }
    if let Err(err) = require_reseller_csrf(&http, &user, &form.csrf) {
        return redirect_reseller("branding", None, Some(&err));
    }
    let target = form.username.trim();
    if !is_panel_admin(&user) && !target.eq_ignore_ascii_case(&user) {
        return redirect_reseller("branding", None, Some(ADMIN_ONLY_CODE));
    }
    let branding = ResellerBranding {
        display_name: form.display_name.clone(),
        tagline: form.tagline.clone(),
        logo_url: form.logo_url.clone(),
        primary_color: form.primary_color.clone(),
    };
    match update_reseller_branding(target, branding) {
        Ok(()) => redirect_reseller("branding", Some("Branding saved"), None),
        Err(err) => redirect_reseller("branding", None, Some(&err)),
    }
}

#[post("/account/users/reseller/assign")]
pub async fn users_reseller_assign_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<ResellerAssignForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !is_panel_admin(&user) {
        return redirect_reseller("resellers", None, Some(ADMIN_ONLY_CODE));
    }
    if let Err(err) = require_reseller_csrf(&http, &user, &form.csrf) {
        return redirect_reseller("resellers", None, Some(&err));
    }
    match assign_user_to_reseller(&form.child, &form.reseller) {
        Ok(()) => redirect_reseller("resellers", Some("User assigned under reseller"), None),
        Err(err) => redirect_reseller("resellers", None, Some(&err)),
    }
}

#[post("/account/users/reseller/unassign")]
pub async fn users_reseller_unassign_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<ResellerAssignForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !is_panel_admin(&user) {
        return redirect_reseller("resellers", None, Some(ADMIN_ONLY_CODE));
    }
    if let Err(err) = require_reseller_csrf(&http, &user, &form.csrf) {
        return redirect_reseller("resellers", None, Some(&err));
    }
    match unassign_user(&form.child) {
        Ok(()) => redirect_reseller("resellers", Some("User unassigned from reseller"), None),
        Err(err) => redirect_reseller("resellers", None, Some(&err)),
    }
}

#[post("/account/users/reseller/create-user")]
pub async fn users_reseller_create_user_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<ResellerCreateUserForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !can_access_reseller_center(&user) {
        return forbid_reseller(&user);
    }
    if let Err(err) = require_reseller_csrf(&http, &user, &form.csrf) {
        return redirect_reseller("resellers", None, Some(&err));
    }
    let parent = if is_panel_admin(&user) {
        form.reseller.trim().to_string()
    } else {
        user.clone()
    };
    if parent.is_empty() {
        return redirect_reseller("resellers", None, Some("Parent reseller is required"));
    }
    if !is_panel_admin(&user) && !parent.eq_ignore_ascii_case(&user) {
        return redirect_reseller("resellers", None, Some(ADMIN_ONLY_CODE));
    }
    let generate = matches!(
        form.generate.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    );
    let password = if generate {
        None
    } else {
        Some(form.password.as_str())
    };
    let created = match create_account(
        &form.username,
        password,
        generate,
        &form.recovery_email,
        default_password_policy(),
        "en",
    ) {
        Ok(r) => r,
        Err(err) => return redirect_reseller("resellers", None, Some(&err)),
    };
    if let Err(err) = assign_user_to_reseller(&created.public.username, &parent) {
        let _ = crate::account_mgmt::delete_account(&created.public.username);
        return redirect_reseller("resellers", None, Some(&err));
    }
    let mut notice = format!(
        "Created child user {} under {}",
        created.public.username, parent
    );
    if let Some(pw) = created.generated_password {
        notice.push_str(&format!(". Generated password: {pw}"));
    }
    redirect_reseller("resellers", Some(&notice), None)
}
