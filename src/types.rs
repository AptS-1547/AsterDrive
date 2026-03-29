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
