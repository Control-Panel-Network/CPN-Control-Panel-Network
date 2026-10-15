//! Authenticated `/docs` and `/docs/images/...` routes.

use crate::auth_api::panel_user_from_request;
use crate::installer::AppState;
use crate::login_next::login_redirect;
use crate::panel_docs::docs_image_bytes;
use crate::panel_docs_pages::docs_main;
use crate::panel_pages::panel_shell;
use actix_web::{HttpRequest, HttpResponse, get, web};
use std::sync::Arc;

#[derive(Debug, serde::Deserialize)]
pub struct DocsQuery {
    #[serde(default)]
    page: String,
    #[serde(default)]
    q: String,
}

fn html_ok(body: String) -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(body)
}

#[get("/docs")]
pub async fn docs_page(
    state: web::Data<Arc<AppState>>,
    http: HttpRequest,
    query: web::Query<DocsQuery>,
) -> HttpResponse {
    let Some(user) = panel_user_from_request(&state, &http) else {
        return login_redirect(&http);
    };
    let page = query.page.trim().to_string();
    let q = query.q.trim().to_string();
    let main = docs_main(&page, &q);
    html_ok(panel_shell(&user, "docs", "Documentation", &main))
}

#[get("/documentation")]
pub async fn documentation_alias() -> HttpResponse {
    HttpResponse::Found()
        .append_header(("Location", "/docs"))
        .finish()
}

#[get("/docs/images/{path:.*}")]
pub async fn docs_image(
    state: web::Data<Arc<AppState>>,
    http: HttpRequest,
    path: web::Path<String>,
) -> HttpResponse {
    let Some(_user) = panel_user_from_request(&state, &http) else {
        return login_redirect(&http);
    };
    let rel = path.into_inner();
    match docs_image_bytes(&rel) {
        Some((bytes, mime)) => HttpResponse::Ok()
            .content_type(mime)
            .insert_header(("Cache-Control", "public, max-age=86400"))
            .body(bytes),
        None => HttpResponse::NotFound().body("Image not found"),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn docs_path_is_clean() {
        assert_eq!("/docs", "/docs");
        assert!(!"/docs?page=install".contains(' '));
        assert!(!"/docs?page=install".contains("%20"));
    }
}
