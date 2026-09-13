//! Bulk update/delete and duplicate routes for `/packages`.

use crate::auth_api::panel_user_from_request;
use crate::installer::AppState;
use crate::login_next::login_redirect;
use crate::package_bulk::{
    PackageBulkPatch, bulk_delete_packages, bulk_update_packages, duplicate_package,
};
use crate::packages::is_panel_admin;
use actix_web::{HttpRequest, HttpResponse, post, web};
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

fn packages_redirect(notice: Option<&str>, error: Option<&str>) -> String {
    let mut url = "/packages".to_string();
    if let Some(notice) = notice {
        url.push_str(&format!("?notice={}", urlencoding_simple(notice)));
    } else if let Some(error) = error {
        url.push_str(&format!("?error={}", urlencoding_simple(error)));
    }
    url
}

fn parse_optional_limit(raw: &str, field: &str) -> Result<Option<i64>, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    trimmed
        .parse::<i64>()
        .map(Some)
        .map_err(|_| format!("{field} must be a number (-1 for unlimited)"))
}

fn parse_bool_flag(raw: &str) -> bool {
    matches!(
        raw.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn require_admin(user: &str) -> Result<(), String> {
    if is_panel_admin(user) {
        Ok(())
    } else {
        Err("Only the panel admin can manage packages".into())
    }
}

fn redirect_after_bulk(outcome: &crate::package_bulk::BulkOutcome, verb: &str) -> HttpResponse {
    let summary = outcome.summary(verb);
    let location = if outcome.ok > 0 {
        packages_redirect(Some(&summary), None)
    } else {
        packages_redirect(None, Some(&summary))
    };
    HttpResponse::SeeOther()
        .append_header(("Location", location))
        .finish()
}

#[derive(Debug, serde::Deserialize)]
pub struct PackageDuplicateForm {
    #[serde(default)]
    id: String,
    #[serde(default)]
    new_name: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct PackageBulkForm {
    #[serde(default)]
    action: String,
    /// Comma-separated package ids (filled by the packages list JS).
    #[serde(default)]
    package_ids: String,
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
    #[serde(default)]
    apply_notes: String,
}

fn parse_package_ids(raw: &str) -> Vec<String> {
    raw.split([',', ' ', '\n', '\r', '\t'])
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_string)
        .collect()
}

#[post("/packages/duplicate")]
pub async fn packages_duplicate(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<PackageDuplicateForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Err(error) = require_admin(&user) {
        return HttpResponse::SeeOther()
            .append_header(("Location", packages_redirect(None, Some(&error))))
            .finish();
    }
    match duplicate_package(&form.id, &form.new_name) {
        Ok(pkg) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                packages_redirect(Some(&format!("Duplicated package as {}", pkg.name)), None),
            ))
            .finish(),
        Err(error) => HttpResponse::SeeOther()
            .append_header(("Location", packages_redirect(None, Some(&error))))
            .finish(),
    }
}

#[post("/packages/bulk")]
pub async fn packages_bulk(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<PackageBulkForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Err(error) = require_admin(&user) {
        return HttpResponse::SeeOther()
            .append_header(("Location", packages_redirect(None, Some(&error))))
            .finish();
    }
    let ids = parse_package_ids(&form.package_ids);
    let action = form.action.trim().to_ascii_lowercase();
    match action.as_str() {
        "delete" => redirect_after_bulk(&bulk_delete_packages(&ids), "Deleted"),
        "fqdn_enable" => redirect_after_bulk(
            &bulk_update_packages(
                &ids,
                &PackageBulkPatch {
                    fqdn_enabled: Some(true),
                    ..Default::default()
                },
            ),
            "Enabled FQDN on",
        ),
        "fqdn_disable" => redirect_after_bulk(
            &bulk_update_packages(
                &ids,
                &PackageBulkPatch {
                    fqdn_enabled: Some(false),
                    ..Default::default()
                },
            ),
            "Disabled FQDN on",
        ),
        "update" => {
            let patch = match (|| -> Result<PackageBulkPatch, String> {
                Ok(PackageBulkPatch {
                    disk_mb: parse_optional_limit(&form.disk_mb, "disk_mb")?,
                    bandwidth_mb: parse_optional_limit(&form.bandwidth_mb, "bandwidth_mb")?,
                    domains: parse_optional_limit(&form.domains, "domains")?,
                    emails: parse_optional_limit(&form.emails, "emails")?,
                    databases: parse_optional_limit(&form.databases, "databases")?,
                    ftp_accounts: parse_optional_limit(&form.ftp_accounts, "ftp_accounts")?,
                    fqdn_enabled: match form.fqdn_enabled.trim() {
                        "" => None,
                        "1" | "true" | "yes" | "on" | "enable" => Some(true),
                        "0" | "false" | "no" | "off" | "disable" => Some(false),
                        other => {
                            return Err(format!("Invalid fqdn_enabled value: {other}"));
                        }
                    },
                    notes: if parse_bool_flag(&form.apply_notes) {
                        Some(form.notes.clone())
                    } else {
                        None
                    },
                })
            })() {
                Ok(patch) => patch,
                Err(error) => {
                    return HttpResponse::SeeOther()
                        .append_header(("Location", packages_redirect(None, Some(&error))))
                        .finish();
                }
            };
            redirect_after_bulk(&bulk_update_packages(&ids, &patch), "Updated")
        }
        _ => HttpResponse::SeeOther()
            .append_header((
                "Location",
                packages_redirect(None, Some("Unknown bulk action")),
            ))
            .finish(),
    }
}
