//! Routes for sidebar visibility ACL grants.

use crate::installer::AppState;
use crate::packages::is_panel_admin;
use crate::panel_hub_http::{html_ok, login_redirect, redirect_notice, require_panel_user};
use crate::panel_hub_pages_sidebar_acl::sidebar_acl_standalone_page;
use crate::panel_pages::panel_shell;
use crate::sidebar_visibility::{remove_grant_at, set_grant_for_member};
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use std::sync::Arc;

fn require_admin(user: &str) -> Result<(), String> {
    if is_panel_admin(user) {
        Ok(())
    } else {
        Err("Only the panel admin can manage sidebar visibility ACL".into())
    }
}

fn parse_flag(raw: &str) -> bool {
    matches!(
        raw.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn collect_hidden_ids(pairs: &[(String, String)]) -> Vec<String> {
    pairs
        .iter()
        .filter(|(k, _)| k == "hidden_nav_ids")
        .map(|(_, v)| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .collect()
}

#[get("/account/acl/sidebar")]
pub async fn sidebar_acl_get(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Err(error) = require_admin(&user) {
        return redirect_notice("/account/users", None, Some(&error));
    }
    html_ok(panel_shell(
        &user,
        "users",
        "Sidebar visibility",
        &sidebar_acl_standalone_page(
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}

#[post("/account/acl/sidebar")]
pub async fn sidebar_acl_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<Vec<(String, String)>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Err(error) = require_admin(&user) {
        return redirect_notice("/account/users", None, Some(&error));
    }
    let member = form
        .iter()
        .find(|(k, _)| k == "member")
        .map(|(_, v)| v.as_str())
        .unwrap_or("");
    let restrict = form
        .iter()
        .find(|(k, _)| k == "restrict_admin")
        .map(|(_, v)| parse_flag(v))
        .unwrap_or(false);
    let hidden = collect_hidden_ids(&form);
    match set_grant_for_member(member, hidden, restrict) {
        Ok(()) => redirect_notice(
            "/account/acl/modify",
            Some("Sidebar visibility grant saved"),
            None,
        ),
        Err(error) => redirect_notice("/account/acl/sidebar", None, Some(&error)),
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct SidebarAclDeleteForm {
    #[serde(default)]
    index: String,
}

#[post("/account/acl/sidebar/delete")]
pub async fn sidebar_acl_delete_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<SidebarAclDeleteForm>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Err(error) = require_admin(&user) {
        return redirect_notice("/account/users", None, Some(&error));
    }
    let index = match form.index.trim().parse::<usize>() {
        Ok(v) => v,
        Err(_) => {
            return redirect_notice(
                "/account/acl/modify",
                None,
                Some("Invalid sidebar grant index"),
            );
        }
    };
    match remove_grant_at(index) {
        Ok(()) => redirect_notice(
            "/account/acl/modify",
            Some("Sidebar visibility grant removed"),
            None,
        ),
        Err(error) => redirect_notice("/account/acl/modify", None, Some(&error)),
    }
}
