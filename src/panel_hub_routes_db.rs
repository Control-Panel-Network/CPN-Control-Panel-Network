//! Databases & FTP hub feature routes.

use crate::installer::AppState;
use crate::panel_db_acl::require_manage_database;
use crate::panel_hub_http::{html_ok, login_redirect, redirect_notice, require_panel_user};
use crate::panel_hub_pages_db_manage::{
    databases_all_page_for, databases_delete_page_for, databases_password_page_for,
};
use crate::panel_hub_pages_ftp::{
    ftp_accounts_page, ftp_create_page, ftp_delete_page, ftp_reset_page,
};
use crate::panel_hub_pages_hosting::{
    databases_create_page, databases_manager_page, phpmyadmin_page, run_create_database,
};
use crate::panel_ops_db::{
    change_database_user_password, drop_database_with_optional_users, is_protected_db_user,
};
use crate::panel_pages::panel_shell;
use crate::uninstall_confirm::{CONFIRM_REQUIRED_MSG, confirm_accepted};
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use std::sync::Arc;

#[get("/databases/all")]
pub async fn databases_all_route(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    html_ok(panel_shell(
        &user,
        "databases",
        "All Databases",
        &databases_all_page_for(
            &user,
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
        return login_redirect(&http);
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
        return login_redirect(&http);
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
        return login_redirect(&http);
    };
    html_ok(panel_shell(
        &user,
        "databases",
        "Delete Database",
        &databases_delete_page_for(
            &user,
            query.get("name").map(String::as_str),
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}

#[derive(Debug, serde::Deserialize)]
pub struct DbDeleteForm {
    #[serde(default)]
    name: String,
    #[serde(default)]
    confirm_name: String,
    #[serde(default)]
    confirm: String,
    #[serde(default)]
    drop_users: String,
}

#[post("/databases/delete")]
pub async fn databases_delete_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<DbDeleteForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let name = form.name.trim();
    let confirm_name = form.confirm_name.trim();
    if name.is_empty() {
        return redirect_notice("/databases/delete", None, Some("Database name is required."));
    }
    if confirm_name != name {
        return redirect_notice(
            &format!("/databases/delete?name={}", urlencoding_simple(name)),
            None,
            Some("Type the exact database name to confirm delete."),
        );
    }
    if !confirm_accepted(&form.confirm) {
        return redirect_notice(
            &format!("/databases/delete?name={}", urlencoding_simple(name)),
            None,
            Some(CONFIRM_REQUIRED_MSG),
        );
    }
    if let Err(err) = require_manage_database(&user, name) {
        return redirect_notice(
            &format!("/databases/delete?name={}", urlencoding_simple(name)),
            None,
            Some(&err),
        );
    }
    let drop_users = matches!(
        form.drop_users.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    );
    match drop_database_with_optional_users(name, drop_users) {
        Ok(msg) => {
            let _ = crate::resource_accounts::delete_database(name);
            redirect_notice("/databases/all", Some(&msg), None)
        }
        Err(err) => redirect_notice(
            &format!("/databases/delete?name={}", urlencoding_simple(name)),
            None,
            Some(&err),
        ),
    }
}

#[get("/databases/password")]
pub async fn databases_password_get(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    html_ok(panel_shell(
        &user,
        "databases",
        "Change database password",
        &databases_password_page_for(
            &user,
            query.get("name").map(String::as_str),
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}

#[derive(Debug, serde::Deserialize)]
pub struct DbPasswordForm {
    #[serde(default)]
    name: String,
    #[serde(default)]
    db_user: String,
    #[serde(default)]
    db_user_pick: String,
    #[serde(default)]
    password: String,
    #[serde(default)]
    password_confirm: String,
}

#[post("/databases/password")]
pub async fn databases_password_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<DbPasswordForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let name = form.name.trim();
    let db_user = if !form.db_user.trim().is_empty() {
        form.db_user.trim()
    } else {
        form.db_user_pick.trim()
    };
    let redirect = if name.is_empty() {
        "/databases/password".to_string()
    } else {
        format!("/databases/password?name={}", urlencoding_simple(name))
    };
    if name.is_empty() {
        return redirect_notice(&redirect, None, Some("Select a database."));
    }
    if db_user.is_empty() {
        return redirect_notice(&redirect, None, Some("Enter or pick a MariaDB username."));
    }
    if is_protected_db_user(db_user) {
        return redirect_notice(
            &redirect,
            None,
            Some("That MariaDB user is protected and cannot be changed here."),
        );
    }
    if form.password != form.password_confirm {
        return redirect_notice(&redirect, None, Some("Passwords do not match."));
    }
    if let Err(err) = require_manage_database(&user, name) {
        return redirect_notice(&redirect, None, Some(&err));
    }
    // Intentionally do not log form.password / password_confirm.
    match change_database_user_password(db_user, &form.password) {
        Ok(msg) => redirect_notice(&redirect, Some(&msg), None),
        Err(err) => redirect_notice(&redirect, None, Some(&err)),
    }
}

fn urlencoding_simple(value: &str) -> String {
    let mut out = String::new();
    for b in value.as_bytes() {
        match *b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[get("/databases/manager")]
pub async fn databases_manager_route(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
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
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    html_ok(panel_shell(
        &user,
        "databases",
        "phpMyAdmin",
        &phpmyadmin_page(
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}

#[get("/databases/phpmyadmin/open")]
pub async fn databases_phpmyadmin_open(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let domain = query.get("domain").map(|s| s.trim()).unwrap_or("");
    let open = if !domain.is_empty() {
        if let Err(err) =
            crate::site_acl::require_manage_site(&user, domain, crate::site_acl::SitePerm::Enable)
        {
            return redirect_notice("/databases/phpmyadmin", None, Some(&err));
        }
        crate::apps_phpmyadmin_sso::open_phpmyadmin_autologin_for_domain(domain)
    } else if crate::panel_admin::is_panel_admin(&user) {
        crate::apps_phpmyadmin_sso::open_phpmyadmin_autologin()
    } else {
        Err(
            "Select a domain to open phpMyAdmin with a jailed database list, or ask the panel admin for Host open."
                .into(),
        )
    };
    match open {
        Ok(url) => {
            let secure = crate::panel_session::request_https_from_headers(&http);
            let mut builder = HttpResponse::SeeOther();
            builder.append_header(("Location", url.as_str()));
            for cookie in crate::panel_phpmyadmin_proxy::clear_phpmyadmin_cookie_headers(secure) {
                builder.append_header(("Set-Cookie", cookie));
            }
            builder.finish()
        }
        Err(err) => redirect_notice("/databases/phpmyadmin", None, Some(&err)),
    }
}

#[get("/ftp/accounts")]
pub async fn ftp_accounts_route(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
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
        return login_redirect(&http);
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
        return login_redirect(&http);
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
        return login_redirect(&http);
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
        return login_redirect(&http);
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
        return login_redirect(&http);
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
pub async fn ftp_reset_post(http: HttpRequest, state: web::Data<Arc<AppState>>) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
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
        return login_redirect(&http);
    };
    match crate::panel_ops_sftp::reset_sftp_password(&form.username, &form.password) {
        Ok(msg) => redirect_notice("/ftp/reset", Some(&msg), None),
        Err(err) => redirect_notice("/ftp/reset", None, Some(&err)),
    }
}
