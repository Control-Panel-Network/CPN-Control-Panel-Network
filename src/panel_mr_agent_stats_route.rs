//! `GET /plugins/mr-agent/stats` privacy-safe metrics for Host or Site.

use crate::auth_api::panel_user_from_request;
use crate::installer::AppState;
use crate::login_next::login_redirect;
use crate::mr_agent_install::{HOST_DOMAIN_SENTINEL, is_mr_agent};
use crate::mr_agent_stats::collect_stats;
use crate::panel_admin::is_panel_admin;
use crate::site_acl::{SitePerm, can_manage_site};
use actix_web::{HttpRequest, HttpResponse, get, web};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct MrAgentStatsQuery {
    #[serde(default)]
    pub domain: String,
    #[serde(default)]
    pub id: String,
}

fn authz(user: &str, domain: &str) -> Result<(), String> {
    if domain.trim().eq_ignore_ascii_case(HOST_DOMAIN_SENTINEL) {
        if is_panel_admin(user) {
            return Ok(());
        }
        return Err("Only panel administrators can view Host Mr Agent statistics".into());
    }
    if matches!(can_manage_site(user, domain, SitePerm::Enable), Ok(true)) {
        return Ok(());
    }
    Err("Not allowed to view Mr Agent statistics for this site".into())
}

#[get("/plugins/mr-agent/stats")]
pub async fn plugins_mr_agent_stats(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<MrAgentStatsQuery>,
) -> HttpResponse {
    let Some(user) = panel_user_from_request(&state, &http) else {
        return login_redirect(&http);
    };
    if !query.id.trim().is_empty() && !is_mr_agent(&query.id) {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "ok": false,
            "error": "Statistics are only for Mr Agent",
        }));
    }
    let domain = {
        let d = query.domain.trim();
        if d.is_empty() {
            HOST_DOMAIN_SENTINEL
        } else {
            d
        }
    };
    if let Err(err) = authz(&user, domain) {
        return HttpResponse::Forbidden().json(serde_json::json!({
            "ok": false,
            "error": err,
        }));
    }
    let stats = collect_stats(domain);
    HttpResponse::Ok()
        .content_type("application/json; charset=utf-8")
        .json(stats)
}
