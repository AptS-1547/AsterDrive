use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::ToSchema;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
#[cfg_attr(all(debug_assertions, feature = "openapi"), schema(as = SystemConfig))]
#[sea_orm(table_name = "system_config")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub key: String,
    pub value: String,
    /// 值类型：string / multiline / number / boolean
    #[serde(default = "default_value_type")]
    pub value_type: String,
    /// 修改后是否需要重启才生效
    #[serde(default)]
    pub requires_restart: bool,
    /// 是否敏感值（前端脱敏显示）
    #[serde(default)]
    pub is_sensitive: bool,
    /// 来源：system（代码定义）/ custom（用户创建）
    #[serde(default = "default_source")]
    pub source: String,
    /// 自定义配置的命名空间，系统配置为 ""
    #[serde(default)]
    pub namespace: String,
    /// 分类（前端分组用）
    #[serde(default)]
    pub category: String,
    /// 描述
    #[serde(default)]
    pub description: String,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = String))]
    pub updated_at: DateTimeUtc,
    pub updated_by: Option<i64>,
}

fn default_value_type() -> String {
    "string".to_string()
}

fn default_source() -> String {
    "system".to_string()
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
