use actix_web::HttpRequest;
use chrono::{DateTime, Duration, Utc};
use sea_orm::Set;
use serde::{Deserialize, Serialize};
use std::fmt;
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::IntoParams;

use crate::api::pagination::{OffsetPage, load_offset_page};
use crate::db::repository::audit_log_repo;
use crate::entities::audit_log;
use crate::errors::Result;
use crate::runtime::AppState;
use crate::services::auth_service::Claims;
use crate::types::{TeamMemberRole, UserRole, UserStatus};

const DEFAULT_RETENTION_DAYS: i64 = 90;

/// 从 HttpRequest 提取的审计上下文
pub struct AuditContext {
    pub user_id: i64,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuditAction {
    AdminCreateUser,
    AdminCreateTeam,
    AdminCreatePolicyGroup,
    AdminArchiveTeam,
    AdminRestoreTeam,
    AdminRevokeUserSessions,
    AdminResetUserPassword,
    AdminUpdateTeam,
    AdminUpdateUser,
    AdminDeletePolicyGroup,
    AdminMigratePolicyGroupUsers,
    AdminUpdatePolicyGroup,
    BatchCopy,
    BatchDelete,
    BatchMove,
    ConfigUpdate,
    FileCopy,
    FileDelete,
    FileDownload,
    FileEdit,
    FileMove,
    FileRename,
    FileUpload,
    FolderCopy,
    FolderCreate,
    FolderDelete,
    FolderMove,
    FolderPolicyChange,
    FolderRename,
    ShareBatchDelete,
    ShareCreate,
    ShareDelete,
    ShareUpdate,
    SystemSetup,
    TeamArchive,
    TeamCleanupExpired,
    TeamCreate,
    TeamRestore,
    TeamUpdate,
    UserChangePassword,
    UserLogin,
    UserLogout,
    UserRegister,
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
            Self::TeamRestore => "team_restore",
            Self::TeamUpdate => "team_update",
            Self::UserChangePassword => "user_change_password",
            Self::UserLogin => "user_login",
            Self::UserLogout => "user_logout",
            Self::UserRegister => "user_register",
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

#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(IntoParams))]
pub struct AuditLogFilterQuery {
    pub user_id: Option<i64>,
    pub action: Option<String>,
    pub entity_type: Option<String>,
    pub after: Option<String>,
    pub before: Option<String>,
}

pub struct AuditLogFilters {
    pub user_id: Option<i64>,
    pub action: Option<String>,
    pub entity_type: Option<String>,
    pub after: Option<DateTime<Utc>>,
    pub before: Option<DateTime<Utc>>,
}

impl AuditLogFilters {
    pub fn from_query(query: &AuditLogFilterQuery) -> Self {
        Self {
            user_id: query.user_id,
            action: query.action.clone(),
            entity_type: query.entity_type.clone(),
            after: query
                .after
                .as_deref()
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&Utc)),
            before: query
                .before
                .as_deref()
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&Utc)),
        }
    }
}

#[derive(Serialize)]
pub struct ConfigUpdateDetails<'a> {
    pub value: &'a str,
}

#[derive(Serialize)]
pub struct AdminCreateUserDetails<'a> {
    pub email: &'a str,
    pub role: UserRole,
    pub status: UserStatus,
    pub storage_quota: i64,
    pub policy_group_id: Option<i64>,
}

#[derive(Serialize)]
pub struct AdminUpdateUserDetails {
    pub role: UserRole,
    pub status: UserStatus,
    pub storage_quota: i64,
    pub policy_group_id: Option<i64>,
}

#[derive(Serialize)]
pub struct PolicyGroupAuditDetails {
    pub is_default: bool,
    pub is_enabled: bool,
    pub item_count: usize,
}

#[derive(Serialize)]
pub struct PolicyGroupMigrationDetails<'a> {
    pub source_group_id: i64,
    pub source_group_name: &'a str,
    pub target_group_id: i64,
    pub target_group_name: &'a str,
    pub affected_users: u64,
    pub migrated_assignments: u64,
}

#[derive(Serialize)]
pub struct BatchDeleteDetails<'a> {
    pub file_ids: &'a [i64],
    pub folder_ids: &'a [i64],
    pub succeeded: u32,
    pub failed: u32,
}

#[derive(Serialize)]
pub struct BatchTransferDetails<'a> {
    pub file_ids: &'a [i64],
    pub folder_ids: &'a [i64],
    pub target_folder_id: Option<i64>,
    pub succeeded: u32,
    pub failed: u32,
}

#[derive(Serialize)]
pub struct ShareBatchDeleteDetails<'a> {
    pub share_ids: &'a [i64],
    pub succeeded: u32,
    pub failed: u32,
}

#[derive(Serialize)]
pub struct ShareUpdateDetails {
    pub has_password: bool,
    pub expires_at: Option<DateTime<Utc>>,
    pub max_downloads: i64,
}

#[derive(Serialize)]
pub struct TeamAuditDetails<'a> {
    pub description: &'a str,
    pub member_count: u64,
    pub storage_quota: i64,
    pub policy_group_id: Option<i64>,
    pub archived_at: Option<DateTime<Utc>>,
    pub actor_role: Option<TeamMemberRole>,
}

#[derive(Serialize)]
pub struct TeamCleanupAuditDetails {
    pub archived_at: Option<DateTime<Utc>>,
    pub retention_days: i64,
}

pub fn details<T: Serialize>(value: T) -> Option<serde_json::Value> {
    match serde_json::to_value(value) {
        Ok(value) => Some(value),
        Err(e) => {
            tracing::warn!("failed to serialize audit details: {e}");
            None
        }
    }
}

impl AuditContext {
    pub fn system() -> Self {
        Self {
            user_id: 0,
            ip_address: None,
            user_agent: None,
        }
    }

    pub fn from_request(req: &HttpRequest, claims: &Claims) -> Self {
        let ip_address = req
            .connection_info()
            .realip_remote_addr()
            .map(|s| s.to_string());
        let user_agent = req
            .headers()
            .get("user-agent")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
        Self {
            user_id: claims.user_id,
            ip_address,
            user_agent,
        }
    }
}

/// Fire-and-forget 审计日志。DB 错误只 warn 不传播。
pub async fn log(
    state: &AppState,
    ctx: &AuditContext,
    action: AuditAction,
    entity_type: Option<&str>,
    entity_id: Option<i64>,
    entity_name: Option<&str>,
    details: Option<serde_json::Value>,
) {
    // 检查运行时配置
    if matches!(
        state.runtime_config.get_bool("audit_log_enabled"),
        Some(false)
    ) {
        return;
    }

    let model = audit_log::ActiveModel {
        id: Default::default(),
        user_id: Set(ctx.user_id),
        action: Set(action.to_string()),
        entity_type: Set(entity_type.map(|s| s.to_string())),
        entity_id: Set(entity_id),
        entity_name: Set(entity_name.map(|s| s.to_string())),
        details: Set(details.map(|v| v.to_string())),
        ip_address: Set(ctx.ip_address.clone()),
        user_agent: Set(ctx.user_agent.clone()),
        created_at: Set(Utc::now()),
    };

    if let Err(e) = audit_log_repo::create(&state.db, model).await {
        tracing::warn!("failed to write audit log: {e}");
    }
}

pub async fn query(
    state: &AppState,
    filters: AuditLogFilters,
    limit: u64,
    offset: u64,
) -> Result<OffsetPage<audit_log::Model>> {
    load_offset_page(limit, offset, 200, |limit, offset| async move {
        audit_log_repo::find_with_filters(
            &state.db,
            filters.user_id,
            filters.action.as_deref(),
            filters.entity_type.as_deref(),
            filters.after,
            filters.before,
            limit,
            offset,
        )
        .await
    })
    .await
}

/// 清理过期审计日志
pub async fn cleanup_expired(state: &AppState) -> Result<u64> {
    let retention_days = state
        .runtime_config
        .get_i64("audit_log_retention_days")
        .unwrap_or_else(|| {
            if let Some(raw) = state.runtime_config.get("audit_log_retention_days") {
                tracing::warn!(
                    "invalid audit_log_retention_days value '{}', using default",
                    raw
                );
            }
            DEFAULT_RETENTION_DAYS
        });

    let cutoff = Utc::now() - Duration::days(retention_days);
    let deleted = audit_log_repo::delete_before(&state.db, cutoff).await?;
    if deleted > 0 {
        tracing::info!("cleaned up {deleted} expired audit log entries");
    }
    Ok(deleted)
}

#[cfg(test)]
mod tests {
    use super::AuditAction;

    #[test]
    fn audit_action_strings_match_existing_contract() {
        let cases = [
            (AuditAction::AdminCreateUser, "admin_create_user"),
            (AuditAction::AdminCreateTeam, "admin_create_team"),
            (
                AuditAction::AdminCreatePolicyGroup,
                "admin_create_policy_group",
            ),
            (AuditAction::AdminArchiveTeam, "admin_archive_team"),
            (AuditAction::AdminRestoreTeam, "admin_restore_team"),
            (
                AuditAction::AdminDeletePolicyGroup,
                "admin_delete_policy_group",
            ),
            (
                AuditAction::AdminMigratePolicyGroupUsers,
                "admin_migrate_policy_group_users",
            ),
            (
                AuditAction::AdminRevokeUserSessions,
                "admin_revoke_user_sessions",
            ),
            (
                AuditAction::AdminResetUserPassword,
                "admin_reset_user_password",
            ),
            (AuditAction::AdminUpdateTeam, "admin_update_team"),
            (
                AuditAction::AdminUpdatePolicyGroup,
                "admin_update_policy_group",
            ),
            (AuditAction::AdminUpdateUser, "admin_update_user"),
            (AuditAction::BatchCopy, "batch_copy"),
            (AuditAction::BatchDelete, "batch_delete"),
            (AuditAction::BatchMove, "batch_move"),
            (AuditAction::ConfigUpdate, "config_update"),
            (AuditAction::FileCopy, "file_copy"),
            (AuditAction::FileDelete, "file_delete"),
            (AuditAction::FileDownload, "file_download"),
            (AuditAction::FileEdit, "file_edit"),
            (AuditAction::FileMove, "file_move"),
            (AuditAction::FileRename, "file_rename"),
            (AuditAction::FileUpload, "file_upload"),
            (AuditAction::FolderCopy, "folder_copy"),
            (AuditAction::FolderCreate, "folder_create"),
            (AuditAction::FolderDelete, "folder_delete"),
            (AuditAction::FolderMove, "folder_move"),
            (AuditAction::FolderPolicyChange, "folder_policy_change"),
            (AuditAction::FolderRename, "folder_rename"),
            (AuditAction::ShareBatchDelete, "share_batch_delete"),
            (AuditAction::ShareCreate, "share_create"),
            (AuditAction::ShareDelete, "share_delete"),
            (AuditAction::ShareUpdate, "share_update"),
            (AuditAction::SystemSetup, "system_setup"),
            (AuditAction::TeamArchive, "team_archive"),
            (AuditAction::TeamCleanupExpired, "team_cleanup_expired"),
            (AuditAction::TeamCreate, "team_create"),
            (AuditAction::TeamRestore, "team_restore"),
            (AuditAction::TeamUpdate, "team_update"),
            (AuditAction::UserChangePassword, "user_change_password"),
            (AuditAction::UserLogin, "user_login"),
            (AuditAction::UserLogout, "user_logout"),
            (AuditAction::UserRegister, "user_register"),
        ];

        for (action, expected) in cases {
            assert_eq!(action.as_str(), expected);
            assert_eq!(action.as_ref(), expected);
            assert_eq!(action.to_string(), expected);
        }
    }
}
