//! Backups hub feature routes.

use crate::backup_restore::{RestoreRequest, restore_backup};
use crate::installer::AppState;
use crate::panel_backups::BackupsPageQuery;
use crate::backup_restore_extract::RESTORE_PLAN_RENDER_BUDGET;
use crate::panel_hub_http::{
    html_blocking, html_blocking_budget, html_ok, login_redirect, redirect_notice,
    require_panel_user, urlencoding_simple,
};
use crate::panel_hub_pages_backups::{
    backups_create_page, backups_destinations_page, backups_restore_page, backups_schedule_page,
    save_backup_destinations, save_backup_schedule,
};
use crate::panel_hub_pages_backups_plan::backups_restore_plan_page;
use crate::panel_hub_pages_hosting::scaffold_feature;
use crate::panel_pages::panel_shell;
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use std::sync::Arc;

#[get("/backups/create")]
pub async fn backups_create_route(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    html_ok(panel_shell(
        &user,
        "backups",
        "Create Backup",
        &backups_create_page(BackupsPageQuery {
            notice: query.get("notice").map(String::as_str),
            error: query.get("error").map(String::as_str),
            scope: query.get("scope").map(String::as_str).unwrap_or("panel"),
            domain: query.get("domain").map(String::as_str).unwrap_or(""),
        }),
    ))
}

#[get("/backups/restore")]
pub async fn backups_restore_route(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let scope = query
        .get("scope")
        .map(String::as_str)
        .unwrap_or("site")
        .to_string();
    let domain = query
        .get("domain")
        .map(String::as_str)
        .unwrap_or("")
        .to_string();
    let notice = query.get("notice").cloned();
    let error = query.get("error").cloned();
    html_blocking(move || {
        panel_shell(
            &user,
            "backups",
            "Restore Backup",
            &backups_restore_page(&scope, &domain, notice.as_deref(), error.as_deref()),
        )
    })
    .await
}

#[get("/backups/restore/plan")]
pub async fn backups_restore_plan_route(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let archive = query.get("archive").map(String::as_str).unwrap_or("");
    if archive.trim().is_empty() {
        return redirect_notice(
            "/backups/restore",
            None,
            Some("Choose an archive, then Select entities."),
        );
    }
    let scope = query
        .get("scope")
        .map(String::as_str)
        .unwrap_or("site")
        .to_string();
    let domain = query
        .get("domain")
        .map(String::as_str)
        .unwrap_or("")
        .to_string();
    let archive = archive.to_string();
    let format = query
        .get("format")
        .map(String::as_str)
        .unwrap_or("auto")
        .to_string();
    let db_name = query
        .get("db_name")
        .map(String::as_str)
        .unwrap_or("")
        .to_string();
    let notice = query.get("notice").cloned();
    let error = query.get("error").cloned();
    // Inventory runs on the blocking pool. Large classic source control-panel
    // archives use a long budget (and disk cache) so plan listing is not cut at 15-20s.
    html_blocking_budget(RESTORE_PLAN_RENDER_BUDGET, move || {
        panel_shell(
            &user,
            "backups",
            "Select restore entities",
            &backups_restore_plan_page(
                &scope,
                &domain,
                &archive,
                &format,
                &db_name,
                notice.as_deref(),
                error.as_deref(),
            ),
        )
    })
    .await
}

#[derive(Debug, serde::Deserialize)]
pub struct RestoreRunForm {
    #[serde(default)]
    scope: String,
    #[serde(default)]
    domain: String,
    #[serde(default)]
    archive: String,
    #[serde(default)]
    format: String,
    #[serde(default)]
    db_name: String,
    #[serde(default)]
    create_domain_if_missing: String,
    #[serde(default)]
    confirm_create_domain: String,
    #[serde(default)]
    confirm_overwrite_files: String,
    #[serde(default)]
    confirm_import_databases: String,
    #[serde(default)]
    confirm_optional_entities: String,
    #[serde(default)]
    confirm_users_acl_packages: String,
    /// Comma-separated entity ids from the plan page (`website`, `db:name`, `site:x`, …).
    /// Checkboxes sync into this hidden field so Actix never sees repeated `entity` keys.
    #[serde(default)]
    entities: String,
}

#[post("/backups/restore/run")]
pub async fn backups_restore_run(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<RestoreRunForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let scope = if form.scope.trim().is_empty() {
        "site"
    } else {
        form.scope.trim()
    };
    let domain = form.domain.trim();
    let mut return_base = format!(
        "/backups/restore/plan?scope={}&domain={}&archive={}",
        urlencoding_simple(scope),
        urlencoding_simple(domain),
        urlencoding_simple(form.archive.trim())
    );
    let mut entity_ids: Vec<String> = form
        .entities
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    // Deduplicate while preserving order.
    let mut seen = std::collections::HashSet::new();
    entity_ids.retain(|id| seen.insert(id.clone()));
    let mut req = RestoreRequest::from_form_flags(
        scope.to_string(),
        domain.to_string(),
        form.archive.clone(),
        form.format.clone(),
        form.db_name.clone(),
        &form.create_domain_if_missing,
        &form.confirm_create_domain,
        &form.confirm_overwrite_files,
        &form.confirm_import_databases,
        &form.confirm_optional_entities,
        &form.confirm_users_acl_packages,
        entity_ids,
    );
    req.owner = user.clone();
    // Restore can take a while; never occupy an Actix worker with tar extract / SQL import.
    let outcome = match web::block(move || restore_backup(&req)).await {
        Ok(inner) => inner,
        Err(_) => {
            Err("Restore worker failed unexpectedly. Reload the plan page and try again.".into())
        }
    };
    match outcome {
        Ok(result) => {
            let mut msg = result.message;
            if !result.entity_statuses.is_empty() {
                let summary: Vec<String> = result
                    .entity_statuses
                    .iter()
                    .map(|s| format!("{}:{}", s.id, s.status))
                    .collect();
                msg.push_str(" Entities: ");
                msg.push_str(&summary.join(", "));
            }
            if !result.warnings.is_empty() {
                msg.push(' ');
                msg.push_str(&result.warnings.join(" "));
            }
            // Keep redirect query reasonable.
            if msg.len() > 500 {
                msg.truncate(497);
                msg.push_str("...");
            }
            return_base.push_str("&notice=");
            return_base.push_str(&urlencoding_simple(&msg));
            HttpResponse::SeeOther()
                .append_header(("Location", return_base))
                .finish()
        }
        Err(err) => {
            return_base.push_str("&error=");
            return_base.push_str(&urlencoding_simple(&err));
            HttpResponse::SeeOther()
                .append_header(("Location", return_base))
                .finish()
        }
    }
}

#[get("/backups/schedule")]
pub async fn backups_schedule_route(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    html_ok(panel_shell(
        &user,
        "backups",
        "Schedule Backup",
        &backups_schedule_page(
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}

#[derive(Debug, serde::Deserialize)]
pub struct ScheduleForm {
    #[serde(default)]
    enabled: String,
    #[serde(default)]
    cron: String,
    #[serde(default)]
    scope: String,
    #[serde(default)]
    domain: String,
}

#[post("/backups/schedule/save")]
pub async fn backups_schedule_save(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<ScheduleForm>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let enabled = matches!(form.enabled.trim(), "1" | "true" | "on" | "yes");
    match save_backup_schedule(enabled, &form.cron, &form.scope, &form.domain) {
        Ok(msg) => redirect_notice("/backups/schedule", Some(&msg), None),
        Err(err) => redirect_notice("/backups/schedule", None, Some(&err)),
    }
}

#[get("/backups/destinations")]
pub async fn backups_destinations_route(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    html_ok(panel_shell(
        &user,
        "backups",
        "Destinations",
        &backups_destinations_page(
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}

#[derive(Debug, serde::Deserialize)]
pub struct DestinationsForm {
    #[serde(default)]
    local_enabled: String,
    #[serde(default)]
    google_drive_note: String,
    #[serde(default)]
    remote_note: String,
}

#[post("/backups/destinations/save")]
pub async fn backups_destinations_save(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<DestinationsForm>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let local = matches!(form.local_enabled.trim(), "1" | "true" | "on" | "yes");
    match save_backup_destinations(local, &form.google_drive_note, &form.remote_note) {
        Ok(msg) => redirect_notice("/backups/destinations", Some(&msg), None),
        Err(err) => redirect_notice("/backups/destinations", None, Some(&err)),
    }
}

#[get("/backups/google-drive")]
pub async fn backups_gdrive_route(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    html_ok(panel_shell(
        &user,
        "backups",
        "Google Drive",
        &scaffold_feature(
            "Backups",
            "/backups",
            "Google Drive",
            "Backup to Drive",
            "Google Drive OAuth and sync are not configured yet.",
        ),
    ))
}

#[get("/backups/remote")]
pub async fn backups_remote_route(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    html_ok(panel_shell(
        &user,
        "backups",
        "Remote Backups",
        &scaffold_feature(
            "Backups",
            "/backups",
            "Remote Backups",
            "Transfer to another server",
            "Remote transfer targets are not configured yet.",
        ),
    ))
}

#[cfg(test)]
mod tests {
    use super::RestoreRunForm;

    #[test]
    fn restore_run_form_parses_comma_entities() {
        let form: RestoreRunForm = serde_json::from_str(
            r#"{"entities":"site:newstargeted.com,website,db:news_disco","archive":"a.tar.gz"}"#,
        )
        .expect("entities");
        assert_eq!(form.archive, "a.tar.gz");
        assert!(form.entities.contains("website"));
        assert!(form.entities.contains("db:news_disco"));
    }

    #[test]
    fn restore_run_form_allows_empty_entities() {
        let form: RestoreRunForm =
            serde_json::from_str(r#"{"archive":"a.tar.gz"}"#).expect("empty entities");
        assert!(form.entities.is_empty());
    }
}
