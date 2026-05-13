use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::ToSchema;

/// 邮件模板代码
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
#[serde(rename_all = "snake_case")]
pub enum MailTemplateCode {
    #[sea_orm(string_value = "register_activation")]
    RegisterActivation,
    #[sea_orm(string_value = "contact_change_confirmation")]
    ContactChangeConfirmation,
    #[sea_orm(string_value = "password_reset")]
    PasswordReset,
    #[sea_orm(string_value = "password_reset_notice")]
    PasswordResetNotice,
    #[sea_orm(string_value = "contact_change_notice")]
    ContactChangeNotice,
}

impl MailTemplateCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RegisterActivation => "register_activation",
            Self::ContactChangeConfirmation => "contact_change_confirmation",
            Self::PasswordReset => "password_reset",
            Self::PasswordResetNotice => "password_reset_notice",
            Self::ContactChangeNotice => "contact_change_notice",
        }
    }
}

/// Raw JSON payload stored in `mail_outbox.payload_json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, DeriveValueType)]
pub struct StoredMailPayload(pub String);

impl StoredMailPayload {
    pub const CLEARED_JSON: &str = "{}";

    pub fn cleared() -> Self {
        Self(Self::CLEARED_JSON.to_string())
    }
}

impl AsRef<str> for StoredMailPayload {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<String> for StoredMailPayload {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<StoredMailPayload> for String {
    fn from(value: StoredMailPayload) -> Self {
        value.0
    }
}

/// 邮件 outbox 状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(16))")]
#[serde(rename_all = "snake_case")]
pub enum MailOutboxStatus {
    #[sea_orm(string_value = "pending")]
    Pending,
    #[sea_orm(string_value = "processing")]
    Processing,
    #[sea_orm(string_value = "retry")]
    Retry,
    #[sea_orm(string_value = "sent")]
    Sent,
    #[sea_orm(string_value = "failed")]
    Failed,
}

impl MailOutboxStatus {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Sent | Self::Failed)
    }
}
