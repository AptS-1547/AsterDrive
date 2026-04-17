use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
#[cfg(all(debug_assertions, feature = "openapi"))]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
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

/// 联系方式验证渠道
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(16))")]
#[serde(rename_all = "snake_case")]
pub enum VerificationChannel {
    #[sea_orm(string_value = "email")]
    Email,
    #[sea_orm(string_value = "phone")]
    Phone,
}

/// 联系方式验证用途
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
#[serde(rename_all = "snake_case")]
pub enum VerificationPurpose {
    #[sea_orm(string_value = "register_activation")]
    RegisterActivation,
    #[sea_orm(string_value = "contact_change")]
    ContactChange,
    #[sea_orm(string_value = "password_reset")]
    PasswordReset,
}

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

/// Raw JSON payload stored in `background_tasks.payload_json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, DeriveValueType)]
pub struct StoredTaskPayload(pub String);

impl AsRef<str> for StoredTaskPayload {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<String> for StoredTaskPayload {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<StoredTaskPayload> for String {
    fn from(value: StoredTaskPayload) -> Self {
        value.0
    }
}

/// Raw JSON payload stored in `background_tasks.result_json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, DeriveValueType)]
pub struct StoredTaskResult(pub String);

impl AsRef<str> for StoredTaskResult {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<String> for StoredTaskResult {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<StoredTaskResult> for String {
    fn from(value: StoredTaskResult) -> Self {
        value.0
    }
}

/// Raw JSON payload stored in `background_tasks.steps_json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, DeriveValueType)]
pub struct StoredTaskSteps(pub String);

impl AsRef<str> for StoredTaskSteps {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<String> for StoredTaskSteps {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<StoredTaskSteps> for String {
    fn from(value: StoredTaskSteps) -> Self {
        value.0
    }
}

/// Raw mixed owner payload stored in `resource_locks.owner_info`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, DeriveValueType)]
pub struct StoredLockOwnerInfo(pub String);

impl AsRef<str> for StoredLockOwnerInfo {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<String> for StoredLockOwnerInfo {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<StoredLockOwnerInfo> for String {
    fn from(value: StoredLockOwnerInfo) -> Self {
        value.0
    }
}

/// 后台任务类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
#[serde(rename_all = "snake_case")]
pub enum BackgroundTaskKind {
    #[sea_orm(string_value = "archive_extract")]
    ArchiveExtract,
    #[sea_orm(string_value = "archive_compress")]
    ArchiveCompress,
    #[sea_orm(string_value = "thumbnail_generate")]
    ThumbnailGenerate,
    #[sea_orm(string_value = "system_runtime")]
    SystemRuntime,
}

/// 后台任务状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(16))")]
#[serde(rename_all = "snake_case")]
pub enum BackgroundTaskStatus {
    #[sea_orm(string_value = "pending")]
    Pending,
    #[sea_orm(string_value = "processing")]
    Processing,
    #[sea_orm(string_value = "retry")]
    Retry,
    #[sea_orm(string_value = "succeeded")]
    Succeeded,
    #[sea_orm(string_value = "failed")]
    Failed,
    #[sea_orm(string_value = "canceled")]
    Canceled,
}

impl BackgroundTaskStatus {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Canceled)
    }
}

/// 团队成员角色
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(16))")]
#[serde(rename_all = "lowercase")]
pub enum TeamMemberRole {
    #[sea_orm(string_value = "owner")]
    Owner,
    #[sea_orm(string_value = "admin")]
    Admin,
    #[sea_orm(string_value = "member")]
    Member,
}

impl TeamMemberRole {
    pub fn can_manage_team(&self) -> bool {
        matches!(self, Self::Owner | Self::Admin)
    }

    pub fn is_owner(&self) -> bool {
        matches!(self, Self::Owner)
    }
}

/// 用户头像来源
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
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

/// Theme mode for the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    #[default]
    System,
    Light,
    Dark,
}

/// Color preset for the UI accent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum ColorPreset {
    #[default]
    Blue,
    Green,
    Purple,
    Orange,
}

/// File browser view mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum PrefViewMode {
    #[default]
    List,
    Grid,
}

/// Preferred gesture for opening items in the browser.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum BrowserOpenMode {
    #[default]
    SingleClick,
    DoubleClick,
}

/// Interface display language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum Language {
    #[default]
    En,
    Zh,
}

/// Stored user preferences (serialized as JSON in `users.config`).
/// Empty struct (all fields None) is treated as null by `get_preferences`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct UserPreferences {
    pub theme_mode: Option<ThemeMode>,
    pub color_preset: Option<ColorPreset>,
    pub view_mode: Option<PrefViewMode>,
    pub browser_open_mode: Option<BrowserOpenMode>,
    pub sort_by: Option<crate::api::pagination::SortBy>,
    pub sort_order: Option<crate::api::pagination::SortOrder>,
    pub language: Option<Language>,
    pub storage_event_stream_enabled: Option<bool>,
}

impl UserPreferences {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// Open-ended `users.config` payload:
/// structured built-in preferences + arbitrary custom frontend keys.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UserConfig {
    #[serde(flatten, default)]
    pub preferences: UserPreferences,
    #[serde(flatten, default)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

impl UserConfig {
    pub fn is_empty(&self) -> bool {
        self.preferences.is_empty() && self.extra.is_empty()
    }
}

/// Raw JSON string wrapper stored in `users.config`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, DeriveValueType)]
pub struct StoredUserConfig(pub String);

impl StoredUserConfig {
    pub fn parse(&self) -> serde_json::Result<UserConfig> {
        serde_json::from_str(&self.0)
    }

    pub fn from_config(config: &UserConfig) -> serde_json::Result<Self> {
        serde_json::to_string(config).map(Self)
    }
}

impl AsRef<str> for StoredUserConfig {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<String> for StoredUserConfig {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<StoredUserConfig> for String {
    fn from(value: StoredUserConfig) -> Self {
        value.0
    }
}

/// 审计日志动作
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(64))")]
#[serde(rename_all = "snake_case")]
pub enum AuditAction {
    #[sea_orm(string_value = "admin_create_user")]
    AdminCreateUser,
    #[sea_orm(string_value = "admin_create_team")]
    AdminCreateTeam,
    #[sea_orm(string_value = "admin_create_policy_group")]
    AdminCreatePolicyGroup,
    #[sea_orm(string_value = "admin_archive_team")]
    AdminArchiveTeam,
    #[sea_orm(string_value = "admin_restore_team")]
    AdminRestoreTeam,
    #[sea_orm(string_value = "admin_revoke_user_sessions")]
    AdminRevokeUserSessions,
    #[sea_orm(string_value = "admin_reset_user_password")]
    AdminResetUserPassword,
    #[sea_orm(string_value = "admin_update_team")]
    AdminUpdateTeam,
    #[sea_orm(string_value = "admin_update_user")]
    AdminUpdateUser,
    #[sea_orm(string_value = "admin_delete_policy_group")]
    AdminDeletePolicyGroup,
    #[sea_orm(string_value = "admin_migrate_policy_group_users")]
    AdminMigratePolicyGroupUsers,
    #[sea_orm(string_value = "admin_update_policy_group")]
    AdminUpdatePolicyGroup,
    #[sea_orm(string_value = "batch_copy")]
    BatchCopy,
    #[sea_orm(string_value = "batch_delete")]
    BatchDelete,
    #[sea_orm(string_value = "batch_move")]
    BatchMove,
    #[sea_orm(string_value = "config_action_execute")]
    ConfigActionExecute,
    #[sea_orm(string_value = "config_update")]
    ConfigUpdate,
    #[sea_orm(string_value = "file_copy")]
    FileCopy,
    #[sea_orm(string_value = "file_delete")]
    FileDelete,
    #[sea_orm(string_value = "file_download")]
    FileDownload,
    #[sea_orm(string_value = "file_edit")]
    FileEdit,
    #[sea_orm(string_value = "file_move")]
    FileMove,
    #[sea_orm(string_value = "file_rename")]
    FileRename,
    #[sea_orm(string_value = "file_upload")]
    FileUpload,
    #[sea_orm(string_value = "folder_copy")]
    FolderCopy,
    #[sea_orm(string_value = "folder_create")]
    FolderCreate,
    #[sea_orm(string_value = "folder_delete")]
    FolderDelete,
    #[sea_orm(string_value = "folder_move")]
    FolderMove,
    #[sea_orm(string_value = "folder_policy_change")]
    FolderPolicyChange,
    #[sea_orm(string_value = "folder_rename")]
    FolderRename,
    #[sea_orm(string_value = "share_batch_delete")]
    ShareBatchDelete,
    #[sea_orm(string_value = "share_create")]
    ShareCreate,
    #[sea_orm(string_value = "share_delete")]
    ShareDelete,
    #[sea_orm(string_value = "share_update")]
    ShareUpdate,
    #[sea_orm(string_value = "system_setup")]
    SystemSetup,
    #[sea_orm(string_value = "team_archive")]
    TeamArchive,
    #[sea_orm(string_value = "team_cleanup_expired")]
    TeamCleanupExpired,
    #[sea_orm(string_value = "team_create")]
    TeamCreate,
    #[sea_orm(string_value = "team_member_add")]
    TeamMemberAdd,
    #[sea_orm(string_value = "team_member_remove")]
    TeamMemberRemove,
    #[sea_orm(string_value = "team_member_update")]
    TeamMemberUpdate,
    #[sea_orm(string_value = "team_restore")]
    TeamRestore,
    #[sea_orm(string_value = "team_update")]
    TeamUpdate,
    #[sea_orm(string_value = "user_change_password")]
    UserChangePassword,
    #[sea_orm(string_value = "user_confirm_password_reset")]
    UserConfirmPasswordReset,
    #[sea_orm(string_value = "user_confirm_email_change")]
    UserConfirmEmailChange,
    #[sea_orm(string_value = "user_confirm_registration")]
    UserConfirmRegistration,
    #[sea_orm(string_value = "user_login")]
    UserLogin,
    #[sea_orm(string_value = "user_logout")]
    UserLogout,
    #[sea_orm(string_value = "user_request_email_change")]
    UserRequestEmailChange,
    #[sea_orm(string_value = "user_request_password_reset")]
    UserRequestPasswordReset,
    #[sea_orm(string_value = "user_register")]
    UserRegister,
    #[sea_orm(string_value = "user_resend_email_change")]
    UserResendEmailChange,
    #[sea_orm(string_value = "user_resend_registration")]
    UserResendRegistration,
}

impl AuditAction {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AdminCreateUser => "admin_create_user",
            Self::AdminCreateTeam => "admin_create_team",
            Self::AdminCreatePolicyGroup => "admin_create_policy_group",
            Self::AdminArchiveTeam => "admin_archive_team",
            Self::AdminRestoreTeam => "admin_restore_team",
            Self::AdminRevokeUserSessions => "admin_revoke_user_sessions",
            Self::AdminResetUserPassword => "admin_reset_user_password",
            Self::AdminUpdateTeam => "admin_update_team",
            Self::AdminUpdateUser => "admin_update_user",
            Self::AdminDeletePolicyGroup => "admin_delete_policy_group",
            Self::AdminMigratePolicyGroupUsers => "admin_migrate_policy_group_users",
            Self::AdminUpdatePolicyGroup => "admin_update_policy_group",
            Self::BatchCopy => "batch_copy",
            Self::BatchDelete => "batch_delete",
            Self::BatchMove => "batch_move",
            Self::ConfigActionExecute => "config_action_execute",
            Self::ConfigUpdate => "config_update",
            Self::FileCopy => "file_copy",
            Self::FileDelete => "file_delete",
            Self::FileDownload => "file_download",
            Self::FileEdit => "file_edit",
            Self::FileMove => "file_move",
            Self::FileRename => "file_rename",
            Self::FileUpload => "file_upload",
            Self::FolderCopy => "folder_copy",
            Self::FolderCreate => "folder_create",
            Self::FolderDelete => "folder_delete",
            Self::FolderMove => "folder_move",
            Self::FolderPolicyChange => "folder_policy_change",
            Self::FolderRename => "folder_rename",
            Self::ShareBatchDelete => "share_batch_delete",
            Self::ShareCreate => "share_create",
            Self::ShareDelete => "share_delete",
            Self::ShareUpdate => "share_update",
            Self::SystemSetup => "system_setup",
            Self::TeamArchive => "team_archive",
            Self::TeamCleanupExpired => "team_cleanup_expired",
            Self::TeamCreate => "team_create",
            Self::TeamMemberAdd => "team_member_add",
            Self::TeamMemberRemove => "team_member_remove",
            Self::TeamMemberUpdate => "team_member_update",
            Self::TeamRestore => "team_restore",
            Self::TeamUpdate => "team_update",
            Self::UserChangePassword => "user_change_password",
            Self::UserConfirmPasswordReset => "user_confirm_password_reset",
            Self::UserConfirmEmailChange => "user_confirm_email_change",
            Self::UserConfirmRegistration => "user_confirm_registration",
            Self::UserLogin => "user_login",
            Self::UserLogout => "user_logout",
            Self::UserRequestEmailChange => "user_request_email_change",
            Self::UserRequestPasswordReset => "user_request_password_reset",
            Self::UserRegister => "user_register",
            Self::UserResendEmailChange => "user_resend_email_change",
            Self::UserResendRegistration => "user_resend_registration",
        }
    }

    pub fn from_str_name(value: &str) -> Option<Self> {
        match value {
            "admin_create_user" => Some(Self::AdminCreateUser),
            "admin_create_team" => Some(Self::AdminCreateTeam),
            "admin_create_policy_group" => Some(Self::AdminCreatePolicyGroup),
            "admin_archive_team" => Some(Self::AdminArchiveTeam),
            "admin_restore_team" => Some(Self::AdminRestoreTeam),
            "admin_revoke_user_sessions" => Some(Self::AdminRevokeUserSessions),
            "admin_reset_user_password" => Some(Self::AdminResetUserPassword),
            "admin_update_team" => Some(Self::AdminUpdateTeam),
            "admin_update_user" => Some(Self::AdminUpdateUser),
            "admin_delete_policy_group" => Some(Self::AdminDeletePolicyGroup),
            "admin_migrate_policy_group_users" => Some(Self::AdminMigratePolicyGroupUsers),
            "admin_update_policy_group" => Some(Self::AdminUpdatePolicyGroup),
            "batch_copy" => Some(Self::BatchCopy),
            "batch_delete" => Some(Self::BatchDelete),
            "batch_move" => Some(Self::BatchMove),
            "config_action_execute" => Some(Self::ConfigActionExecute),
            "config_update" => Some(Self::ConfigUpdate),
            "file_copy" => Some(Self::FileCopy),
            "file_delete" => Some(Self::FileDelete),
            "file_download" => Some(Self::FileDownload),
            "file_edit" => Some(Self::FileEdit),
            "file_move" => Some(Self::FileMove),
            "file_rename" => Some(Self::FileRename),
            "file_upload" => Some(Self::FileUpload),
            "folder_copy" => Some(Self::FolderCopy),
            "folder_create" => Some(Self::FolderCreate),
            "folder_delete" => Some(Self::FolderDelete),
            "folder_move" => Some(Self::FolderMove),
            "folder_policy_change" => Some(Self::FolderPolicyChange),
            "folder_rename" => Some(Self::FolderRename),
            "share_batch_delete" => Some(Self::ShareBatchDelete),
            "share_create" => Some(Self::ShareCreate),
            "share_delete" => Some(Self::ShareDelete),
            "share_update" => Some(Self::ShareUpdate),
            "system_setup" => Some(Self::SystemSetup),
            "team_archive" => Some(Self::TeamArchive),
            "team_cleanup_expired" => Some(Self::TeamCleanupExpired),
            "team_create" => Some(Self::TeamCreate),
            "team_member_add" => Some(Self::TeamMemberAdd),
            "team_member_remove" => Some(Self::TeamMemberRemove),
            "team_member_update" => Some(Self::TeamMemberUpdate),
            "team_restore" => Some(Self::TeamRestore),
            "team_update" => Some(Self::TeamUpdate),
            "user_change_password" => Some(Self::UserChangePassword),
            "user_confirm_password_reset" => Some(Self::UserConfirmPasswordReset),
            "user_confirm_email_change" => Some(Self::UserConfirmEmailChange),
            "user_confirm_registration" => Some(Self::UserConfirmRegistration),
            "user_login" => Some(Self::UserLogin),
            "user_logout" => Some(Self::UserLogout),
            "user_request_email_change" => Some(Self::UserRequestEmailChange),
            "user_request_password_reset" => Some(Self::UserRequestPasswordReset),
            "user_register" => Some(Self::UserRegister),
            "user_resend_email_change" => Some(Self::UserResendEmailChange),
            "user_resend_registration" => Some(Self::UserResendRegistration),
            _ => None,
        }
    }
}

impl AsRef<str> for AuditAction {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for AuditAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

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
            Self::Number => "number",
            Self::Boolean => "boolean",
        }
    }

    pub fn from_str_name(value: &str) -> Option<Self> {
        match value {
            "string" => Some(Self::String),
            "multiline" => Some(Self::Multiline),
            "number" => Some(Self::Number),
            "boolean" => Some(Self::Boolean),
            _ => None,
        }
    }

    pub const fn is_multiline(self) -> bool {
        matches!(self, Self::Multiline)
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

/// 存储驱动类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
#[serde(rename_all = "lowercase")]
pub enum DriverType {
    #[sea_orm(string_value = "local")]
    Local,
    #[sea_orm(string_value = "s3")]
    S3,
}

/// 上传 session 状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum UploadMode {
    Direct,
    Chunked,
    Presigned,
    PresignedMultipart,
}

/// S3 上传传输策略（存储策略 options JSON）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum S3UploadStrategy {
    /// 服务端将请求体直接中继到 S3，不落本地临时文件
    RelayStream,
    /// 浏览器直传 S3 / MinIO
    Presigned,
}

/// S3 下载传输策略（存储策略 options JSON）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum S3DownloadStrategy {
    /// 服务端从 S3 拉流后回传给客户端
    RelayStream,
    /// 服务端完成鉴权后重定向到 S3 presigned GET URL
    Presigned,
}

/// Raw JSON array stored in `storage_policies.allowed_types`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, DeriveValueType)]
pub struct StoredStoragePolicyAllowedTypes(pub String);

impl StoredStoragePolicyAllowedTypes {
    pub const EMPTY_JSON: &str = "[]";

    pub fn empty() -> Self {
        Self(Self::EMPTY_JSON.to_string())
    }
}

impl AsRef<str> for StoredStoragePolicyAllowedTypes {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<String> for StoredStoragePolicyAllowedTypes {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<StoredStoragePolicyAllowedTypes> for String {
    fn from(value: StoredStoragePolicyAllowedTypes) -> Self {
        value.0
    }
}

/// Raw JSON object stored in `storage_policies.options`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, DeriveValueType)]
pub struct StoredStoragePolicyOptions(pub String);

impl StoredStoragePolicyOptions {
    pub const EMPTY_JSON: &str = "{}";

    pub fn empty() -> Self {
        Self(Self::EMPTY_JSON.to_string())
    }
}

impl AsRef<str> for StoredStoragePolicyOptions {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<String> for StoredStoragePolicyOptions {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<StoredStoragePolicyOptions> for String {
    fn from(value: StoredStoragePolicyOptions) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct StoragePolicyOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub s3_upload_strategy: Option<S3UploadStrategy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub s3_download_strategy: Option<S3DownloadStrategy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_dedup: Option<bool>,
}

impl StoragePolicyOptions {
    pub fn effective_s3_upload_strategy(&self) -> S3UploadStrategy {
        self.s3_upload_strategy
            .unwrap_or(S3UploadStrategy::RelayStream)
    }

    pub fn effective_s3_download_strategy(&self) -> S3DownloadStrategy {
        self.s3_download_strategy
            .unwrap_or(S3DownloadStrategy::RelayStream)
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

pub fn serialize_storage_policy_options(
    options: &StoragePolicyOptions,
) -> std::result::Result<StoredStoragePolicyOptions, serde_json::Error> {
    serde_json::to_string(options).map(StoredStoragePolicyOptions)
}

pub fn parse_storage_policy_allowed_types(raw: &str) -> Vec<String> {
    serde_json::from_str(raw).unwrap_or_else(|error| {
        if !raw.is_empty() && raw != StoredStoragePolicyAllowedTypes::EMPTY_JSON {
            tracing::warn!("invalid storage policy allowed_types JSON '{raw}': {error}");
        }
        Vec::new()
    })
}

pub fn serialize_storage_policy_allowed_types(
    allowed_types: &[String],
) -> std::result::Result<StoredStoragePolicyAllowedTypes, serde_json::Error> {
    serde_json::to_string(allowed_types).map(StoredStoragePolicyAllowedTypes)
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(16))")]
#[serde(rename_all = "lowercase")]
pub enum EntityType {
    #[sea_orm(string_value = "file")]
    File,
    #[sea_orm(string_value = "folder")]
    Folder,
}

/// JWT Token 类型（不存 DB）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
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

#[cfg(test)]
mod tests {
    use super::{
        S3DownloadStrategy, S3UploadStrategy, StoragePolicyOptions, parse_storage_policy_options,
    };

    #[test]
    fn s3_strategy_defaults_to_relay_stream() {
        let options = StoragePolicyOptions::default();
        assert_eq!(
            options.effective_s3_upload_strategy(),
            S3UploadStrategy::RelayStream
        );
    }

    #[test]
    fn explicit_presigned_strategy_maps_to_presigned() {
        let options = parse_storage_policy_options(r#"{"s3_upload_strategy":"presigned"}"#);
        assert_eq!(
            options.effective_s3_upload_strategy(),
            S3UploadStrategy::Presigned
        );
    }

    #[test]
    fn s3_download_strategy_defaults_to_relay_stream() {
        let options = StoragePolicyOptions::default();
        assert_eq!(
            options.effective_s3_download_strategy(),
            S3DownloadStrategy::RelayStream
        );
    }

    #[test]
    fn explicit_presigned_download_strategy_maps_to_presigned() {
        let options = parse_storage_policy_options(r#"{"s3_download_strategy":"presigned"}"#);
        assert_eq!(
            options.effective_s3_download_strategy(),
            S3DownloadStrategy::Presigned
        );
    }

    #[test]
    fn removed_proxy_tempfile_strategy_falls_back_to_relay_stream() {
        let options = parse_storage_policy_options(r#"{"s3_upload_strategy":"proxy_tempfile"}"#);
        assert_eq!(
            options.effective_s3_upload_strategy(),
            S3UploadStrategy::RelayStream
        );
    }

    #[test]
    fn serialize_storage_policy_options_omits_default_fields() {
        let json = serde_json::to_string(&StoragePolicyOptions::default()).unwrap();
        assert_eq!(json, "{}");

        let json = serde_json::to_string(&StoragePolicyOptions {
            s3_upload_strategy: Some(S3UploadStrategy::Presigned),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(json, r#"{"s3_upload_strategy":"presigned"}"#);

        let json = serde_json::to_string(&StoragePolicyOptions {
            s3_download_strategy: Some(S3DownloadStrategy::Presigned),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(json, r#"{"s3_download_strategy":"presigned"}"#);
    }
}
