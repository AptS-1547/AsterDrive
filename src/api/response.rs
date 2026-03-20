use actix_web::HttpResponse;
use serde::Serialize;
use utoipa::ToSchema;

use super::error_code::ErrorCode;

/// 统一 API 响应格式
///
/// 成功: `{ "code": 0, "msg": "", "data": {...} }`
/// 失败: `{ "code": 2000, "msg": "Invalid Credentials", "data": null }`
#[derive(Serialize, ToSchema)]
pub struct ApiResponse<T: Serialize> {
    pub code: ErrorCode,
    pub msg: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            code: ErrorCode::Success,
            msg: String::new(),
            data: Some(data),
        }
    }

    pub fn ok_empty() -> ApiResponse<()> {
        ApiResponse {
            code: ErrorCode::Success,
            msg: String::new(),
            data: None,
        }
    }

    pub fn error(code: ErrorCode, msg: &str) -> ApiResponse<()> {
        ApiResponse {
            code,
            msg: msg.to_string(),
            data: None,
        }
    }

    pub fn into_response(self) -> HttpResponse {
        HttpResponse::Ok().json(self)
    }
}

// DTO schemas for OpenAPI

#[derive(Serialize, ToSchema)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: String,
}

#[derive(Serialize, ToSchema)]
pub struct RefreshResponse {
    pub access_token: String,
}

#[derive(Serialize, ToSchema)]
pub struct FolderContentsResponse {
    pub folders: Vec<crate::entities::folder::Model>,
    pub files: Vec<crate::entities::file::Model>,
}

#[derive(Serialize, ToSchema)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub build_time: String,
}
