use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// PATCH 请求里的可空字段三态：
/// - `Absent`：字段未传，保持不变
/// - `Null`：字段显式传 `null`，清空该字段
/// - `Value`：字段传具体值，更新为该值
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NullablePatch<T> {
    #[default]
    Absent,
    Null,
    Value(T),
}

impl<T> NullablePatch<T> {
    pub fn is_present(&self) -> bool {
        !matches!(self, Self::Absent)
    }
}

impl<T> From<Option<T>> for NullablePatch<T> {
    fn from(value: Option<T>) -> Self {
        match value {
            Some(value) => Self::Value(value),
            None => Self::Null,
        }
    }
}

impl<'de, T> Deserialize<'de> for NullablePatch<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(match Option::<T>::deserialize(deserializer)? {
            Some(value) => Self::Value(value),
            None => Self::Null,
        })
    }
}

/// 用户角色
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize, ToSchema,
)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(16))")]
#[serde(rename_all = "lowercase")]
pub enum UserRole {
    #[sea_orm(string_value = "admin")]
    Admin,
    #[sea_orm(string_value = "user")]
    User,
}

impl UserRole {
    pub fn is_admin(&self) -> bool {
        matches!(self, Self::Admin)
    }
}

/// 用户状态
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize, ToSchema,
)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(16))")]
#[serde(rename_all = "lowercase")]
pub enum UserStatus {
    #[sea_orm(string_value = "active")]
    Active,
    #[sea_orm(string_value = "disabled")]
    Disabled,
}

impl UserStatus {
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Active)
    }
}

/// 用户头像来源
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize, ToSchema,
)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(16))")]
#[serde(rename_all = "snake_case")]
pub enum AvatarSource {
    #[sea_orm(string_value = "none")]
    None,
    #[sea_orm(string_value = "gravatar")]
    Gravatar,
    #[sea_orm(string_value = "upload")]
    Upload,
}

/// 存储驱动类型
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize, ToSchema,
)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
#[serde(rename_all = "lowercase")]
pub enum DriverType {
    #[sea_orm(string_value = "local")]
    Local,
    #[sea_orm(string_value = "s3")]
    S3,
}

/// 上传 session 状态
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize, ToSchema,
)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(16))")]
#[serde(rename_all = "lowercase")]
pub enum UploadSessionStatus {
    #[sea_orm(string_value = "uploading")]
    Uploading,
    #[sea_orm(string_value = "assembling")]
    Assembling,
    #[sea_orm(string_value = "completed")]
    Completed,
    #[sea_orm(string_value = "failed")]
    Failed,
    #[sea_orm(string_value = "presigned")]
    Presigned,
}

/// 上传模式（不存 DB，仅 API 响应用）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum UploadMode {
    Direct,
    Chunked,
    Presigned,
    PresignedMultipart,
}

/// S3 上传传输策略（存储策略 options JSON）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum S3UploadStrategy {
    /// 先落服务端临时文件，再写入 S3
    ProxyTempfile,
    /// 服务端将请求体直接中继到 S3，不落本地临时文件
    RelayStream,
    /// 浏览器直传 S3 / MinIO
    Presigned,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct StoragePolicyOptions {
    #[serde(default)]
    pub presigned_upload: bool,
    #[serde(default)]
    pub s3_upload_strategy: Option<S3UploadStrategy>,
}

impl StoragePolicyOptions {
    pub fn effective_s3_upload_strategy(&self) -> S3UploadStrategy {
        match self.s3_upload_strategy {
            Some(strategy) => strategy,
            None if self.presigned_upload => S3UploadStrategy::Presigned,
            None => S3UploadStrategy::ProxyTempfile,
        }
    }
}

pub fn parse_storage_policy_options(options: &str) -> StoragePolicyOptions {
    serde_json::from_str(options).unwrap_or_else(|e| {
        if !options.is_empty() && options != "{}" {
            tracing::warn!("invalid storage policy options JSON '{options}': {e}");
        }
        StoragePolicyOptions::default()
    })
}

pub const S3_MULTIPART_MIN_PART_SIZE: i64 = 5 * 1024 * 1024;

pub fn effective_s3_multipart_chunk_size(configured: i64) -> i64 {
    if configured <= 0 {
        S3_MULTIPART_MIN_PART_SIZE
    } else {
        configured.max(S3_MULTIPART_MIN_PART_SIZE)
    }
}

/// 实体类型（文件/文件夹）
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize, ToSchema,
)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(16))")]
#[serde(rename_all = "lowercase")]
pub enum EntityType {
    #[sea_orm(string_value = "file")]
    File,
    #[sea_orm(string_value = "folder")]
    Folder,
}

/// JWT Token 类型（不存 DB）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum TokenType {
    Access,
    Refresh,
}

impl TokenType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Access => "access",
            Self::Refresh => "refresh",
        }
    }
}
