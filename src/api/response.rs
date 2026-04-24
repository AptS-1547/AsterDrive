//! 统一 API 响应封装。

use actix_web::HttpResponse;
use serde::{Deserialize, Serialize};
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::ToSchema;

use super::error_code::ErrorCode;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct ApiErrorInfo {
    pub internal_code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subcode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retryable: Option<bool>,
}

/// 统一 API 响应格式
///
/// 成功: `{ "code": 0, "msg": "", "data": {...} }`
/// 失败: `{ "code": 2000, "msg": "Invalid Credentials", "data": null }`
#[derive(Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct ApiResponse<T: Serialize> {
    pub code: ErrorCode,
    pub msg: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ApiErrorInfo>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            code: ErrorCode::Success,
            msg: String::new(),
            data: Some(data),
            error: None,
        }
    }

    pub fn ok_empty() -> ApiResponse<()> {
        ApiResponse {
            code: ErrorCode::Success,
            msg: String::new(),
            data: None,
            error: None,
        }
    }

    pub fn error(code: ErrorCode, msg: &str) -> ApiResponse<()> {
        Self::error_with_details(code, msg, None)
    }

    pub fn error_with_details(
        code: ErrorCode,
        msg: &str,
        error: Option<ApiErrorInfo>,
    ) -> ApiResponse<()> {
        ApiResponse {
            code,
            msg: msg.to_string(),
            data: None,
            error,
        }
    }

    pub fn into_response(self) -> HttpResponse {
        HttpResponse::Ok().json(self)
    }
}

#[derive(Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub build_time: String,
}

#[derive(Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct MemoryStatsResponse {
    pub heap_allocated_mb: String,
    pub heap_peak_mb: String,
}

#[derive(Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct PurgedCountResponse {
    pub purged: u32,
}

#[derive(Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct RemovedCountResponse {
    pub removed: u64,
}
