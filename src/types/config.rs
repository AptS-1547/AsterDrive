use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt;
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::ToSchema;

/// 运行时配置值类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
#[serde(rename_all = "snake_case")]
pub enum SystemConfigValueType {
    #[sea_orm(string_value = "string")]
    String,
    #[sea_orm(string_value = "multiline")]
    Multiline,
    #[sea_orm(string_value = "string_array")]
    StringArray,
    #[sea_orm(string_value = "number")]
    Number,
    #[sea_orm(string_value = "boolean")]
    Boolean,
}

impl SystemConfigValueType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::String => "string",
            Self::Multiline => "multiline",
            Self::StringArray => "string_array",
            Self::Number => "number",
            Self::Boolean => "boolean",
        }
    }

    pub fn from_str_name(value: &str) -> Option<Self> {
        match value {
            "string" => Some(Self::String),
            "multiline" => Some(Self::Multiline),
            "string_array" => Some(Self::StringArray),
            "number" => Some(Self::Number),
            "boolean" => Some(Self::Boolean),
            _ => None,
        }
    }

    pub const fn is_multiline(self) -> bool {
        matches!(self, Self::Multiline)
    }

    pub const fn is_string_array(self) -> bool {
        matches!(self, Self::StringArray)
    }
}

impl fmt::Display for SystemConfigValueType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// 运行时配置来源
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(16))")]
#[serde(rename_all = "snake_case")]
pub enum SystemConfigSource {
    #[sea_orm(string_value = "system")]
    System,
    #[sea_orm(string_value = "custom")]
    Custom,
}

impl SystemConfigSource {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Custom => "custom",
        }
    }

    pub fn from_str_name(value: &str) -> Option<Self> {
        match value {
            "system" => Some(Self::System),
            "custom" => Some(Self::Custom),
            _ => None,
        }
    }
}

impl fmt::Display for SystemConfigSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
