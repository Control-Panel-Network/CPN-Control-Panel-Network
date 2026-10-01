//! Server > Logs route (panel activity plus links to every other log).

use crate::installer::AppState;
use crate::panel_hub_http::{html_blocking, login_redirect, require_panel_user};
use crate::panel_hub_pages_server_logs::server_logs_page;
use crate::panel_pages::panel_shell;
use actix_web::{HttpRequest, HttpResponse, get, web};
use std::sync::Arc;

#[get("/server/logs")]
pub async fn server_logs_route(http: HttpRequest, state: web::Data<Arc<AppState>>) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    html_blocking(move || panel_shell(&user, "server", "Logs", &server_logs_page(&user))).await
}
