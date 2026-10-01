//! Actix middleware: serve the fancy maintenance page while an upgrade flag is active.

use crate::auth_api::panel_user_from_request;
use crate::installer::AppState;
use crate::panel_admin::is_panel_admin;
use crate::panel_maintenance_mode::{self, bypass_cookie_name};
use crate::panel_maintenance_page;
use actix_web::body::EitherBody;
use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::http::header::{HeaderValue, SET_COOKIE};
use actix_web::{Error, HttpResponse};
use futures_util::future::LocalBoxFuture;
use std::future::{Ready, ready};
use std::rc::Rc;
use std::sync::Arc;

pub struct PanelMaintenanceGuard;

impl<S, B> Transform<S, ServiceRequest> for PanelMaintenanceGuard
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type InitError = ();
    type Transform = PanelMaintenanceGuardMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(PanelMaintenanceGuardMiddleware {
            service: Rc::new(service),
        }))
    }
}

pub struct PanelMaintenanceGuardMiddleware<S> {
    service: Rc<S>,
}

fn path_is_exempt(path: &str) -> bool {
    matches!(
        path,
        "/api/panel-maintenance"
            | "/maintenance"
            | "/cpn-logo.png"
            | "/favicon.ico"
            | "/favicon.svg"
            | "/apple-touch-icon.png"
            | "/cpn-brand-mark.svg"
            // Status/apply APIs stay reachable for the signed-in owner (session bypass
            // also covers them; exempt keeps polls alive if cookies race during restart).
            | "/api/maintenance"
            | "/api/maintenance/status"
            | "/api/version-check"
            | "/api/version-source"
            | "/api/releases"
    )
}

fn query_bypass_token(req: &ServiceRequest) -> Option<String> {
    let query = req.query_string();
    for pair in query.split('&').filter(|p| !p.is_empty()) {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        if k == "cpn_maint_bypass" && !v.is_empty() {
            return Some(
                urlencoding_decode(v)
                    .unwrap_or_else(|| v.to_string())
                    .trim()
                    .to_string(),
            );
        }
    }
    None
}

fn cookie_bypass_token(req: &ServiceRequest) -> Option<String> {
    let cookie_header = req.headers().get(actix_web::http::header::COOKIE)?;
    let raw = cookie_header.to_str().ok()?;
    let name = bypass_cookie_name();
    for part in raw.split(';') {
        let part = part.trim();
        if let Some(rest) = part.strip_prefix(name)
            && let Some(value) = rest.strip_prefix('=')
        {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

fn urlencoding_decode(raw: &str) -> Option<String> {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                let h = from_hex(bytes[i + 1])?;
                let l = from_hex(bytes[i + 2])?;
                out.push((h << 4) | l);
                i += 3;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

fn from_hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn bypass_cookie_header(token: &str, secure: bool) -> String {
    let secure_flag = if secure { "; Secure" } else { "" };
    format!(
        "{}={}; Path=/; HttpOnly; SameSite=Lax; Max-Age=7200{}",
        bypass_cookie_name(),
        token,
        secure_flag
    )
}

fn wants_json(req: &ServiceRequest) -> bool {
    req.headers()
        .get(actix_web::http::header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.contains("application/json"))
        .unwrap_or(false)
        || req.path().starts_with("/api/")
}

impl<S, B> Service<ServiceRequest> for PanelMaintenanceGuardMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    actix_web::dev::forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let service = Rc::clone(&self.service);
        Box::pin(async move {
            let path = req.path().to_string();
            if path_is_exempt(&path) {
                let res = service.call(req).await?;
                return Ok(res.map_into_left_body());
            }

            let Some(flag) = panel_maintenance_mode::load_active() else {
                let res = service.call(req).await?;
                return Ok(res.map_into_left_body());
            };

            let mut set_bypass_cookie: Option<String> = None;
            if let Some(token) = query_bypass_token(&req)
                && panel_maintenance_mode::bypass_token_matches(&token)
            {
                let secure = req.connection_info().scheme() == "https";
                set_bypass_cookie = Some(bypass_cookie_header(&token, secure));
            }

            let has_bypass = set_bypass_cookie.is_some()
                || cookie_bypass_token(&req)
                    .map(|t| panel_maintenance_mode::bypass_token_matches(&t))
                    .unwrap_or(false);

            let is_admin = req
                .app_data::<actix_web::web::Data<Arc<AppState>>>()
                .and_then(|state| {
                    panel_user_from_request(state.get_ref(), req.request())
                        .filter(|user| is_panel_admin(user))
                })
                .is_some();

            if has_bypass || is_admin {
                let mut res = service.call(req).await?;
                if let Some(cookie) = set_bypass_cookie
                    && let Ok(value) = HeaderValue::from_str(&cookie)
                {
                    res.headers_mut().append(SET_COOKIE, value);
                }
                return Ok(res.map_into_left_body());
            }

            let response = if wants_json(&req) {
                HttpResponse::ServiceUnavailable().json(serde_json::json!({
                    "error": "maintenance",
                    "active": true,
                    "phase": flag.phase,
                    "progress": flag.progress,
                    "message": flag.message,
                    "title": flag.title,
                    "target": flag.target,
                }))
            } else {
                HttpResponse::ServiceUnavailable()
                    .content_type("text/html; charset=utf-8")
                    .insert_header(("Retry-After", "5"))
                    .body(panel_maintenance_page::render_html(&flag))
            };
            Ok(req.into_response(response).map_into_right_body())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exempt_paths_cover_version_and_status() {
        assert!(path_is_exempt("/api/panel-maintenance"));
        assert!(path_is_exempt("/api/maintenance/status"));
        assert!(!path_is_exempt("/settings/version"));
        assert!(!path_is_exempt("/dashboard"));
        assert!(!path_is_exempt("/login"));
    }
}
