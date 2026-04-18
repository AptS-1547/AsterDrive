//! API 中间件：`rate_limit`。

use actix_governor::{
    GovernorConfig, GovernorConfigBuilder, KeyExtractor, SimpleKeyExtractionError,
};
use actix_web::dev::ServiceRequest;
use actix_web::{HttpResponse, HttpResponseBuilder};
use governor::NotUntil;
use governor::clock::{Clock, DefaultClock, QuantaInstant};
use governor::middleware::NoOpMiddleware;
use std::net::IpAddr;

use crate::api::response::ApiResponse;
use crate::config::RateLimitTier;

/// IP-based key extractor，429 响应返回 ApiResponse JSON 格式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AsterIpKeyExtractor;

impl KeyExtractor for AsterIpKeyExtractor {
    type Key = IpAddr;
    type KeyExtractionError = SimpleKeyExtractionError<&'static str>;

    fn extract(&self, req: &ServiceRequest) -> Result<Self::Key, Self::KeyExtractionError> {
        let ip = req
            .peer_addr()
            .map(|socket| socket.ip())
            .unwrap_or(IpAddr::from([127, 0, 0, 1]));
        Ok(ip)
    }

    fn exceed_rate_limit_response(
        &self,
        negative: &NotUntil<QuantaInstant>,
        _response: HttpResponseBuilder,
    ) -> HttpResponse {
        let wait_time = negative
            .wait_time_from(DefaultClock::default().now())
            .as_secs();
        let msg = format!("Too Many Requests, retry after {wait_time}s");
        HttpResponse::TooManyRequests()
            .insert_header(("Retry-After", wait_time.to_string()))
            .json(ApiResponse::<()>::error(
                crate::api::error_code::ErrorCode::RateLimited,
                &msg,
            ))
    }
}

/// 根据 tier 配置创建 Governor 实例
pub fn build_governor(tier: &RateLimitTier) -> GovernorConfig<AsterIpKeyExtractor, NoOpMiddleware> {
    GovernorConfigBuilder::default()
        .key_extractor(AsterIpKeyExtractor)
        .seconds_per_request(tier.seconds_per_request.get())
        .burst_size(tier.burst_size.get())
        .finish()
        .expect("non-zero rate limit tier should always build")
}
