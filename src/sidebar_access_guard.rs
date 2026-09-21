//! Actix middleware: authenticated panel paths respect sidebar visibility ACL.

use crate::auth_api::panel_user_from_request;
use crate::installer::AppState;
use crate::panel_error_messages::forbidden_page_html;
use crate::sidebar_visibility::{path_allowed, path_is_exempt};
use actix_web::body::EitherBody;
use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::{Error, HttpResponse};
use futures_util::future::LocalBoxFuture;
use std::future::{Ready, ready};
use std::rc::Rc;
use std::sync::Arc;

pub struct SidebarAccessGuard;

impl<S, B> Transform<S, ServiceRequest> for SidebarAccessGuard
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type InitError = ();
    type Transform = SidebarAccessGuardMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(SidebarAccessGuardMiddleware {
            service: Rc::new(service),
        }))
    }
}

pub struct SidebarAccessGuardMiddleware<S> {
    service: Rc<S>,
}

impl<S, B> Service<ServiceRequest> for SidebarAccessGuardMiddleware<S>
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
            if !path_is_exempt(&path)
                && let Some(state) = req.app_data::<actix_web::web::Data<Arc<AppState>>>()
            {
                let http = req.request();
                if let Some(user) = panel_user_from_request(state.get_ref(), http)
                    && !path_allowed(&user, &path)
                {
                    let response = HttpResponse::Forbidden()
                        .content_type("text/html; charset=utf-8")
                        .body(forbidden_page_html());
                    return Ok(req.into_response(response).map_into_right_body());
                }
            }
            let res = service.call(req).await?;
            Ok(res.map_into_left_body())
        })
    }
}
