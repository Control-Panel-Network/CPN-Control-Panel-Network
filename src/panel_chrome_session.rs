//! Injects authenticated session expiry into request extensions for panel chrome (countdown).

use crate::auth_api::panel_session_expiry_from_request;
use crate::installer::AppState;
use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::{Error, HttpMessage, HttpRequest};
use futures_util::future::LocalBoxFuture;
use std::future::{Ready, ready};
use std::rc::Rc;
use std::sync::Arc;

/// Unix epoch seconds when the current browser session cookie expires (HMAC session token `exp`).
#[derive(Clone, Copy, Debug)]
pub struct PanelSessionExpiry(pub u64);

pub fn session_exp_from_http(http: &HttpRequest) -> Option<u64> {
    if let Some(value) = http.extensions().get::<PanelSessionExpiry>() {
        return Some(value.0);
    }
    if let Some(state) = http.app_data::<actix_web::web::Data<Arc<AppState>>>() {
        return panel_session_expiry_from_request(state.get_ref(), http);
    }
    None
}

pub struct PanelChromeSession;

impl<S, B> Transform<S, ServiceRequest> for PanelChromeSession
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = PanelChromeSessionMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(PanelChromeSessionMiddleware {
            service: Rc::new(service),
        }))
    }
}

pub struct PanelChromeSessionMiddleware<S> {
    service: Rc<S>,
}

impl<S, B> Service<ServiceRequest> for PanelChromeSessionMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    actix_web::dev::forward_ready!(service);

    fn call(&self, mut req: ServiceRequest) -> Self::Future {
        if let Some(state) = req.app_data::<actix_web::web::Data<Arc<AppState>>>() {
            if let Some(exp) = panel_session_expiry_from_request(state.get_ref(), req.request()) {
                req.extensions_mut().insert(PanelSessionExpiry(exp));
            }
        }
        let service = Rc::clone(&self.service);
        Box::pin(async move { service.call(req).await })
    }
}
