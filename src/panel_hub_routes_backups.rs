//! Backups hub feature routes.

use crate::backup_restore::{RestoreRequest, restore_backup};
use crate::installer::AppState;
use crate::panel_backups::BackupsPageQuery;
use crate::panel_hub_http::{
    html_ok, login_redirect, redirect_notice, require_panel_user, urlencoding_simple,
};
use crate::panel_hub_pages_backups::{
    backups_create_page, backups_destinations_page, backups_restore_page, backups_schedule_page,
    save_backup_destinations, save_backup_schedule,
};
use crate::panel_hub_pages_backups_plan::backups_restore_plan_page;
use crate::panel_hub_pages_hosting::scaffold_feature;
use crate::panel_pages::panel_shell;
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use serde::Deserializer;
use std::sync::Arc;

/// Accept a lone form value or repeated keys for multi-select `entity`.
fn deserialize_string_or_vec<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    struct V;
    impl<'de> serde::de::Visitor<'de> for V {
        type Value = Vec<String>;

        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("a string or a sequence of strings")
        }

        fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
            if v.trim().is_empty() {
                Ok(Vec::new())
            } else {
                Ok(vec![v.to_string()])
            }
        }

        fn visit_string<E: serde::de::Error>(self, v: String) -> Result<Self::Value, E> {
            self.visit_str(&v)
        }

        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut seq: A,
        ) -> Result<Self::Value, A::Error> {
            let mut out = Vec::new();
            while let Some(item) = seq.next_element::<String>()? {
                if !item.trim().is_empty() {
                    out.push(item);
                }
            }
            Ok(out)
        }

        fn visit_none<E: serde::de::Error>(self) -> Result<Self::Value, E> {
            Ok(Vec::new())
        }

        fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
            Ok(Vec::new())
        }
    }
    deserializer.deserialize_any(V)
}

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
    html_ok(panel_shell(
        &user,
        "backups",
        "Restore Backup",
        &backups_restore_page(
            query.get("scope").map(String::as_str).unwrap_or("site"),
            query.get("domain").map(String::as_str).unwrap_or(""),
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
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
    html_ok(panel_shell(
        &user,
        "backups",
        "Select restore entities",
        &backups_restore_plan_page(
            query.get("scope").map(String::as_str).unwrap_or("site"),
            query.get("domain").map(String::as_str).unwrap_or(""),
            archive,
            query.get("format").map(String::as_str).unwrap_or("auto"),
            query.get("db_name").map(String::as_str).unwrap_or(""),
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
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
    /// Multi-select entity ids (`website`, `db:name`, `site:x`, …).
    /// Browsers may post one value or many; accept both (avoids Actix parse 400).
    #[serde(default, deserialize_with = "deserialize_string_or_vec")]
    entity: Vec<String>,
    /// Comma-separated fallback when a client posts a single field.
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
    let mut entity_ids = form.entity.clone();
    if entity_ids.is_empty() && !form.entities.trim().is_empty() {
        entity_ids = form
            .entities
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
    }
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
        entity_ids,
    );
    req.owner = user.clone();
    match restore_backup(&req) {
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
    fn restore_run_form_accepts_single_entity_string() {
        let form: RestoreRunForm =
            serde_json::from_str(r#"{"entity":"site:newstargeted.com","archive":"a.tar.gz"}"#)
                .expect("single entity");
        assert_eq!(form.entity, vec!["site:newstargeted.com".to_string()]);
        assert_eq!(form.archive, "a.tar.gz");
    }

    #[test]
    fn restore_run_form_accepts_repeated_entity_keys() {
        // Form posts map repeated keys to a sequence; JSON array exercises the same path.
        let form: RestoreRunForm = serde_json::from_str(
            r#"{"entity":["website","db:news_disco"],"entities":"website,db:news_disco"}"#,
        )
        .expect("multi entity");
        assert_eq!(
            form.entity,
            vec!["website".to_string(), "db:news_disco".to_string()]
        );
        assert!(form.entities.contains("website"));
    }
}
