//! 配置服务聚合入口。

mod actions;
mod public;
mod schema;
mod system;

pub use actions::{
    ConfigActionResult, ConfigActionType, ExecuteConfigActionInput, MAIL_CONFIG_ACTION_KEY,
    execute_action, execute_action_with_audit,
};
pub use public::{
    PublicBranding, get_public_branding, get_public_preview_apps, get_public_thumbnail_support,
};
pub use schema::{
    ConfigSchemaItem, TemplateVariableGroup, TemplateVariableItem, get_schema,
    list_template_variable_groups,
};
pub use system::{
    SystemConfig, SystemConfigValue, delete, get_by_key, list_paginated, set, set_with_audit,
};
