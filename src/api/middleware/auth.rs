use actix_web::{
    Error, HttpMessage,
    dev::{Service, ServiceRequest, ServiceResponse, Transform, forward_ready},
    web,
};
use futures::future::{LocalBoxFuture, Ready, ok};
use std::rc::Rc;

use crate::runtime::AppState;
use crate::services::auth_service;

const ACCESS_COOKIE: &str = "aster_access";

/// JWT 认证中间件
/// 优先从 cookie 取 token，fallback 到 Authorization: Bearer header
pub struct JwtAuth;

impl<S, B> Transform<S, ServiceRequest> for JwtAuth
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = JwtAuthMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(JwtAuthMiddleware {
            service: Rc::new(service),
        })
    }
}

pub struct JwtAuthMiddleware<S> {
    service: Rc<S>,
}

impl<S, B> Service<ServiceRequest> for JwtAuthMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let svc = self.service.clone();

        Box::pin(async move {
            let state = req
                .app_data::<web::Data<AppState>>()
                .expect("AppState not found");

            // 1. Cookie 优先
            // 2. Authorization: Bearer fallback
            let token = req
                .cookie(ACCESS_COOKIE)
                .map(|c| c.value().to_string())
                .or_else(|| {
                    req.headers()
                        .get("Authorization")
                        .and_then(|v| v.to_str().ok())
                        .and_then(|v| v.strip_prefix("Bearer "))
                        .map(|s| s.to_string())
                });

            match token {
                None => Err(actix_web::error::ErrorUnauthorized("missing token")),
                Some(t) => match auth_service::verify_token(&t, &state.config.auth.jwt_secret) {
                    Ok(claims) => {
                        req.extensions_mut().insert(claims);
                        svc.call(req).await
                    }
                    Err(_) => Err(actix_web::error::ErrorUnauthorized("invalid token")),
                },
            }
        })
    }
}
