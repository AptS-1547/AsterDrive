use crate::config::definitions::ALL_CONFIGS;
use crate::types::SystemConfigValueType;
use serde::Serialize;
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::ToSchema;

#[derive(Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct ConfigSchemaItem {
    pub key: String,
    pub label_i18n_key: String,
    pub description_i18n_key: String,
    pub value_type: SystemConfigValueType,
    pub category: String,
    pub description: String,
    pub requires_restart: bool,
    pub is_sensitive: bool,
}

#[derive(Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct TemplateVariableItem {
    pub token: String,
    pub label_i18n_key: String,
    pub description_i18n_key: String,
}

#[derive(Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct TemplateVariableGroup {
    pub category: String,
    pub template_code: String,
    pub label_i18n_key: String,
    pub variables: Vec<TemplateVariableItem>,
}

pub fn get_schema() -> Vec<ConfigSchemaItem> {
    ALL_CONFIGS
        .iter()
        .map(|def| ConfigSchemaItem {
            key: def.key.to_string(),
            label_i18n_key: def.label_i18n_key.to_string(),
            description_i18n_key: def.description_i18n_key.to_string(),
            value_type: def.value_type,
            category: def.category.to_string(),
            description: def.description.to_string(),
            requires_restart: def.requires_restart,
            is_sensitive: def.is_sensitive,
        })
        .collect()
}

pub fn list_template_variable_groups() -> Vec<TemplateVariableGroup> {
    crate::services::mail_template::list_template_variable_groups()
        .into_iter()
        .map(|group| TemplateVariableGroup {
            category: group.category,
            template_code: group.template_code,
            label_i18n_key: group.label_i18n_key,
            variables: group
                .variables
                .into_iter()
                .map(|variable| TemplateVariableItem {
                    token: variable.token,
                    label_i18n_key: variable.label_i18n_key,
                    description_i18n_key: variable.description_i18n_key,
                })
                .collect(),
        })
        .collect()
}
