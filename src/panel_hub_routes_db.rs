//! Databases & FTP hub feature routes.

use crate::installer::AppState;
use crate::panel_hub_http::{html_ok, login_redirect, redirect_notice, require_panel_user};
use crate::panel_hub_pages_ftp::{
    ftp_accounts_page, ftp_create_page, ftp_delete_page, ftp_reset_page,
};
use crate::panel_hub_pages_hosting::{
    databases_all_page, databases_create_page, databases_delete_page, databases_manager_page,
    phpmyadmin_page, run_create_database, run_drop_database,
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
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    html_ok(panel_shell(
        &user,
        "databases",
        "SFTP Accounts",
        &ftp_accounts_page(
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}

#[derive(Debug, serde::Deserialize)]
pub struct FtpCreateForm {
    #[serde(default)]
    domain: String,
    #[serde(default)]
    username: String,
    #[serde(default)]
    password: String,
}

#[get("/ftp/create")]
pub async fn ftp_create(
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
        "Create SFTP Account",
        &ftp_create_page(
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}

#[post("/ftp/create")]
pub async fn ftp_create_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<FtpCreateForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    match crate::panel_ops_sftp::create_jailed_sftp_account(
        &user,
        &form.username,
        &form.domain,
        &form.password,
    ) {
        Ok(acct) => redirect_notice(
            "/ftp/accounts",
            Some(&format!(
                "Created jailed SFTP user `{}` for `{}`. Password was set (not shown again).",
                acct.username, acct.domain
            )),
            None,
        ),
        Err(err) => redirect_notice("/ftp/create", None, Some(&err)),
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct FtpUserForm {
    #[serde(default)]
    username: String,
}

#[get("/ftp/delete")]
pub async fn ftp_delete(
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
        "Delete SFTP Account",
        &ftp_delete_page(
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}

#[post("/ftp/delete")]
pub async fn ftp_delete_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<FtpUserForm>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    match crate::panel_ops_sftp::delete_jailed_sftp_account(&form.username) {
        Ok(msg) => redirect_notice("/ftp/accounts", Some(&msg), None),
        Err(err) => redirect_notice("/ftp/delete", None, Some(&err)),
    }
}

#[get("/ftp/reset")]
pub async fn ftp_reset(
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
        "Reset SFTP",
        &ftp_reset_page(
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}

#[post("/ftp/reset")]
pub async fn ftp_reset_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    match crate::panel_ops_sftp::ensure_sftp_stack() {
        Ok(msg) => redirect_notice("/ftp/reset", Some(&msg), None),
        Err(err) => redirect_notice("/ftp/reset", None, Some(&err)),
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct FtpPasswordForm {
    #[serde(default)]
    username: String,
    #[serde(default)]
    password: String,
}

#[post("/ftp/reset-password")]
pub async fn ftp_reset_password_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<FtpPasswordForm>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    match crate::panel_ops_sftp::reset_sftp_password(&form.username, &form.password) {
        Ok(msg) => redirect_notice("/ftp/reset", Some(&msg), None),
        Err(err) => redirect_notice("/ftp/reset", None, Some(&err)),
    }
}
