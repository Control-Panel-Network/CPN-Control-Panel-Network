//! Actix middleware: record plugin and host-package POST actions in the panel action log.
//!
//! Handlers answer these POSTs with a PRG redirect whose `Location` carries `notice=` or
//! `error=` (and `domain=`). The guard reads that outcome after the handler ran, so install,
//! activate, deactivate, uninstall, start and stop are all logged in one place without editing
//! every route.

use crate::auth_api::panel_user_from_request;
use crate::installer::AppState;
use crate::panel_action_log::{action_for_path, outcome_from_location, record};
use actix_web::Error;
use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::http::Method;
use actix_web::http::header::LOCATION;
use futures_util::future::LocalBoxFuture;
use std::future::{Ready, ready};
use std::rc::Rc;
use std::sync::Arc;

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
            let action = if req.method() == Method::POST {
                action_for_path(req.path())
            } else {
                None
            };
            let actor = action.and_then(|_| {
                req.app_data::<actix_web::web::Data<Arc<AppState>>>()
                    .and_then(|state| panel_user_from_request(state.get_ref(), req.request()))
            });
            let res = service.call(req).await?;
            if let (Some(action), Some(actor)) = (action, actor)
                && let Some(location) = res
                    .headers()
                    .get(LOCATION)
                    .and_then(|value| value.to_str().ok())
                && let Some(outcome) = outcome_from_location(location)
                && let Err(error) = record(
                    &actor,
                    action,
                    &outcome.domain,
                    outcome.ok,
                    &outcome.message,
                )
            {
                eprintln!("panel-action-log: could not record {action}: {error}");
            }
            Ok(res)
        })
    }
}
