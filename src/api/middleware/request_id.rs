use actix_web::{
    Error, HttpMessage,
    dev::{Service, ServiceRequest, ServiceResponse, Transform, forward_ready},
    http::header::{HeaderName, HeaderValue},
};
use futures::future::{LocalBoxFuture, Ready, ok};
use std::rc::Rc;
use tracing::Instrument;

/// 存储在请求扩展中的请求唯一标识
#[derive(Clone, Debug)]
pub struct RequestId(pub String);

/// 为每个请求生成 UUID v4，写入扩展和响应头 `X-Request-ID`，
/// 并创建包含 request_id / method / path 的 tracing span。
pub struct RequestIdMiddleware;

impl<S, B> Transform<S, ServiceRequest> for RequestIdMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = RequestIdService<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(RequestIdService {
            service: Rc::new(service),
        })
    }
}

pub struct RequestIdService<S> {
    service: Rc<S>,
}

impl<S, B> Service<ServiceRequest> for RequestIdService<S>
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
        let request_id = uuid::Uuid::new_v4().to_string();
        let method = req.method().to_string();
        let path = req.path().to_string();

        req.extensions_mut().insert(RequestId(request_id.clone()));

        let span = tracing::info_span!(
            "request",
            request_id = %request_id,
            method = %method,
            path = %path,
        );

        Box::pin(
            async move {
                let mut resp = svc.call(req).await?;

                if let Ok(val) = HeaderValue::from_str(&request_id) {
                    resp.headers_mut()
                        .insert(HeaderName::from_static("x-request-id"), val);
                }

                Ok(resp)
            }
            .instrument(span),
        )
    }
}
