//! Actix middleware: deny-by-default audit for authenticated mutating HTTP.
//!
//! Every authenticated POST/PUT/PATCH/DELETE is recorded unless the path is on the
//! explicit noisy read-like denylist. Known paths get stable action keys; others
//! synthesize `http.<method>:<path>`. PRG `notice=` / `error=` outcomes are preferred
//! when present; otherwise HTTP status drives `ok`.

use crate::auth_api::panel_user_from_request;
use crate::installer::AppState;
use crate::panel_action_log::outcome_from_location;
use crate::panel_audit::{action_for_path, is_noisy_read_like, record_full, synthesize_action};
use crate::panel_audit_paths::{category_for_action, category_for_path, default_severity};
use actix_web::Error;
use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::http::Method;
use actix_web::http::header::{LOCATION, USER_AGENT};
use futures_util::future::LocalBoxFuture;
use std::future::{Ready, ready};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

static REQ_COUNTER: AtomicU64 = AtomicU64::new(1);

pub struct ActionLogGuard;

impl<S, B> Transform<S, ServiceRequest> for ActionLogGuard
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = ActionLogGuardMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(ActionLogGuardMiddleware {
            service: Rc::new(service),
        }))
    }
}

pub struct ActionLogGuardMiddleware<S> {
    service: Rc<S>,
}

fn is_mutating(method: &Method) -> bool {
    matches!(
        *method,
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    )
}

fn client_ip(req: &ServiceRequest) -> String {
    req.headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| req.peer_addr().map(|a| a.ip().to_string()))
        .unwrap_or_default()
}

fn user_agent(req: &ServiceRequest) -> String {
    req.headers()
        .get(USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .chars()
        .take(200)
        .collect()
}

fn request_id(req: &ServiceRequest) -> String {
    if let Some(id) = req
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        return id.chars().take(64).collect();
    }
    let n = REQ_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("cpn-{n}")
}

impl<S, B> Service<ServiceRequest> for ActionLogGuardMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    actix_web::dev::forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let service = Rc::clone(&self.service);
        Box::pin(async move {
            let method = req.method().clone();
            let path = req.path().to_string();
            let self_recorded = matches!(
                path.trim_end_matches('/'),
                "/login"
                    | "/login/2fa"
                    | "/logout"
                    | "/api/logout"
                    | "/forgot-password"
                    | "/reset-password"
            );
            let should_log =
                is_mutating(&method) && !is_noisy_read_like(&path) && !self_recorded;
            let actor = if should_log {
                req.app_data::<actix_web::web::Data<Arc<AppState>>>()
                    .and_then(|state| panel_user_from_request(state.get_ref(), req.request()))
            } else {
                None
            };
            let ip = if should_log {
                client_ip(&req)
            } else {
                String::new()
            };
            let ua = if should_log {
                user_agent(&req)
            } else {
                String::new()
            };
            let req_id = if should_log {
                request_id(&req)
            } else {
                String::new()
            };
            let mapped = if should_log {
                action_for_path(&path)
            } else {
                None
            };
            let res = service.call(req).await?;
            if let Some(actor) = actor {
                let status = res.status().as_u16();
                let (ok, message, target) = if let Some(location) = res
                    .headers()
                    .get(LOCATION)
                    .and_then(|value| value.to_str().ok())
                {
                    if let Some(outcome) = outcome_from_location(location) {
                        (outcome.ok, outcome.message, outcome.domain)
                    } else {
                        (status < 400, format!("HTTP {status}"), String::new())
                    }
                } else {
                    (status < 400, format!("HTTP {status}"), String::new())
                };
                let action_owned = mapped
                    .map(str::to_string)
                    .unwrap_or_else(|| synthesize_action(method.as_str(), &path));
                let category = mapped
                    .map(category_for_action)
                    .unwrap_or_else(|| category_for_path(&path));
                let severity = default_severity(&action_owned, ok);
                if let Err(error) = record_full(
                    &actor,
                    &action_owned,
                    &target,
                    ok,
                    &message,
                    &ip,
                    &ua,
                    &req_id,
                    Some(category),
                    Some(severity),
                ) {
                    eprintln!("panel-audit: could not record {action_owned}: {error}");
                }
            }
            Ok(res)
        })
    }
}
