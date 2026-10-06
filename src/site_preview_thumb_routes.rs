//! Authenticated Site preview thumbnail routes (image + refresh).

use crate::auth_api::panel_user_from_request;
use crate::installer::AppState;
use crate::login_next::login_redirect;
use crate::site_acl::{SitePerm, require_manage_site};
use crate::site_preview_capture::{
    capture_available, capture_site_preview, capture_site_preview_local,
};
use crate::site_preview_microlink::remote_preview_ready;
use crate::site_preview_thumb::{
    PreviewFreshness, cached_shot_usable, freshness, load_meta, placeholder_svg, read_cached_image,
};
use crate::sites::normalize_domain;
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, OnceLock};

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
    // Serve any successful disk cache (Fresh or Stale). List reloads must not
    // force a remote recapture just because TTL expired.
    if cached_shot_usable(&domain)
        && let Ok((bytes, ctype)) = read_cached_image(&domain)
    {
        let cache = if freshness(&domain) == PreviewFreshness::Fresh {
            // Align browser cache with durable disk TTL; `v=` busts after Refresh.
            "private, max-age=3600"
        } else {
            "private, max-age=300"
        };
        return HttpResponse::Ok()
            .content_type(ctype)
            .append_header(("Cache-Control", cache))
            .append_header(("X-Content-Type-Options", "nosniff"))
            .body(bytes);
    }

    let detail = if !meta.error.is_empty() {
        meta.error.clone()
    } else if !capture_available() && !remote_preview_ready(&domain) {
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
                    format!("/websites/list?error={}", urlencoding_simple(&err)),
                ))
                .finish();
        }
    };
    if let Err(err) = require_manage_site(&user, &domain, SitePerm::Enable) {
        return HttpResponse::SeeOther()
            .append_header((
                "Location",
                format!("/websites/list?error={}", urlencoding_simple(&err)),
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

fn path_of(url: &str) -> &str {
    url.split(['?', '#']).next().unwrap_or(url)
}

fn query_joiner(trimmed: &str) -> &'static str {
    if !trimmed.contains('?') {
        "?"
    } else if trimmed.ends_with('?') || trimmed.ends_with('&') {
        ""
    } else {
        "&"
    }
}

fn is_safe_relative(trimmed: &str) -> bool {
    if !trimmed.starts_with('/') || trimmed.starts_with("//") {
        return false;
    }
    if trimmed.contains('\n')
        || trimmed.contains('\r')
        || trimmed.contains('\\')
        || trimmed.contains("://")
    {
        return false;
    }
    true
}

fn sanitize_next(raw: &str, domain: &str) -> String {
    let trimmed = raw.trim();
    if !is_safe_relative(trimmed) {
        return "/websites/list?".to_string();
    }
    let path = path_of(trimmed);
    let allowed = match path {
        "/subdomains" | "/websites" | "/websites/list" => true,
        "/websites/manage" => !domain.is_empty() && trimmed.contains(domain),
        _ => false,
    };
    if !allowed {
        return "/websites/list?".to_string();
    }
    format!("{trimmed}{}", query_joiner(trimmed))
}

/// Domains with a background capture already running, so repeated list loads do
/// not stack chromium runs or outbound screenshot requests for the same site.
fn in_flight() -> &'static Mutex<HashSet<String>> {
    static IN_FLIGHT: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    IN_FLIGHT.get_or_init(|| Mutex::new(HashSet::new()))
}

fn claim_capture(domain: &str) -> bool {
    let Ok(mut guard) = in_flight().lock() else {
        return false;
    };
    guard.insert(domain.to_string())
}

fn release_capture(domain: &str) {
    if let Ok(mut guard) = in_flight().lock() {
        guard.remove(domain);
    }
}

/// Kick a background capture when the list page loads (non-minimalist).
///
/// Local Chromium only. Never calls Microlink from list N+1 loads. Skips when a
/// usable (Fresh or Stale) disk cache already exists. Missing shots wait for
/// Refresh when no headless browser is installed.
pub fn spawn_background_capture(domain: String) {
    if cached_shot_usable(&domain) {
        return;
    }
    if freshness(&domain) == PreviewFreshness::Fresh {
        return;
    }
    if !capture_available() {
        // No local browser: leave the SVG placeholder. Remote capture is
        // Refresh-only so /websites and /subdomains do not burn API quota.
        return;
    }
    if !claim_capture(&domain) {
        return;
    }
    let spawned = std::thread::Builder::new()
        .name("cpn-site-preview".into())
        .spawn({
            let domain = domain.clone();
            move || {
                let _ = capture_site_preview_local(&domain);
                release_capture(&domain);
            }
        })
        .is_ok();
    if !spawned {
        release_capture(&domain);
    }
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
    fn next_allows_subdomains_list() {
        let n = sanitize_next("/subdomains", "cmstest.newstargeted.com");
        assert!(n.starts_with("/subdomains"));
        assert!(n.contains('?'));
    }

    #[test]
    fn next_preserves_list_pagination_and_search() {
        let websites = sanitize_next("/websites/list?q=blog&mode=page&per_page=5&page=2", "x.com");
        assert_eq!(
            websites,
            "/websites/list?q=blog&mode=page&per_page=5&page=2&"
        );
        let subs = sanitize_next("/subdomains?mode=page&per_page=10&page=3", "a.b.com");
        assert_eq!(subs, "/subdomains?mode=page&per_page=10&page=3&");
        assert_eq!(sanitize_next("/websites/list", "x.com"), "/websites/list?");
        assert_eq!(
            sanitize_next("/websites/list-evil", "x.com"),
            "/websites/list?"
        );
        assert_eq!(sanitize_next("//evil.example", "x.com"), "/websites/list?");
    }

    #[test]
    fn next_allows_manage_with_domain() {
        let n = sanitize_next("/websites/manage?domain=x.com&tab=overview", "x.com");
        assert_eq!(n, "/websites/manage?domain=x.com&tab=overview&");
    }
}
