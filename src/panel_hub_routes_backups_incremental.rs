//! Incremental backup job routes.

use crate::installer::AppState;
use crate::panel_hub_http::{html_ok, login_redirect, redirect_notice, require_panel_user};
use crate::panel_hub_pages_backups_incremental::{
    backups_incremental_page, run_incremental_form, save_incremental_form,
};
use crate::panel_pages::panel_shell;
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use std::sync::Arc;

#[get("/backups/incremental")]
pub async fn backups_incremental_route(
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
            "Incremental backups",
            &crate::panel_feature_gate::backup_destinations_plugin_required_page(
                "Incremental backups",
            ),
        ));
    }
    html_ok(panel_shell(
        &user,
        "backups",
        "Incremental backups",
        &backups_incremental_page(
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}

#[derive(Debug, serde::Deserialize)]
pub struct IncrementalSaveForm {
    #[serde(default)]
    id: String,
    #[serde(default)]
    engine: String,
    #[serde(default)]
    source: String,
    #[serde(default)]
    destination: String,
    #[serde(default)]
    schedule_cron: String,
    #[serde(default)]
    enabled: String,
}

#[post("/backups/incremental/save")]
pub async fn backups_incremental_save(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<IncrementalSaveForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !crate::panel_feature_gate::backup_destinations_unlocked() {
        return redirect_notice(
            "/backups/incremental",
            None,
            Some("Install the free backupDestinations plugin from the Plugin Store first."),
        );
    }
    if !crate::panel_admin::is_panel_admin(&user) {
        return redirect_notice(
            "/backups/incremental",
            None,
            Some("Only panel admins can manage incremental backup jobs."),
        );
    }
    let enabled = form.enabled == "1" || form.enabled.eq_ignore_ascii_case("on");
    match save_incremental_form(
        &form.id,
        &form.engine,
        &form.source,
        &form.destination,
        &form.schedule_cron,
        enabled,
    ) {
        Ok(msg) => redirect_notice("/backups/incremental", Some(&msg), None),
        Err(err) => redirect_notice("/backups/incremental", None, Some(&err)),
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct IncrementalRunForm {
    #[serde(default)]
    id: String,
}

#[post("/backups/incremental/run")]
pub async fn backups_incremental_run(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<IncrementalRunForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !crate::panel_feature_gate::backup_destinations_unlocked() {
        return redirect_notice(
            "/backups/incremental",
            None,
            Some("Install the free backupDestinations plugin from the Plugin Store first."),
        );
    }
    if !crate::panel_admin::is_panel_admin(&user) {
        return redirect_notice(
            "/backups/incremental",
            None,
            Some("Only panel admins can run incremental backup jobs."),
        );
    }
    let id = form.id.clone();
    let result = web::block(move || run_incremental_form(&id))
        .await
        .unwrap_or_else(|e| Err(format!("Incremental job task failed: {e}")));
    match result {
        Ok(msg) => redirect_notice("/backups/incremental", Some(&msg), None),
        Err(err) => redirect_notice("/backups/incremental", None, Some(&err)),
    }
}
