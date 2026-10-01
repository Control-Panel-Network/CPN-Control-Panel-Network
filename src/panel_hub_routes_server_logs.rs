//! Server > Logs route (panel activity plus links to every other log).

use crate::installer::AppState;
use crate::panel_hub_http::{html_blocking, login_redirect, require_panel_user};
use crate::panel_hub_pages_server_log_view::host_log_page;
use crate::panel_hub_pages_server_logs::server_logs_page;
use crate::panel_pages::panel_shell;
use crate::panel_server_logs::{DEFAULT_PER_PAGE, HostLogKind};
use actix_web::{HttpRequest, HttpResponse, get, web};
use std::sync::Arc;

#[get("/server/logs")]
pub async fn server_logs_route(http: HttpRequest, state: web::Data<Arc<AppState>>) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    html_blocking(move || panel_shell(&user, "server", "Logs", &server_logs_page(&user))).await
}

/// One searchable, paginated page per host log (`/server/logs/panel`, `access`, `error`, `email`,
/// `ftp`, `modsec`). The slug maps to a fixed allowlist; unknown slugs are a plain 404.
#[get("/server/logs/{kind}")]
pub async fn server_log_view_route(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    path: web::Path<String>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let Some(kind) = HostLogKind::from_slug(&path.into_inner()) else {
        return HttpResponse::NotFound()
            .content_type("text/plain; charset=utf-8")
            .body("Unknown log");
    };
    let search: String = query
        .get("q")
        .map(|v| v.chars().filter(|c| !c.is_control()).take(120).collect())
        .unwrap_or_default();
    let page = query
        .get("page")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(1);
    let per = query
        .get("per")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(DEFAULT_PER_PAGE);
    html_blocking(move || {
        panel_shell(
            &user,
            "logs",
            kind.title(),
            &host_log_page(&user, kind, &search, page, per),
        )
    })
    .await
}
