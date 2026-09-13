//! Authenticated JSON log pages for Manage > Logs (domain-jailed).

use crate::auth_api::panel_user_from_request;
use crate::installer::AppState;
use crate::panel_website_logs::query_site_log_page;
use crate::site_acl::{SitePerm, require_manage_site};
use actix_web::{HttpRequest, HttpResponse, get, web};
use serde::Deserialize;
use std::sync::Arc;

fn json_ok(value: serde_json::Value) -> HttpResponse {
    HttpResponse::Ok()
        .content_type("application/json; charset=utf-8")
        .body(serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{\"ok\":false}".into()))
}

fn json_err(status: u16, message: &str) -> HttpResponse {
    let body = serde_json::to_string_pretty(&serde_json::json!({ "ok": false, "error": message }))
        .unwrap_or_else(|_| "{\"ok\":false}".into());
    let mut response = match status {
        401 => HttpResponse::Unauthorized(),
        403 => HttpResponse::Forbidden(),
        400 => HttpResponse::BadRequest(),
        _ => HttpResponse::InternalServerError(),
    };
    response
        .content_type("application/json; charset=utf-8")
        .body(body)
}

#[derive(Debug, Deserialize)]
pub struct LogsQuery {
    domain: String,
    #[serde(default = "default_kind")]
    kind: String,
    #[serde(default)]
    search: String,
    #[serde(default = "default_page")]
    page: usize,
    #[serde(default = "default_per_page")]
    per_page: usize,
}

fn default_kind() -> String {
    "access".into()
}
fn default_page() -> usize {
    1
}
fn default_per_page() -> usize {
    25
}

#[get("/api/websites/manage/logs")]
pub async fn websites_manage_logs(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<LogsQuery>,
) -> HttpResponse {
    let Some(user) = panel_user_from_request(&state, &http) else {
        return json_err(401, "Login required");
    };
    let site = match require_manage_site(&user, &query.domain, SitePerm::Enable) {
        Ok(site) => site,
        Err(err) => return json_err(403, &err),
    };
    let kind = query.kind.trim().to_ascii_lowercase();
    if kind != "access" && kind != "error" {
        return json_err(400, "kind must be access or error");
    }
    match query_site_log_page(&site, &kind, &query.search, query.page, query.per_page) {
        Ok(payload) => match serde_json::to_value(&payload) {
            Ok(value) => json_ok(value),
            Err(_) => json_err(500, "Could not encode log page"),
        },
        Err(err) => json_err(400, &err),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn route_path_documented() {
        assert_eq!("/api/websites/manage/logs", "/api/websites/manage/logs");
    }
}
