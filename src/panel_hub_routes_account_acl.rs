//! Admin ACL routes: site permission grants (create, modify, delete).

use crate::installer::AppState;
use crate::panel_hub_admin_gate::{ADMIN_ONLY_CODE, admin_only_html};
use crate::panel_hub_http::{html_ok, login_redirect, redirect_notice, require_panel_user};
use crate::panel_hub_pages_account::{acl_create_page, acl_modify_page, grant_from_form_fields};
use crate::panel_hub_routes_account::{parse_flag, require_admin};
use crate::panel_pages::panel_shell;
use crate::site_acl::{add_grant, remove_grant_at};
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use std::sync::Arc;

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

#[get("/account/acl/create")]
pub async fn acl_create_get(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if require_admin(&user).is_err() {
        return admin_only_html(&user, "Create ACL");
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
        return login_redirect(&http);
    };
    if require_admin(&user).is_err() {
        return redirect_notice("/account/users", None, Some(ADMIN_ONLY_CODE));
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
        return login_redirect(&http);
    };
    if require_admin(&user).is_err() {
        return admin_only_html(&user, "Modify ACL");
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
        return login_redirect(&http);
    };
    if require_admin(&user).is_err() {
        return redirect_notice("/account/users", None, Some(ADMIN_ONLY_CODE));
    }
    let index = match form.index.trim().parse::<usize>() {
        Ok(v) => v,
        Err(_) => {
            return redirect_notice("/account/acl/modify", None, Some("invalid-grant-index"));
        }
    };
    match remove_grant_at(index) {
        Ok(()) => redirect_notice("/account/acl/modify", Some("ACL grant removed"), None),
        Err(error) => redirect_notice("/account/acl/modify", None, Some(&error)),
    }
}
