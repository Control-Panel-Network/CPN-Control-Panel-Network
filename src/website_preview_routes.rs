//! Preview mode routes: chrome shell + same-origin docroot content proxy.

use crate::auth_api::panel_user_from_request;
use crate::installer::AppState;
use crate::site_acl::{SitePerm, require_manage_site};
use crate::website_preview::{
    guess_content_type, preview_content_url, preview_mode_html, preview_mode_url, public_site_url,
    resolve_index_file, resolve_under_docroot,
};
use actix_web::{HttpRequest, HttpResponse, get, web};
use std::path::Path;
use std::sync::Arc;

fn login_redirect() -> HttpResponse {
    HttpResponse::SeeOther()
        .append_header(("Location", "/login"))
        .finish()
}

fn urlencoding_simple(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for b in value.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Query alias: `/websites/preview?domain=` -> `/preview/<domain>/`.
#[get("/websites/preview")]
pub async fn websites_preview_redirect(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = panel_user_from_request(&state, &http) else {
        return login_redirect();
    };
    let domain = query.get("domain").map(String::as_str).unwrap_or("");
    match require_manage_site(&user, domain, SitePerm::Enable) {
        Ok(site) => match preview_mode_url(&site.domain) {
            Ok(url) => HttpResponse::SeeOther()
                .append_header(("Location", url))
                .finish(),
            Err(err) => HttpResponse::SeeOther()
                .append_header((
                    "Location",
                    format!("/websites?error={}", urlencoding_simple(&err)),
                ))
                .finish(),
        },
        Err(err) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                format!("/websites?error={}", urlencoding_simple(&err)),
            ))
            .finish(),
    }
}

/// Pretty manage alias: `/websites/<domain>` -> manage dashboard.
#[get("/websites/{domain}")]
pub async fn websites_pretty_manage(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    path: web::Path<String>,
) -> HttpResponse {
    let Some(_user) = panel_user_from_request(&state, &http) else {
        return login_redirect();
    };
    let domain = path.into_inner();
    // Avoid capturing reserved subpaths.
    if matches!(
        domain.as_str(),
        "manage" | "create" | "delete" | "suspend" | "resume" | "prefs" | "preview"
    ) {
        return HttpResponse::NotFound().finish();
    }
    HttpResponse::SeeOther()
        .append_header((
            "Location",
            format!("/websites/manage?domain={}", urlencoding_simple(&domain)),
        ))
        .finish()
}

#[get("/preview/{domain}/")]
pub async fn preview_mode_page(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    path: web::Path<String>,
) -> HttpResponse {
    let Some(user) = panel_user_from_request(&state, &http) else {
        return login_redirect();
    };
    let domain = path.into_inner();
    let site = match require_manage_site(&user, &domain, SitePerm::Enable) {
        Ok(site) => site,
        Err(err) => {
            return HttpResponse::SeeOther()
                .append_header((
                    "Location",
                    format!("/websites?error={}", urlencoding_simple(&err)),
                ))
                .finish();
        }
    };
    let live = public_site_url(&site.domain).unwrap_or_else(|_| format!("http://{}", site.domain));
    let content = match preview_content_url(&site.domain, "") {
        Ok(url) => url,
        Err(err) => {
            return HttpResponse::SeeOther()
                .append_header((
                    "Location",
                    format!("/websites?error={}", urlencoding_simple(&err)),
                ))
                .finish();
        }
    };
    match preview_mode_html(&site.domain, &live, &content) {
        Ok(html) => HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .append_header(("X-Frame-Options", "SAMEORIGIN"))
            .body(html),
        Err(err) => HttpResponse::BadRequest().body(err),
    }
}

#[get("/preview/{domain}/content/{tail:.*}")]
pub async fn preview_content(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    path: web::Path<(String, String)>,
) -> HttpResponse {
    let Some(user) = panel_user_from_request(&state, &http) else {
        return login_redirect();
    };
    let (domain, tail) = path.into_inner();
    let site = match require_manage_site(&user, &domain, SitePerm::Enable) {
        Ok(site) => site,
        Err(err) => {
            return HttpResponse::Forbidden().body(err);
        }
    };
    let docroot = Path::new(&site.docroot);
    let resolved = match resolve_under_docroot(docroot, &tail) {
        Ok(path) => path,
        Err(err) => return HttpResponse::BadRequest().body(err),
    };
    let file_path = if resolved.is_dir() {
        match resolve_index_file(&resolved) {
            Some(index) => index,
            None => {
                return HttpResponse::Ok()
                    .content_type("text/html; charset=utf-8")
                    .body(format!(
                        "<!DOCTYPE html><html><body><h1>Directory</h1><p>No index file in <code>{}</code>.</p></body></html>",
                        html_escape_min(&resolved.display().to_string())
                    ));
            }
        }
    } else {
        resolved
    };
    if !file_path.is_file() {
        return HttpResponse::NotFound().body("File not found in site document root");
    }
    // Do not execute PHP; serve source or static bytes only.
    match std::fs::read(&file_path) {
        Ok(bytes) => HttpResponse::Ok()
            .content_type(guess_content_type(&file_path))
            .append_header(("X-Content-Type-Options", "nosniff"))
            .append_header(("Cache-Control", "private, no-store"))
            .body(bytes),
        Err(err) => HttpResponse::InternalServerError().body(format!("Cannot read file: {err}")),
    }
}

fn html_escape_min(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::urlencoding_simple;

    #[test]
    fn encodes_domain_query() {
        assert_eq!(urlencoding_simple("a.b"), "a.b");
        assert!(urlencoding_simple("a b").contains('%'));
    }
}
