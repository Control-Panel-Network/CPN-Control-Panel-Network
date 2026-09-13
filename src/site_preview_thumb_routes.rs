//! Authenticated Site preview thumbnail routes (image + refresh).

use crate::auth_api::panel_user_from_request;
use crate::installer::AppState;
use crate::login_next::login_redirect;
use crate::site_acl::{SitePerm, require_manage_site};
use crate::site_preview_capture::{capture_available, capture_site_preview};
use crate::site_preview_thumb::{
    PreviewFreshness, freshness, load_meta, placeholder_svg, read_cached_image,
};
use crate::sites::normalize_domain;
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use std::collections::HashMap;
use std::sync::Arc;

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

fn domain_from_query(query: &HashMap<String, String>) -> Result<String, String> {
    let raw = query.get("domain").map(String::as_str).unwrap_or("").trim();
    if raw.is_empty() {
        return Err("domain is required".into());
    }
    normalize_domain(raw)
}

/// GET `/websites/site-preview/image?domain=` : cached PNG or SVG placeholder.
#[get("/websites/site-preview/image")]
pub async fn site_preview_image(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = panel_user_from_request(&state, &http) else {
        return login_redirect(&http);
    };
    let domain = match domain_from_query(&query) {
        Ok(d) => d,
        Err(err) => return HttpResponse::BadRequest().body(err),
    };
    if let Err(err) = require_manage_site(&user, &domain, SitePerm::Enable) {
        return HttpResponse::Forbidden().body(err);
    }

    let meta = load_meta(&domain);
    if meta.ok {
        if let Ok((bytes, ctype)) = read_cached_image(&domain) {
            let cache = if freshness(&domain) == PreviewFreshness::Fresh {
                "private, max-age=300"
            } else {
                "private, max-age=60"
            };
            return HttpResponse::Ok()
                .content_type(ctype)
                .append_header(("Cache-Control", cache))
                .append_header(("X-Content-Type-Options", "nosniff"))
                .body(bytes);
        }
    }

    let detail = if !meta.error.is_empty() {
        meta.error.clone()
    } else if !capture_available() {
        "Capture tool not installed on this host".into()
    } else {
        "No screenshot yet. Use Refresh preview.".into()
    };
    let svg = placeholder_svg(&domain, &detail);
    HttpResponse::Ok()
        .content_type("image/svg+xml; charset=utf-8")
        .append_header(("Cache-Control", "private, no-store"))
        .append_header(("X-Content-Type-Options", "nosniff"))
        .body(svg)
}

/// POST `/websites/site-preview/refresh` : force recapture, then redirect.
#[derive(Debug, serde::Deserialize)]
pub struct SitePreviewRefreshForm {
    #[serde(default)]
    domain: String,
    #[serde(default)]
    next: String,
}

#[post("/websites/site-preview/refresh")]
pub async fn site_preview_refresh(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<SitePreviewRefreshForm>,
) -> HttpResponse {
    let Some(user) = panel_user_from_request(&state, &http) else {
        return login_redirect(&http);
    };
    let domain = match normalize_domain(form.domain.trim()) {
        Ok(d) => d,
        Err(err) => {
            return HttpResponse::SeeOther()
                .append_header((
                    "Location",
                    format!("/websites?error={}", urlencoding_simple(&err)),
                ))
                .finish();
        }
    };
    if let Err(err) = require_manage_site(&user, &domain, SitePerm::Enable) {
        return HttpResponse::SeeOther()
            .append_header((
                "Location",
                format!("/websites?error={}", urlencoding_simple(&err)),
            ))
            .finish();
    }

    let next = sanitize_next(&form.next, &domain);
    let capture_domain = domain.clone();
    let result = web::block(move || capture_site_preview(&capture_domain)).await;

    let location = match result {
        Ok(Ok(_)) => format!(
            "{next}notice={}",
            urlencoding_simple("Site preview updated.")
        ),
        Ok(Err(err)) => format!("{next}error={}", urlencoding_simple(&err)),
        Err(_) => format!(
            "{next}error={}",
            urlencoding_simple("Site preview capture worker failed")
        ),
    };
    HttpResponse::SeeOther()
        .append_header(("Location", location))
        .finish()
}

fn sanitize_next(raw: &str, domain: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.starts_with("/websites/manage?") && trimmed.contains(domain) {
        let joiner = if trimmed.contains('?') {
            if trimmed.ends_with('?') || trimmed.ends_with('&') {
                ""
            } else {
                "&"
            }
        } else {
            "?"
        };
        return format!("{trimmed}{joiner}");
    }
    if trimmed == "/websites" || trimmed.starts_with("/websites?") {
        let joiner = if trimmed.contains('?') { "&" } else { "?" };
        return format!("/websites{joiner}");
    }
    "/websites?".to_string()
}

/// Kick a background capture when the list page loads (non-minimalist).
pub fn spawn_background_capture(domain: String) {
    if freshness(&domain) == PreviewFreshness::Fresh {
        return;
    }
    std::thread::Builder::new()
        .name("cpn-site-preview".into())
        .spawn(move || {
            let _ = capture_site_preview(&domain);
        })
        .ok();
}

#[cfg(test)]
mod tests {
    use super::sanitize_next;

    #[test]
    fn next_defaults_to_websites() {
        assert!(sanitize_next("", "x.com").starts_with("/websites"));
        assert!(sanitize_next("https://evil.example/", "x.com").starts_with("/websites"));
    }

    #[test]
    fn next_allows_manage_for_domain() {
        let n = sanitize_next("/websites/manage?domain=x.com&tab=overview", "x.com");
        assert!(n.contains("x.com"));
        assert!(n.ends_with('&') || n.contains("notice=") || n.ends_with('&'));
    }
}
