//! FTP and Dropbox destination routes (requires backupDestinations Host plugin).

use crate::installer::AppState;
use crate::panel_hub_http::{
    authenticated_panel_shell, html_ok, login_redirect, redirect_notice, require_panel_user,
};
use crate::panel_hub_pages_backups_dest::parse_flag;
use crate::panel_hub_pages_backups_dest_more::{
    backups_dropbox_page, backups_ftp_page, run_dropbox_test, run_ftp_test, save_dropbox_form,
    save_ftp_form,
};
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use std::sync::Arc;

#[get("/backups/ftp")]
pub async fn backups_ftp_route(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !crate::panel_feature_gate::backup_destinations_unlocked() {
        return html_ok(authenticated_panel_shell(
            &http,
            &user,
            "backups",
            "FTP",
            &crate::panel_feature_gate::backup_destinations_plugin_required_page("FTP"),
        ));
    }
    html_ok(authenticated_panel_shell(
        &http,
        &user,
        "backups",
        "FTP",
        &backups_ftp_page(
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}

#[derive(Debug, serde::Deserialize)]
pub struct FtpSaveForm {
    #[serde(default)]
    enabled: String,
    #[serde(default)]
    host: String,
    #[serde(default)]
    port: String,
    #[serde(default)]
    username: String,
    #[serde(default)]
    password: String,
    #[serde(default)]
    remote_path: String,
    #[serde(default)]
    passive: String,
    #[serde(default)]
    note: String,
}

#[post("/backups/ftp/save")]
pub async fn backups_ftp_save(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<FtpSaveForm>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !crate::panel_feature_gate::backup_destinations_unlocked() {
        return redirect_notice(
            "/backups/ftp",
            None,
            Some("Install the free backupDestinations plugin from the Plugin Store first."),
        );
    }
    let port = form.port.trim().parse::<u16>().unwrap_or(21);
    match save_ftp_form(
        parse_flag(&form.enabled),
        &form.host,
        port,
        &form.username,
        &form.password,
        &form.remote_path,
        parse_flag(&form.passive),
        &form.note,
    ) {
        Ok(msg) => redirect_notice("/backups/ftp", Some(&msg), None),
        Err(err) => redirect_notice("/backups/ftp", None, Some(&err)),
    }
}

#[post("/backups/ftp/test")]
pub async fn backups_ftp_test(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !crate::panel_feature_gate::backup_destinations_unlocked() {
        return redirect_notice(
            "/backups/ftp",
            None,
            Some("Install the free backupDestinations plugin from the Plugin Store first."),
        );
    }
    match run_ftp_test() {
        Ok(msg) => redirect_notice("/backups/ftp", Some(&msg), None),
        Err(err) => redirect_notice("/backups/ftp", None, Some(&err)),
    }
}

#[get("/backups/dropbox")]
pub async fn backups_dropbox_route(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !crate::panel_feature_gate::backup_destinations_unlocked() {
        return html_ok(authenticated_panel_shell(
            &http,
            &user,
            "backups",
            "Dropbox",
            &crate::panel_feature_gate::backup_destinations_plugin_required_page("Dropbox"),
        ));
    }
    html_ok(authenticated_panel_shell(
        &http,
        &user,
        "backups",
        "Dropbox",
        &backups_dropbox_page(
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}

#[derive(Debug, serde::Deserialize)]
pub struct DropboxSaveForm {
    #[serde(default)]
    enabled: String,
    #[serde(default)]
    app_key: String,
    #[serde(default)]
    app_secret: String,
    #[serde(default)]
    refresh_token: String,
    #[serde(default)]
    folder_path: String,
    #[serde(default)]
    note: String,
}

#[post("/backups/dropbox/save")]
pub async fn backups_dropbox_save(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<DropboxSaveForm>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !crate::panel_feature_gate::backup_destinations_unlocked() {
        return redirect_notice(
            "/backups/dropbox",
            None,
            Some("Install the free backupDestinations plugin from the Plugin Store first."),
        );
    }
    match save_dropbox_form(
        parse_flag(&form.enabled),
        &form.app_key,
        &form.app_secret,
        &form.refresh_token,
        &form.folder_path,
        &form.note,
    ) {
        Ok(msg) => redirect_notice("/backups/dropbox", Some(&msg), None),
        Err(err) => redirect_notice("/backups/dropbox", None, Some(&err)),
    }
}

#[post("/backups/dropbox/test")]
pub async fn backups_dropbox_test(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !crate::panel_feature_gate::backup_destinations_unlocked() {
        return redirect_notice(
            "/backups/dropbox",
            None,
            Some("Install the free backupDestinations plugin from the Plugin Store first."),
        );
    }
    match run_dropbox_test() {
        Ok(msg) => redirect_notice("/backups/dropbox", Some(&msg), None),
        Err(err) => redirect_notice("/backups/dropbox", None, Some(&err)),
    }
}
