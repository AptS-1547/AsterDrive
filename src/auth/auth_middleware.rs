use std::future::{ready, Ready};

use actix_web::{
    body::EitherBody,
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    http::header,
    web, Error, HttpMessage, HttpResponse,
};
use futures::future::LocalBoxFuture;
use serde_json::json;

use super::jwt::AuthError;
use crate::api::AppState;

pub struct AuthMiddleware;

impl<S, B> Transform<S, ServiceRequest> for AuthMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type InitError = ();
    type Transform = AuthMiddlewareService<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(AuthMiddlewareService { service }))
    }
}

pub struct AuthMiddlewareService<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for AuthMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        // Extract Authorization header
        let auth_header = req
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok());

        let auth_header = match auth_header {
            Some(header) => header,
            None => {
                let response = HttpResponse::Unauthorized()
                    .json(json!({"error": "Missing authorization header"}));
                return Box::pin(async move {
                    Ok(req.into_response(response).map_into_right_body())
                });
            }
        };

        // Extract token from "Bearer <token>"
        let token = match auth_header.strip_prefix("Bearer ") {
            Some(token) => token,
            None => {
                let response = HttpResponse::Unauthorized()
                    .json(json!({"error": "Invalid authorization header format"}));
                return Box::pin(async move {
                    Ok(req.into_response(response).map_into_right_body())
                });
            }
        };

        // Get app state
        let state = match req.app_data::<web::Data<AppState>>() {
            Some(state) => state.clone(),
            None => {
                let response = HttpResponse::InternalServerError()
                    .json(json!({"error": "Internal server error"}));
                return Box::pin(async move {
                    Ok(req.into_response(response).map_into_right_body())
                });
            }
        };

        // Verify token
        let claims = match state.jwt_manager.verify_token(token) {
            Ok(claims) => claims,
            Err(e) => {
                let error_msg = match e {
                    AuthError::TokenExpired => "Token expired",
                    _ => "Invalid token",
                };
                let response = HttpResponse::Unauthorized()
                    .json(json!({"error": error_msg}));
                return Box::pin(async move {
                    Ok(req.into_response(response).map_into_right_body())
                });
            }
        };

        // Insert claims into request extensions
        req.extensions_mut().insert(claims);

        let fut = self.service.call(req);

        Box::pin(async move {
            let res = fut.await?;
            Ok(res.map_into_left_body())
        })
    }
}
