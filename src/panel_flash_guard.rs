//! Actix middleware: keep flash messages out of the address bar.
//!
//! Handlers redirect with human readable text in `?notice=` / `?error=` (for example
//! `/settings?error=Admin+only`), which shows up as `%20` or `+` in the browser. This guard
//! moves that text into a short-lived HttpOnly flash cookie:
//!
//! * Response side: a relative `Location` carrying `notice=` or `error=` is rewritten to the
//!   clean URL and the text is stored in the `cpn_panel_flash` cookie.
//! * Request side: a page navigation that still carries `notice=` or `error=` (static links,
//!   client-side redirects) is answered with a redirect to the clean URL plus the cookie.
//! * On the next page navigation the cookie text is merged back into the query seen by the
//!   handlers, so every existing page keeps reading `notice` and `error` unchanged, and the
//!   cookie is cleared once a page rendered.

use actix_web::Error;
use actix_web::HttpResponse;
use actix_web::body::EitherBody;
use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::http::header::{ACCEPT, HeaderValue, LOCATION, SET_COOKIE};
use actix_web::http::{Method, Uri};
use futures_util::future::LocalBoxFuture;
use std::future::{Ready, ready};
use std::rc::Rc;

const FLASH_COOKIE: &str = "cpn_panel_flash";
const MAX_FLASH_CHARS: usize = 1200;

/// Flash text pulled out of a URL: `('n', text)` for a notice, `('e', text)` for an error.
type Flash = (char, String);

fn pair_key(pair: &str) -> &str {
    pair.split_once('=').map(|(k, _)| k).unwrap_or(pair)
}

fn pair_value(pair: &str) -> &str {
    pair.split_once('=').map(|(_, v)| v).unwrap_or("")
}

fn truncate_chars(text: &str) -> String {
    text.chars().take(MAX_FLASH_CHARS).collect()
}

/// Split `path?query` into the same URL without `notice`/`error` and the flash they carried.
/// An error wins over a notice when both are present.
pub fn extract_flash(url: &str) -> (String, Option<Flash>) {
    let (before_fragment, fragment) = match url.split_once('#') {
        Some((a, b)) => (a, Some(b)),
        None => (url, None),
    };
    let Some((path, query)) = before_fragment.split_once('?') else {
        return (url.to_string(), None);
    };
    let mut kept: Vec<&str> = Vec::new();
    let mut notice: Option<String> = None;
    let mut error: Option<String> = None;
    for pair in query.split('&').filter(|p| !p.is_empty()) {
        match pair_key(pair) {
            "notice" => {
                if notice.is_none() {
                    notice = Some(crate::panel_action_log::percent_decode(pair_value(pair)));
                }
            }
            "error" => {
                if error.is_none() {
                    error = Some(crate::panel_action_log::percent_decode(pair_value(pair)));
                }
            }
            _ => kept.push(pair),
        }
    }
    let flash = match (error, notice) {
        (Some(e), _) if !e.trim().is_empty() => Some(('e', truncate_chars(e.trim()))),
        (_, Some(n)) if !n.trim().is_empty() => Some(('n', truncate_chars(n.trim()))),
        _ => None,
    };
    if notice_or_error_present(query) {
        let mut clean = path.to_string();
        if !kept.is_empty() {
            clean.push('?');
            clean.push_str(&kept.join("&"));
        }
        if let Some(f) = fragment {
            clean.push('#');
            clean.push_str(f);
        }
        return (clean, flash);
    }
    (url.to_string(), None)
}

fn notice_or_error_present(query: &str) -> bool {
    query
        .split('&')
        .any(|p| matches!(pair_key(p), "notice" | "error"))
}

fn set_cookie_value(flash: &Flash) -> String {
    format!(
        "{FLASH_COOKIE}={}.{}; Path=/; Max-Age=120; HttpOnly; SameSite=Lax",
        flash.0,
        crate::panel_hub_http::urlencoding_simple(&flash.1)
    )
}

fn clear_cookie_value() -> String {
    format!("{FLASH_COOKIE}=; Path=/; Max-Age=0; HttpOnly; SameSite=Lax")
}

fn flash_from_cookie(req: &ServiceRequest) -> Option<Flash> {
    let cookie = req.cookie(FLASH_COOKIE)?;
    let value = cookie.value();
    let (kind, rest) = value.split_once('.')?;
    let text = crate::panel_action_log::percent_decode(rest);
    if text.trim().is_empty() {
        return None;
    }
    match kind {
        "n" => Some(('n', truncate_chars(&text))),
        "e" => Some(('e', truncate_chars(&text))),
        _ => None,
    }
}

fn wants_html(req: &ServiceRequest) -> bool {
    req.headers()
        .get(ACCEPT)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.contains("text/html"))
        .unwrap_or(false)
}

fn is_page_path(path: &str) -> bool {
    !(path.starts_with("/api/")
        || path.starts_with("/static/")
        || path.starts_with("/assets/")
        || path.contains("/oauth/")
        || path.contains("/callback"))
}

fn is_relative_location(location: &str) -> bool {
    location.starts_with('/') && !location.starts_with("//")
}

pub struct FlashGuard;

impl<S, B> Transform<S, ServiceRequest> for FlashGuard
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type InitError = ();
    type Transform = FlashGuardMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(FlashGuardMiddleware {
            service: Rc::new(service),
        }))
    }
}

pub struct FlashGuardMiddleware<S> {
    service: Rc<S>,
}

impl<S, B> Service<ServiceRequest> for FlashGuardMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    actix_web::dev::forward_ready!(service);

    fn call(&self, mut req: ServiceRequest) -> Self::Future {
        let service = Rc::clone(&self.service);
        Box::pin(async move {
            let navigation =
                req.method() == Method::GET && wants_html(&req) && is_page_path(req.path());
            let mut consumed = false;
            if navigation {
                let query = req.query_string().to_string();
                if notice_or_error_present(&query) {
                    let url = format!("{}?{}", req.path(), query);
                    let (clean, flash) = extract_flash(&url);
                    let mut builder = HttpResponse::SeeOther();
                    builder.insert_header((LOCATION, clean));
                    if let Some(flash) = flash {
                        builder.append_header((SET_COOKIE, set_cookie_value(&flash)));
                    }
                    return Ok(req.into_response(builder.finish()).map_into_right_body());
                }
                if let Some((kind, text)) = flash_from_cookie(&req) {
                    let key = if kind == 'e' { "error" } else { "notice" };
                    let joined = if query.is_empty() { "" } else { "&" };
                    let rewritten = format!(
                        "{}?{}{}{}={}",
                        req.path(),
                        query,
                        joined,
                        key,
                        crate::panel_hub_http::urlencoding_simple(&text)
                    );
                    if let Ok(uri) = rewritten.parse::<Uri>() {
                        req.head_mut().uri = uri;
                        consumed = true;
                    }
                }
            }
            let mut res = service.call(req).await?;
            if res.status().is_redirection() {
                let location = res
                    .headers()
                    .get(LOCATION)
                    .and_then(|v| v.to_str().ok())
                    .map(str::to_string);
                if let Some(location) = location
                    && is_relative_location(&location)
                {
                    let (clean, flash) = extract_flash(&location);
                    if let Some(flash) = flash
                        && let (Ok(loc), Ok(cookie)) = (
                            HeaderValue::from_str(&clean),
                            HeaderValue::from_str(&set_cookie_value(&flash)),
                        )
                    {
                        res.headers_mut().insert(LOCATION, loc);
                        res.headers_mut().append(SET_COOKIE, cookie);
                    } else if clean != location
                        && let Ok(loc) = HeaderValue::from_str(&clean)
                    {
                        res.headers_mut().insert(LOCATION, loc);
                    }
                }
            } else if consumed && let Ok(cookie) = HeaderValue::from_str(&clear_cookie_value()) {
                res.headers_mut().append(SET_COOKIE, cookie);
            }
            Ok(res.map_into_left_body())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_spaces_from_error_location() {
        let (clean, flash) = extract_flash("/settings?error=Admin%20only");
        assert_eq!(clean, "/settings");
        assert_eq!(flash, Some(('e', "Admin only".to_string())));
    }

    #[test]
    fn keeps_other_params_and_fragment() {
        let (clean, flash) = extract_flash(
            "/plugins?view=store&domain=a.example.com&notice=Installed+X+on+Host%21#top",
        );
        assert_eq!(clean, "/plugins?view=store&domain=a.example.com#top");
        assert_eq!(flash, Some(('n', "Installed X on Host!".to_string())));
    }

    #[test]
    fn error_wins_over_notice() {
        let (clean, flash) = extract_flash("/x?notice=ok&error=bad+thing");
        assert_eq!(clean, "/x");
        assert_eq!(flash, Some(('e', "bad thing".to_string())));
    }

    #[test]
    fn urls_without_flash_are_untouched() {
        assert_eq!(
            extract_flash("/websites?tab=config"),
            ("/websites?tab=config".to_string(), None)
        );
        assert_eq!(
            extract_flash("/dashboard"),
            ("/dashboard".to_string(), None)
        );
    }

    #[test]
    fn empty_flash_values_are_dropped_from_url() {
        let (clean, flash) = extract_flash("/x?a=1&notice=");
        assert_eq!(clean, "/x?a=1");
        assert_eq!(flash, None);
    }

    #[test]
    fn flash_text_is_capped() {
        let long = "a".repeat(5000);
        let (_, flash) = extract_flash(&format!("/x?error={long}"));
        assert_eq!(flash.unwrap().1.chars().count(), MAX_FLASH_CHARS);
    }

    #[test]
    fn cookie_value_never_contains_spaces() {
        let value = set_cookie_value(&('e', "Admin only; a=b".to_string()));
        let payload = value.split(';').next().unwrap();
        assert!(!payload.contains(' '));
        assert!(payload.starts_with("cpn_panel_flash=e."));
    }

    #[test]
    fn only_relative_locations_are_rewritten() {
        assert!(is_relative_location("/settings?error=x"));
        assert!(!is_relative_location("//evil.example/?error=x"));
        assert!(!is_relative_location("https://example.com/?error=x"));
    }
}
