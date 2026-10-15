//! Backups hub feature routes.

use crate::backup_restore::{RestoreRequest, restore_backup};
use crate::backup_restore_extract::RESTORE_PLAN_RENDER_BUDGET;
use crate::installer::AppState;
use crate::panel_backups::BackupsPageQuery;
use crate::panel_hub_http::{
    html_blocking, html_blocking_budget, html_ok, login_redirect, redirect_notice,
    require_panel_user, urlencoding_simple,
};
use crate::panel_hub_pages_backups::{
    backups_create_page, backups_destinations_page, backups_restore_page, backups_schedule_page,
    save_backup_schedule,
};
use crate::panel_hub_pages_backups_dest::{
    RemoteSaveInput, backups_gdrive_page, backups_remote_page, backups_s3_page, parse_flag,
    run_gdrive_test, run_remote_test, run_s3_test, save_backup_destinations_toggles,
    save_gdrive_form, save_remote_form, save_s3_form,
};
use crate::panel_hub_pages_backups_plan::backups_restore_plan_page;
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
    gdrive_enabled: String,
    #[serde(default)]
    remote_enabled: String,
    #[serde(default)]
    s3_enabled: String,
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
    let unlocked = crate::panel_feature_gate::backup_destinations_unlocked();
    let (gdrive, remote, s3) = if unlocked {
        (
            parse_flag(&form.gdrive_enabled),
            parse_flag(&form.remote_enabled),
            parse_flag(&form.s3_enabled),
        )
    } else {
        (false, false, false)
    };
    match save_backup_destinations_toggles(parse_flag(&form.local_enabled), gdrive, remote, s3) {
        Ok(msg) => redirect_notice("/backups/destinations", Some(&msg), None),
        Err(err) => redirect_notice("/backups/destinations", None, Some(&err)),
    }
}

#[get("/backups/google-drive")]
pub async fn backups_gdrive_route(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !crate::panel_feature_gate::backup_destinations_unlocked() {
        return html_ok(panel_shell(
            &user,
            "backups",
            "Google Drive",
            &crate::panel_feature_gate::backup_destinations_plugin_required_page("Google Drive"),
        ));
    }
    html_ok(panel_shell(
        &user,
        "backups",
        "Google Drive",
        &backups_gdrive_page(
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}

#[derive(Debug, serde::Deserialize)]
pub struct GdriveSaveForm {
    #[serde(default)]
    enabled: String,
    #[serde(default)]
    client_id: String,
    #[serde(default)]
    client_secret: String,
    #[serde(default)]
    refresh_token: String,
    #[serde(default)]
    folder_id: String,
    #[serde(default)]
    note: String,
}

#[post("/backups/google-drive/save")]
pub async fn backups_gdrive_save(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<GdriveSaveForm>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !crate::panel_feature_gate::backup_destinations_unlocked() {
        return redirect_notice(
            "/backups/google-drive",
            None,
            Some("Install the free backupDestinations plugin from the Plugin Store first."),
        );
    }
    match save_gdrive_form(
        parse_flag(&form.enabled),
        &form.client_id,
        &form.client_secret,
        &form.refresh_token,
        &form.folder_id,
        &form.note,
    ) {
        Ok(msg) => redirect_notice("/backups/google-drive", Some(&msg), None),
        Err(err) => redirect_notice("/backups/google-drive", None, Some(&err)),
    }
}

#[post("/backups/google-drive/test")]
pub async fn backups_gdrive_test(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !crate::panel_feature_gate::backup_destinations_unlocked() {
        return redirect_notice(
            "/backups/google-drive",
            None,
            Some("Install the free backupDestinations plugin from the Plugin Store first."),
        );
    }
    match run_gdrive_test() {
        Ok(msg) => redirect_notice("/backups/google-drive", Some(&msg), None),
        Err(err) => redirect_notice("/backups/google-drive", None, Some(&err)),
    }
}

#[get("/backups/remote")]
pub async fn backups_remote_route(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !crate::panel_feature_gate::backup_destinations_unlocked() {
        return html_ok(panel_shell(
            &user,
            "backups",
            "Remote Backups",
            &crate::panel_feature_gate::backup_destinations_plugin_required_page("Remote Backups"),
        ));
    }
    html_ok(panel_shell(
        &user,
        "backups",
        "Remote Backups",
        &backups_remote_page(
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}

#[derive(Debug, serde::Deserialize)]
pub struct RemoteSaveForm {
    #[serde(default)]
    enabled: String,
    #[serde(default)]
    protocol: String,
    #[serde(default)]
    host: String,
    #[serde(default)]
    port: u16,
    #[serde(default)]
    username: String,
    #[serde(default)]
    auth_mode: String,
    #[serde(default)]
    key_path: String,
    #[serde(default)]
    password: String,
    #[serde(default)]
    remote_path: String,
    #[serde(default)]
    note: String,
}

#[post("/backups/remote/save")]
pub async fn backups_remote_save(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<RemoteSaveForm>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !crate::panel_feature_gate::backup_destinations_unlocked() {
        return redirect_notice(
            "/backups/remote",
            None,
            Some("Install the free backupDestinations plugin from the Plugin Store first."),
        );
    }
    match save_remote_form(RemoteSaveInput {
        enabled: parse_flag(&form.enabled),
        protocol: &form.protocol,
        host: &form.host,
        port: form.port,
        username: &form.username,
        auth_mode: &form.auth_mode,
        key_path: &form.key_path,
        password: &form.password,
        remote_path: &form.remote_path,
        note: &form.note,
    }) {
        Ok(msg) => redirect_notice("/backups/remote", Some(&msg), None),
        Err(err) => redirect_notice("/backups/remote", None, Some(&err)),
    }
}

#[post("/backups/remote/test")]
pub async fn backups_remote_test(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !crate::panel_feature_gate::backup_destinations_unlocked() {
        return redirect_notice(
            "/backups/remote",
            None,
            Some("Install the free backupDestinations plugin from the Plugin Store first."),
        );
    }
    match run_remote_test() {
        Ok(msg) => redirect_notice("/backups/remote", Some(&msg), None),
        Err(err) => redirect_notice("/backups/remote", None, Some(&err)),
    }
}

#[get("/backups/s3")]
pub async fn backups_s3_route(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !crate::panel_feature_gate::backup_destinations_unlocked() {
        return html_ok(panel_shell(
            &user,
            "backups",
            "S3",
            &crate::panel_feature_gate::backup_destinations_plugin_required_page("S3"),
        ));
    }
    html_ok(panel_shell(
        &user,
        "backups",
        "S3",
        &backups_s3_page(
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}

#[derive(Debug, serde::Deserialize)]
pub struct S3SaveForm {
    #[serde(default)]
    enabled: String,
    #[serde(default)]
    endpoint: String,
    #[serde(default)]
    region: String,
    #[serde(default)]
    bucket: String,
    #[serde(default)]
    prefix: String,
    #[serde(default)]
    access_key: String,
    #[serde(default)]
    secret_key: String,
}

#[post("/backups/s3/save")]
pub async fn backups_s3_save(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<S3SaveForm>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !crate::panel_feature_gate::backup_destinations_unlocked() {
        return redirect_notice(
            "/backups/s3",
            None,
            Some("Install the free backupDestinations plugin from the Plugin Store first."),
        );
    }
    match save_s3_form(
        parse_flag(&form.enabled),
        &form.endpoint,
        &form.region,
        &form.bucket,
        &form.prefix,
        &form.access_key,
        &form.secret_key,
    ) {
        Ok(msg) => redirect_notice("/backups/s3", Some(&msg), None),
        Err(err) => redirect_notice("/backups/s3", None, Some(&err)),
    }
}

#[post("/backups/s3/test")]
pub async fn backups_s3_test(http: HttpRequest, state: web::Data<Arc<AppState>>) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !crate::panel_feature_gate::backup_destinations_unlocked() {
        return redirect_notice(
            "/backups/s3",
            None,
            Some("Install the free backupDestinations plugin from the Plugin Store first."),
        );
    }
    match run_s3_test() {
        Ok(msg) => redirect_notice("/backups/s3", Some(&msg), None),
        Err(err) => redirect_notice("/backups/s3", None, Some(&err)),
    }
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
