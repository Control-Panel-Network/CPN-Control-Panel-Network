//! Authenticated `/packages` panel routes.

use crate::auth_api::panel_user_from_request;
use crate::installer::AppState;
use crate::packages::{
    PackageInput, assign_package, create_package, delete_package, ensure_default_package,
    get_package, is_panel_admin, update_package,
};
use crate::panel_packages::{packages_edit_main, packages_main, packages_new_main};
use crate::panel_pages::panel_shell;
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use std::sync::Arc;

fn require_panel_user(state: &AppState, http: &HttpRequest) -> Option<String> {
    panel_user_from_request(state, http)
}

fn html_ok(body: String) -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(body)
}

fn login_redirect() -> HttpResponse {
    HttpResponse::SeeOther()
        .append_header(("Location", "/login"))
        .finish()
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

fn packages_redirect(notice: Option<&str>, error: Option<&str>) -> String {
    let mut url = "/packages".to_string();
    if let Some(notice) = notice {
        url.push_str(&format!("?notice={}", urlencoding_simple(notice)));
    } else if let Some(error) = error {
        url.push_str(&format!("?error={}", urlencoding_simple(error)));
    }
    url
}

fn parse_limit(raw: &str, field: &str) -> Result<i64, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(format!("{field} is required"));
    }
    trimmed
        .parse::<i64>()
        .map_err(|_| format!("{field} must be a number (-1 for unlimited)"))
}

fn parse_bool_flag(raw: &str) -> bool {
    matches!(
        raw.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

#[derive(Debug, serde::Deserialize)]
pub struct PackageForm {
    #[serde(default)]
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    disk_mb: String,
    #[serde(default)]
    bandwidth_mb: String,
    #[serde(default)]
    domains: String,
    #[serde(default)]
    emails: String,
    #[serde(default)]
    databases: String,
    #[serde(default)]
    ftp_accounts: String,
    #[serde(default)]
    fqdn_enabled: String,
    #[serde(default)]
    notes: String,
}

impl PackageForm {
    fn to_input(&self) -> Result<PackageInput, String> {
        Ok(PackageInput {
            name: self.name.clone(),
            disk_mb: parse_limit(&self.disk_mb, "disk_mb")?,
            bandwidth_mb: parse_limit(&self.bandwidth_mb, "bandwidth_mb")?,
            domains: parse_limit(&self.domains, "domains")?,
            emails: parse_limit(&self.emails, "emails")?,
            databases: parse_limit(&self.databases, "databases")?,
            ftp_accounts: parse_limit(&self.ftp_accounts, "ftp_accounts")?,
            fqdn_enabled: parse_bool_flag(&self.fqdn_enabled),
            notes: self.notes.clone(),
        })
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct PackageIdForm {
    #[serde(default)]
    id: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct PackageAssignForm {
    #[serde(default)]
    username: String,
    #[serde(default)]
    package_id: String,
}

fn require_admin(user: &str) -> Result<(), String> {
    if is_panel_admin(user) {
        Ok(())
    } else {
        Err("Only the panel admin can manage packages".into())
    }
}

#[get("/packages")]
pub async fn packages_page(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    let _ = ensure_default_package();
    let notice = query.get("notice").map(String::as_str);
    let error = query.get("error").map(String::as_str);
    html_ok(panel_shell(
        &user,
        "packages",
        "Packages",
        &packages_main(&user, notice, error),
    ))
}

#[get("/packages/new")]
pub async fn packages_new_page(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    if let Err(error) = require_admin(&user) {
        return HttpResponse::SeeOther()
            .append_header(("Location", packages_redirect(None, Some(&error))))
            .finish();
    }
    let notice = query.get("notice").map(String::as_str);
    let error = query.get("error").map(String::as_str);
    html_ok(panel_shell(
        &user,
        "packages",
        "Create package",
        &packages_new_main(notice, error),
    ))
}

#[get("/packages/edit")]
pub async fn packages_edit_page(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    if let Err(error) = require_admin(&user) {
        return HttpResponse::SeeOther()
            .append_header(("Location", packages_redirect(None, Some(&error))))
            .finish();
    }
    let id = query.get("id").map(String::as_str).unwrap_or("");
    match get_package(id) {
        Ok(pkg) => {
            let notice = query.get("notice").map(String::as_str);
            let error = query.get("error").map(String::as_str);
            html_ok(panel_shell(
                &user,
                "packages",
                "Edit package",
                &packages_edit_main(&pkg, notice, error),
            ))
        }
        Err(error) => HttpResponse::SeeOther()
            .append_header(("Location", packages_redirect(None, Some(&error))))
            .finish(),
    }
}

#[post("/packages/create")]
pub async fn packages_create(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<PackageForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    if let Err(error) = require_admin(&user) {
        return HttpResponse::SeeOther()
            .append_header(("Location", packages_redirect(None, Some(&error))))
            .finish();
    }
    match form.to_input().and_then(create_package) {
        Ok(pkg) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                packages_redirect(Some(&format!("Created package {}", pkg.name)), None),
            ))
            .finish(),
        Err(error) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                format!("/packages/new?error={}", urlencoding_simple(&error)),
            ))
            .finish(),
    }
}

#[post("/packages/update")]
pub async fn packages_update(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<PackageForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    if let Err(error) = require_admin(&user) {
        return HttpResponse::SeeOther()
            .append_header(("Location", packages_redirect(None, Some(&error))))
            .finish();
    }
    match form
        .to_input()
        .and_then(|input| update_package(&form.id, input))
    {
        Ok(pkg) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                packages_redirect(Some(&format!("Updated package {}", pkg.name)), None),
            ))
            .finish(),
        Err(error) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                format!(
                    "/packages/edit?id={}&error={}",
                    urlencoding_simple(form.id.trim()),
                    urlencoding_simple(&error)
                ),
            ))
            .finish(),
    }
}

#[post("/packages/delete")]
pub async fn packages_delete(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<PackageIdForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    if let Err(error) = require_admin(&user) {
        return HttpResponse::SeeOther()
            .append_header(("Location", packages_redirect(None, Some(&error))))
            .finish();
    }
    match delete_package(&form.id) {
        Ok(()) => HttpResponse::SeeOther()
            .append_header(("Location", packages_redirect(Some("Package deleted"), None)))
            .finish(),
        Err(error) => HttpResponse::SeeOther()
            .append_header(("Location", packages_redirect(None, Some(&error))))
            .finish(),
    }
}

#[post("/packages/assign")]
pub async fn packages_assign(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<PackageAssignForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    if let Err(error) = require_admin(&user) {
        return HttpResponse::SeeOther()
            .append_header(("Location", packages_redirect(None, Some(&error))))
            .finish();
    }
    match assign_package(&form.username, &form.package_id) {
        Ok(()) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                packages_redirect(
                    Some(&format!("Assigned package to {}", form.username.trim())),
                    None,
                ),
            ))
            .finish(),
        Err(error) => HttpResponse::SeeOther()
            .append_header(("Location", packages_redirect(None, Some(&error))))
            .finish(),
    }
}
