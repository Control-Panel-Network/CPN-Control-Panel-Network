//! Routes for mailing lists, autoresponders, and email filters.

use crate::installer::AppState;
use crate::mail_postfix_maps::owner_for_mail_domain;
use crate::packages::{QuotaResource, require_quota};
use crate::panel_hub_http::{html_ok, login_redirect, redirect_notice, require_panel_user};
use crate::panel_hub_pages_email_features::{
    autoresponders_page, email_filters_page, mailing_lists_page,
};
use crate::panel_ops_email_acl::require_email_csrf;
use crate::panel_ops_mail_autorespond::{
    list_autoresponders, remove_autoresponder, upsert_autoresponder,
};
use crate::panel_ops_mail_filters::{add_filter, remove_filter};
use crate::panel_ops_mail_lists::{
    add_list_member, apply_list_maps, create_mailing_list, delete_mailing_list, remove_list_member,
};
use crate::panel_pages::panel_shell;
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use std::collections::HashMap;
use std::sync::Arc;

fn owner_for_address(actor: &str, address: &str) -> String {
    address
        .rsplit_once('@')
        .and_then(|(_, d)| owner_for_mail_domain(d))
        .unwrap_or_else(|| actor.to_string())
}

#[get("/email/autoresponders")]
pub async fn email_autoresponders_route(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    html_ok(panel_shell(
        &user,
        "email",
        "Autoresponders",
        &autoresponders_page(
            &user,
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}

#[post("/email/autoresponders/save")]
pub async fn email_autoresponders_save(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Some(resp) = require_email_csrf(&http, &user, &form, "/email/autoresponders") {
        return resp;
    }
    let address = form.get("address").map(String::as_str).unwrap_or("");
    let subject = form.get("subject").map(String::as_str).unwrap_or("");
    let body = form.get("body").map(String::as_str).unwrap_or("");
    let enabled = form
        .get("enabled")
        .map(|v| matches!(v.as_str(), "1" | "true" | "on" | "yes"))
        .unwrap_or(false);
    let is_new = !list_autoresponders()
        .iter()
        .any(|a| a.address.eq_ignore_ascii_case(address.trim()));
    let owner = owner_for_address(&user, address);
    let result = if is_new {
        require_quota(&owner, QuotaResource::Autoresponders)
            .and_then(|_| upsert_autoresponder(&user, address, subject, body, enabled))
    } else {
        upsert_autoresponder(&user, address, subject, body, enabled)
    };
    match result {
        Ok(msg) => redirect_notice("/email/autoresponders", Some(&msg), None),
        Err(err) => redirect_notice("/email/autoresponders", None, Some(&err)),
    }
}

#[post("/email/autoresponders/delete")]
pub async fn email_autoresponders_delete(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Some(resp) = require_email_csrf(&http, &user, &form, "/email/autoresponders") {
        return resp;
    }
    let id = form.get("id").map(String::as_str).unwrap_or("");
    match remove_autoresponder(&user, id) {
        Ok(msg) => redirect_notice("/email/autoresponders", Some(&msg), None),
        Err(err) => redirect_notice("/email/autoresponders", None, Some(&err)),
    }
}

#[get("/email/filters")]
pub async fn email_filters_route(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    html_ok(panel_shell(
        &user,
        "email",
        "Email Filters",
        &email_filters_page(
            &user,
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}

#[post("/email/filters/save")]
pub async fn email_filters_save(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Some(resp) = require_email_csrf(&http, &user, &form, "/email/filters") {
        return resp;
    }
    let address = form.get("address").map(String::as_str).unwrap_or("");
    let match_field = form.get("match_field").map(String::as_str).unwrap_or("");
    let match_value = form.get("match_value").map(String::as_str).unwrap_or("");
    let action = form.get("action").map(String::as_str).unwrap_or("");
    let action_arg = form.get("action_arg").map(String::as_str).unwrap_or("");
    let owner = owner_for_address(&user, address);
    match require_quota(&owner, QuotaResource::EmailFilters)
        .and_then(|_| add_filter(&user, address, match_field, match_value, action, action_arg))
    {
        Ok(msg) => redirect_notice("/email/filters", Some(&msg), None),
        Err(err) => redirect_notice("/email/filters", None, Some(&err)),
    }
}

#[post("/email/filters/delete")]
pub async fn email_filters_delete(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Some(resp) = require_email_csrf(&http, &user, &form, "/email/filters") {
        return resp;
    }
    let id = form.get("id").map(String::as_str).unwrap_or("");
    match remove_filter(&user, id) {
        Ok(msg) => redirect_notice("/email/filters", Some(&msg), None),
        Err(err) => redirect_notice("/email/filters", None, Some(&err)),
    }
}

#[get("/email/lists")]
pub async fn email_lists_route(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    html_ok(panel_shell(
        &user,
        "email",
        "Mailing Lists",
        &mailing_lists_page(
            &user,
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
        ),
    ))
}

#[post("/email/lists/create")]
pub async fn email_lists_create(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Some(resp) = require_email_csrf(&http, &user, &form, "/email/lists") {
        return resp;
    }
    let address = form.get("address").map(String::as_str).unwrap_or("");
    let name = form.get("name").map(String::as_str).unwrap_or("");
    let owner = owner_for_address(&user, address);
    match require_quota(&owner, QuotaResource::MailingLists)
        .and_then(|_| create_mailing_list(&user, address, name))
    {
        Ok(msg) => redirect_notice("/email/lists", Some(&msg), None),
        Err(err) => redirect_notice("/email/lists", None, Some(&err)),
    }
}

#[post("/email/lists/member/add")]
pub async fn email_lists_member_add(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Some(resp) = require_email_csrf(&http, &user, &form, "/email/lists") {
        return resp;
    }
    let list_id = form.get("list_id").map(String::as_str).unwrap_or("");
    let member = form.get("member").map(String::as_str).unwrap_or("");
    match add_list_member(&user, list_id, member) {
        Ok(msg) => redirect_notice("/email/lists", Some(&msg), None),
        Err(err) => redirect_notice("/email/lists", None, Some(&err)),
    }
}

#[post("/email/lists/member/delete")]
pub async fn email_lists_member_delete(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Some(resp) = require_email_csrf(&http, &user, &form, "/email/lists") {
        return resp;
    }
    let list_id = form.get("list_id").map(String::as_str).unwrap_or("");
    let member = form.get("member").map(String::as_str).unwrap_or("");
    match remove_list_member(&user, list_id, member) {
        Ok(msg) => redirect_notice("/email/lists", Some(&msg), None),
        Err(err) => redirect_notice("/email/lists", None, Some(&err)),
    }
}

#[post("/email/lists/delete")]
pub async fn email_lists_delete(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Some(resp) = require_email_csrf(&http, &user, &form, "/email/lists") {
        return resp;
    }
    let list_id = form.get("list_id").map(String::as_str).unwrap_or("");
    match delete_mailing_list(&user, list_id) {
        Ok(msg) => redirect_notice("/email/lists", Some(&msg), None),
        Err(err) => redirect_notice("/email/lists", None, Some(&err)),
    }
}

#[post("/email/lists/apply")]
pub async fn email_lists_apply(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Some(resp) = require_email_csrf(&http, &user, &form, "/email/lists") {
        return resp;
    }
    match apply_list_maps() {
        Ok(msg) => redirect_notice("/email/lists", Some(&msg), None),
        Err(err) => redirect_notice("/email/lists", None, Some(&err)),
    }
}
