//! Databases & FTP hub feature routes.

use crate::installer::AppState;
use crate::panel_hub_http::{html_ok, login_redirect, redirect_notice, require_panel_user};
use crate::panel_hub_pages_hosting::{
    databases_all_page, databases_create_page, databases_delete_page, databases_manager_page,
    ftp_accounts_page, phpmyadmin_page, run_create_database, run_drop_database, scaffold_feature,
};
use crate::panel_pages::panel_shell;
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use std::sync::Arc;

#[get("/databases/all")]
pub async fn databases_all_route(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    html_ok(panel_shell(
        &user,
        "databases",
        "All Databases",
        &databases_all_page(
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}

#[get("/databases/create")]
pub async fn databases_create_get(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    html_ok(panel_shell(
        &user,
        "databases",
        "Create Database",
        &databases_create_page(
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}

#[derive(Debug, serde::Deserialize)]
pub struct DbNameForm {
    #[serde(default)]
    name: String,
}

#[post("/databases/create")]
pub async fn databases_create_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<DbNameForm>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    match run_create_database(&form.name) {
        Ok(msg) => redirect_notice("/databases/create", Some(&msg), None),
        Err(err) => redirect_notice("/databases/create", None, Some(&err)),
    }
}

#[get("/databases/delete")]
pub async fn databases_delete_get(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    html_ok(panel_shell(
        &user,
        "databases",
        "Delete Database",
        &databases_delete_page(
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}

#[post("/databases/delete")]
pub async fn databases_delete_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<DbNameForm>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    match run_drop_database(&form.name) {
        Ok(msg) => redirect_notice("/databases/delete", Some(&msg), None),
        Err(err) => redirect_notice("/databases/delete", None, Some(&err)),
    }
}

#[get("/databases/manager")]
pub async fn databases_manager_route(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    html_ok(panel_shell(
        &user,
        "databases",
        "MariaDB Manager",
        &databases_manager_page(
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}

#[get("/databases/phpmyadmin")]
pub async fn databases_phpmyadmin_route(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    html_ok(panel_shell(
        &user,
        "databases",
        "phpMyAdmin",
        &phpmyadmin_page(),
    ))
}

#[get("/ftp/accounts")]
pub async fn ftp_accounts_route(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    html_ok(panel_shell(
        &user,
        "databases",
        "FTP Accounts",
        &ftp_accounts_page(),
    ))
}

macro_rules! ftp_scaffold {
    ($name:ident, $path:literal, $title:literal, $sub:literal, $detail:literal) => {
        #[get($path)]
        pub async fn $name(http: HttpRequest, state: web::Data<Arc<AppState>>) -> HttpResponse {
            let Some(user) = require_panel_user(&state, &http) else {
                return login_redirect();
            };
            html_ok(panel_shell(
                &user,
                "databases",
                $title,
                &scaffold_feature("Databases & FTP", "/databases", $title, $sub, $detail),
            ))
        }
    };
}

ftp_scaffold!(
    ftp_create,
    "/ftp/create",
    "Create FTP Account",
    "Add an FTP user",
    "FTP account creation is not wired yet."
);
ftp_scaffold!(
    ftp_delete,
    "/ftp/delete",
    "Delete FTP Account",
    "Remove an FTP user",
    "FTP account deletion is not wired yet."
);
ftp_scaffold!(
    ftp_reset,
    "/ftp/reset",
    "Reset FTP",
    "Reset configuration",
    "FTP reset is not wired yet."
);
