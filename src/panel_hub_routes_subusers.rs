//! Sub-users routes (list/create/update capabilities/delete).

use crate::installer::AppState;
use crate::panel_hub_admin_gate::{ADMIN_ONLY_CODE, admin_only_html};
use crate::panel_hub_http::{html_ok, login_redirect, redirect_notice, require_panel_user};
use crate::panel_hub_pages_subusers::users_subusers_page;
use crate::panel_pages::panel_shell;
use crate::panel_site_tools_security::same_origin_ok;
use crate::panel_subusers::{
    can_manage_subusers, create_subuser, delete_subuser, update_subuser_capabilities,
};
use crate::panel_subusers_csrf::verify_subusers_csrf;
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use std::sync::Arc;

fn require_owner_csrf(http: &HttpRequest, user: &str, token: &str) -> Result<(), String> {
    if !can_manage_subusers(user) {
        return Err(ADMIN_ONLY_CODE.into());
    }
    if !same_origin_ok(http) {
        return Err("Request origin was rejected".into());
    }
    if !verify_subusers_csrf(user, token) {
        return Err("CSRF check failed; reload the page and try again".into());
    }
    Ok(())
}

fn collect_allowed(pairs: &[(String, String)]) -> Vec<String> {
    pairs
        .iter()
        .filter(|(k, _)| k == "allowed_nav_ids")
        .map(|(_, v)| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .collect()
}

fn form_value<'a>(pairs: &'a [(String, String)], key: &str) -> &'a str {
    pairs
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
        .unwrap_or("")
}

#[get("/account/users/sub-users")]
pub async fn users_subusers_get(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !can_manage_subusers(&user) {
        return admin_only_html(&user, "Sub-users");
    }
    html_ok(panel_shell(
        &user,
        "users",
        "Sub-users",
        &users_subusers_page(
            &user,
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
            None,
        ),
    ))
}

#[post("/account/users/sub-users/create")]
pub async fn users_subusers_create_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<Vec<(String, String)>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Err(error) = require_owner_csrf(&http, &user, form_value(&form, "csrf")) {
        return redirect_notice("/account/users/sub-users", None, Some(&error));
    }
    let username = form_value(&form, "username");
    let email = form_value(&form, "recovery_email");
    let password = form_value(&form, "password");
    let allowed = collect_allowed(&form);
    match create_subuser(
        &user,
        username,
        email,
        if password.trim().is_empty() {
            None
        } else {
            Some(password)
        },
        password.trim().is_empty(),
        &allowed,
    ) {
        Ok((created, generated)) => {
            if let Some(password) = generated {
                return html_ok(panel_shell(
                    &user,
                    "users",
                    "Sub-users",
                    &users_subusers_page(
                        &user,
                        Some(&format!("Sub-user `{}` created.", created.username)),
                        None,
                        Some(&password),
                    ),
                ));
            }
            redirect_notice(
                "/account/users/sub-users",
                Some(&format!("Sub-user `{}` created.", created.username)),
                None,
            )
        }
        Err(error) => redirect_notice("/account/users/sub-users", None, Some(&error)),
    }
}

#[post("/account/users/sub-users/capabilities")]
pub async fn users_subusers_capabilities_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<Vec<(String, String)>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Err(error) = require_owner_csrf(&http, &user, form_value(&form, "csrf")) {
        return redirect_notice("/account/users/sub-users", None, Some(&error));
    }
    let username = form_value(&form, "username");
    let allowed = collect_allowed(&form);
    match update_subuser_capabilities(&user, username, &allowed) {
        Ok(updated) => redirect_notice(
            "/account/users/sub-users",
            Some(&format!("Capabilities updated for `{}`.", updated.username)),
            None,
        ),
        Err(error) => redirect_notice("/account/users/sub-users", None, Some(&error)),
    }
}

#[post("/account/users/sub-users/delete")]
pub async fn users_subusers_delete_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<Vec<(String, String)>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Err(error) = require_owner_csrf(&http, &user, form_value(&form, "csrf")) {
        return redirect_notice("/account/users/sub-users", None, Some(&error));
    }
    let username = form_value(&form, "username");
    match delete_subuser(&user, username) {
        Ok(()) => redirect_notice(
            "/account/users/sub-users",
            Some(&format!("Sub-user `{username}` deleted.")),
            None,
        ),
        Err(error) => redirect_notice("/account/users/sub-users", None, Some(&error)),
    }
}
