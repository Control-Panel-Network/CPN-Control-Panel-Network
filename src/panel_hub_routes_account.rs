//! Users & Plans hub routes: accounts, ACL, and scaffolds.

use crate::account::{default_password_policy, load_bootstrap};
use crate::account_mgmt::{create_account, delete_account, reset_account_password};
use crate::installer::AppState;
use crate::packages::is_panel_admin;
use crate::panel_hub_http::{html_ok, login_redirect, redirect_notice, require_panel_user};
use crate::panel_hub_pages_account::{
    acl_create_page, acl_modify_page, api_access_page, grant_from_form_fields, users_create_page,
    users_create_success_page, users_list_page, users_modify_page, users_password_success_page,
    users_plans_hub_main, users_profile_page, users_reseller_page,
};
use crate::panel_pages::panel_shell;
use crate::site_acl::{add_grant, remove_grant_at};
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use std::sync::Arc;

fn require_admin(user: &str) -> Result<(), String> {
    if is_panel_admin(user) {
        Ok(())
    } else {
        Err("Only the panel admin can manage users and ACL".into())
    }
}

fn parse_flag(raw: &str) -> bool {
    matches!(
        raw.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

#[derive(Debug, serde::Deserialize)]
pub struct UserCreateForm {
    #[serde(default)]
    username: String,
    #[serde(default)]
    recovery_email: String,
    #[serde(default)]
    password: String,
    #[serde(default)]
    generate: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct UserModifyForm {
    #[serde(default)]
    username: String,
    #[serde(default)]
    password: String,
    #[serde(default)]
    generate: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct UserDeleteForm {
    #[serde(default)]
    username: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct AclGrantForm {
    #[serde(default)]
    member: String,
    #[serde(default)]
    domain: String,
    #[serde(default)]
    all_owned_by: String,
    #[serde(default)]
    can_install: String,
    #[serde(default)]
    can_uninstall: String,
    #[serde(default)]
    can_enable: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct AclDeleteForm {
    #[serde(default)]
    index: String,
}

#[get("/account/users")]
pub async fn users_plans_page(http: HttpRequest, state: web::Data<Arc<AppState>>) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    html_ok(panel_shell(
        &user,
        "users",
        "Users & Plans",
        &users_plans_hub_main(),
    ))
}

#[get("/account/users/profile")]
pub async fn users_profile_route(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    html_ok(panel_shell(
        &user,
        "users",
        "View Profile",
        &users_profile_page(&user),
    ))
}

#[get("/account/users/list")]
pub async fn users_list_route(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    html_ok(panel_shell(
        &user,
        "users",
        "List Users",
        &users_list_page(
            &user,
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}

#[get("/account/users/create")]
pub async fn users_create_get(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    if let Err(error) = require_admin(&user) {
        return redirect_notice("/account/users/list", None, Some(&error));
    }
    html_ok(panel_shell(
        &user,
        "users",
        "Create User",
        &users_create_page(
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}

#[post("/account/users/create")]
pub async fn users_create_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<UserCreateForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    if let Err(error) = require_admin(&user) {
        return redirect_notice("/account/users/list", None, Some(&error));
    }
    let generate = parse_flag(&form.generate) || form.password.trim().is_empty();
    let password = if generate {
        None
    } else {
        Some(form.password.as_str())
    };
    let language = load_bootstrap()
        .map(|b| b.language)
        .unwrap_or_else(|| "en".into());
    match create_account(
        &form.username,
        password,
        generate,
        &form.recovery_email,
        default_password_policy(),
        &language,
    ) {
        Ok(result) => html_ok(panel_shell(
            &user,
            "users",
            "Create User",
            &users_create_success_page(
                &result.public.username,
                result.generated_password.as_deref(),
            ),
        )),
        Err(error) => redirect_notice("/account/users/create", None, Some(&error)),
    }
}

#[get("/account/users/modify")]
pub async fn users_modify_get(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    if let Err(error) = require_admin(&user) {
        return redirect_notice("/account/users/list", None, Some(&error));
    }
    html_ok(panel_shell(
        &user,
        "users",
        "Modify User",
        &users_modify_page(
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}

#[post("/account/users/password")]
pub async fn users_password_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<UserModifyForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    if let Err(error) = require_admin(&user) {
        return redirect_notice("/account/users/list", None, Some(&error));
    }
    let generate = parse_flag(&form.generate) || form.password.trim().is_empty();
    let password = if generate {
        None
    } else {
        Some(form.password.as_str())
    };
    match reset_account_password(&form.username, password, generate) {
        Ok(result) => html_ok(panel_shell(
            &user,
            "users",
            "Modify User",
            &users_password_success_page(
                &result.public.username,
                result.generated_password.as_deref(),
            ),
        )),
        Err(error) => redirect_notice("/account/users/modify", None, Some(&error)),
    }
}

#[post("/account/users/delete")]
pub async fn users_delete_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<UserDeleteForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    if let Err(error) = require_admin(&user) {
        return redirect_notice("/account/users/list", None, Some(&error));
    }
    if is_panel_admin(form.username.trim()) {
        return redirect_notice(
            "/account/users/modify",
            None,
            Some("The bootstrap admin account cannot be deleted here"),
        );
    }
    match delete_account(&form.username) {
        Ok(()) => redirect_notice("/account/users/list", Some("Account deleted"), None),
        Err(error) => redirect_notice("/account/users/modify", None, Some(&error)),
    }
}

#[get("/account/users/reseller")]
pub async fn users_reseller_route(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    html_ok(panel_shell(
        &user,
        "users",
        "Reseller Center",
        &users_reseller_page(),
    ))
}

#[get("/account/api-access")]
pub async fn api_access_route(http: HttpRequest, state: web::Data<Arc<AppState>>) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    html_ok(panel_shell(
        &user,
        "users",
        "API Access",
        &api_access_page(),
    ))
}

#[get("/account/acl/create")]
pub async fn acl_create_get(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    if let Err(error) = require_admin(&user) {
        return redirect_notice("/account/users", None, Some(&error));
    }
    html_ok(panel_shell(
        &user,
        "users",
        "Create ACL",
        &acl_create_page(
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}

#[post("/account/acl/create")]
pub async fn acl_create_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<AclGrantForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    if let Err(error) = require_admin(&user) {
        return redirect_notice("/account/users", None, Some(&error));
    }
    let grant = grant_from_form_fields(
        &form.member,
        &form.domain,
        &form.all_owned_by,
        parse_flag(&form.can_install),
        parse_flag(&form.can_uninstall),
        parse_flag(&form.can_enable),
    );
    match add_grant(grant) {
        Ok(()) => redirect_notice("/account/acl/modify", Some("ACL grant saved"), None),
        Err(error) => redirect_notice("/account/acl/create", None, Some(&error)),
    }
}

#[get("/account/acl/modify")]
pub async fn acl_modify_get(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    if let Err(error) = require_admin(&user) {
        return redirect_notice("/account/users", None, Some(&error));
    }
    html_ok(panel_shell(
        &user,
        "users",
        "Modify ACL",
        &acl_modify_page(
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}

#[post("/account/acl/delete")]
pub async fn acl_delete_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<AclDeleteForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect();
    };
    if let Err(error) = require_admin(&user) {
        return redirect_notice("/account/users", None, Some(&error));
    }
    let index = match form.index.trim().parse::<usize>() {
        Ok(v) => v,
        Err(_) => {
            return redirect_notice("/account/acl/modify", None, Some("Invalid grant index"));
        }
    };
    match remove_grant_at(index) {
        Ok(()) => redirect_notice("/account/acl/modify", Some("ACL grant removed"), None),
        Err(error) => redirect_notice("/account/acl/modify", None, Some(&error)),
    }
}
