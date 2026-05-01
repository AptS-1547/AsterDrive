//! 服务模块：`audit_service`。

use actix_web::HttpRequest;
use chrono::{DateTime, Duration, Utc};
use sea_orm::Set;
use serde::{Deserialize, Serialize};
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::{IntoParams, ToSchema};

use crate::api::pagination::{OffsetPage, load_offset_page};
use crate::db::repository::{audit_log_repo, user_repo};
use crate::entities::audit_log;
use crate::errors::Result;
use crate::runtime::PrimaryAppState;
use crate::services::auth_service::Claims;
pub use crate::types::AuditAction;
use crate::types::{TeamMemberRole, UserRole, UserStatus};
use std::collections::{HashMap, HashSet};

const DEFAULT_RETENTION_DAYS: i64 = 90;
const MAX_AUDIT_IP_ADDRESS_LEN: usize = 45;
const MAX_AUDIT_USER_AGENT_LEN: usize = 512;

/// 从 HttpRequest 提取的审计上下文
pub struct AuditContext {
    pub user_id: i64,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

/// 从 HttpRequest 提取的请求级审计元信息。
pub struct AuditRequestInfo {
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(IntoParams))]
pub struct AuditLogFilterQuery {
    pub user_id: Option<i64>,
    pub action: Option<String>,
    pub entity_type: Option<String>,
    pub entity_id: Option<i64>,
    pub after: Option<String>,
    pub before: Option<String>,
}

pub struct AuditLogFilters {
    pub user_id: Option<i64>,
    pub action: Option<String>,
    pub entity_type: Option<String>,
    pub entity_id: Option<i64>,
    pub after: Option<DateTime<Utc>>,
    pub before: Option<DateTime<Utc>>,
}

impl AuditLogFilters {
    pub fn from_query(query: &AuditLogFilterQuery) -> Self {
        Self {
            user_id: query.user_id,
            action: query.action.clone(),
            entity_type: query.entity_type.clone(),
            entity_id: query.entity_id,
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

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct AuditLogEntry {
    pub id: i64,
    pub user_id: i64,
    pub action: AuditAction,
    pub entity_type: Option<String>,
    pub entity_id: Option<i64>,
    pub entity_name: Option<String>,
    pub details: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = String))]
    pub created_at: DateTime<Utc>,
}

impl From<audit_log::Model> for AuditLogEntry {
    fn from(model: audit_log::Model) -> Self {
        Self {
            id: model.id,
            user_id: model.user_id,
            action: model.action,
            entity_type: model.entity_type,
            entity_id: model.entity_id,
            entity_name: model.entity_name,
            details: model.details,
            ip_address: model.ip_address,
            user_agent: model.user_agent,
            created_at: model.created_at,
        }
    }
}

#[derive(Serialize)]
pub struct ConfigUpdateDetails<'a> {
    pub value: &'a str,
}

#[derive(Serialize)]
pub struct ConfigActionDetails<'a> {
    pub action: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_email: Option<&'a str>,
}

#[derive(Serialize)]
pub struct AdminCreateUserDetails<'a> {
    pub email: &'a str,
    pub email_verified: bool,
    pub role: UserRole,
    pub status: UserStatus,
    pub storage_quota: i64,
    pub policy_group_id: Option<i64>,
}

#[derive(Serialize)]
pub struct AdminUpdateUserDetails {
    pub email_verified: bool,
    pub role: UserRole,
    pub status: UserStatus,
    pub storage_quota: i64,
    pub policy_group_id: Option<i64>,
}

#[derive(Serialize)]
pub struct AdminForceDeleteUserDetails {
    pub file_count: usize,
    pub folder_count: usize,
    pub share_count: u64,
    pub webdav_account_count: u64,
    pub upload_session_count: u64,
    pub lock_count: u64,
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
    #[serde(skip_serializing_if = "str::is_empty")]
    pub description: &'a str,
    pub member_count: u64,
    pub storage_quota: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy_group_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archived_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor_role: Option<TeamMemberRole>,
}

#[derive(Serialize)]
pub struct TeamCleanupAuditDetails {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archived_at: Option<DateTime<Utc>>,
    pub retention_days: i64,
}

#[derive(Serialize)]
pub struct TeamMemberAddAuditDetails<'a> {
    pub member_user_id: i64,
    pub member_username: &'a str,
    pub role: TeamMemberRole,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor_role: Option<TeamMemberRole>,
}

#[derive(Serialize)]
pub struct TeamMemberUpdateAuditDetails<'a> {
    pub member_user_id: i64,
    pub member_username: &'a str,
    pub previous_role: TeamMemberRole,
    pub next_role: TeamMemberRole,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor_role: Option<TeamMemberRole>,
}

#[derive(Serialize)]
pub struct TeamMemberRemoveAuditDetails<'a> {
    pub member_user_id: i64,
    pub member_username: &'a str,
    pub removed_role: TeamMemberRole,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor_role: Option<TeamMemberRole>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(utoipa::ToSchema))]
pub struct TeamAuditEntryInfo {
    pub id: i64,
    pub action: AuditAction,
    pub actor_username: String,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = String))]
    pub created_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub member_username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<TeamMemberRole>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_role: Option<TeamMemberRole>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_role: Option<TeamMemberRole>,
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
        AuditRequestInfo::from_request(req).to_context(claims.user_id)
    }
}

impl AuditRequestInfo {
    pub fn from_request(req: &HttpRequest) -> Self {
        Self {
            ip_address: req
                .connection_info()
                .realip_remote_addr()
                .map(|s| bounded_audit_value(s, MAX_AUDIT_IP_ADDRESS_LEN)),
            user_agent: req
                .headers()
                .get("user-agent")
                .and_then(|v| v.to_str().ok())
                .map(|s| bounded_audit_value(s, MAX_AUDIT_USER_AGENT_LEN)),
        }
    }

    pub fn to_context(&self, user_id: i64) -> AuditContext {
        AuditContext {
            user_id,
            ip_address: self.ip_address.clone(),
            user_agent: self.user_agent.clone(),
        }
    }
}

fn bounded_audit_value(value: &str, max_len: usize) -> String {
    if value.len() <= max_len {
        return value.to_string();
    }

    let mut end = max_len;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].to_string()
}

/// Fire-and-forget 审计日志。DB 错误只 warn 不传播。
pub async fn log(
    state: &PrimaryAppState,
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
        action: Set(action),
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

async fn query_models(
    state: &PrimaryAppState,
    filters: AuditLogFilters,
    limit: u64,
    offset: u64,
) -> Result<OffsetPage<audit_log::Model>> {
    load_offset_page(limit, offset, 200, |limit, offset| async move {
        audit_log_repo::find_with_filters(
            &state.db,
            audit_log_repo::AuditLogQuery {
                user_id: filters.user_id,
                action: filters.action.as_deref(),
                entity_type: filters.entity_type.as_deref(),
                entity_id: filters.entity_id,
                after: filters.after,
                before: filters.before,
                limit,
                offset,
            },
        )
        .await
    })
    .await
}

pub async fn query(
    state: &PrimaryAppState,
    filters: AuditLogFilters,
    limit: u64,
    offset: u64,
) -> Result<OffsetPage<AuditLogEntry>> {
    let page = query_models(state, filters, limit, offset).await?;
    let items = page.items.into_iter().map(Into::into).collect();
    Ok(OffsetPage::new(items, page.total, page.limit, page.offset))
}

fn parse_team_member_role(value: Option<&serde_json::Value>) -> Option<TeamMemberRole> {
    serde_json::from_value(value?.clone()).ok()
}

fn parse_string_field(details: &serde_json::Value, key: &str) -> Option<String> {
    details.get(key)?.as_str().map(ToOwned::to_owned)
}

fn build_team_audit_entry(
    entry: audit_log::Model,
    usernames: &HashMap<i64, String>,
) -> TeamAuditEntryInfo {
    let actor_username = usernames
        .get(&entry.user_id)
        .cloned()
        .unwrap_or_else(|| format!("#{}", entry.user_id));
    let parsed_details = entry
        .details
        .as_deref()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok());

    let member_username = parsed_details
        .as_ref()
        .and_then(|details| parse_string_field(details, "member_username"));
    let role = parsed_details
        .as_ref()
        .and_then(|details| parse_team_member_role(details.get("role")))
        .or_else(|| {
            parsed_details
                .as_ref()
                .and_then(|details| parse_team_member_role(details.get("removed_role")))
        });
    let previous_role = parsed_details
        .as_ref()
        .and_then(|details| parse_team_member_role(details.get("previous_role")));
    let next_role = parsed_details
        .as_ref()
        .and_then(|details| parse_team_member_role(details.get("next_role")));

    TeamAuditEntryInfo {
        id: entry.id,
        action: entry.action,
        actor_username,
        created_at: entry.created_at,
        member_username,
        role,
        previous_role,
        next_role,
    }
}

pub async fn query_team_entries(
    state: &PrimaryAppState,
    filters: AuditLogFilters,
    limit: u64,
    offset: u64,
) -> Result<OffsetPage<TeamAuditEntryInfo>> {
    let page = query_models(state, filters, limit, offset).await?;
    let user_ids: Vec<i64> = page
        .items
        .iter()
        .map(|entry| entry.user_id)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let usernames = user_repo::find_by_ids(&state.db, &user_ids)
        .await?
        .into_iter()
        .map(|user| (user.id, user.username))
        .collect::<HashMap<_, _>>();
    let items = page
        .items
        .into_iter()
        .map(|entry| build_team_audit_entry(entry, &usernames))
        .collect();

    Ok(OffsetPage::new(items, page.total, page.limit, page.offset))
}

/// 清理过期审计日志
pub async fn cleanup_expired(state: &PrimaryAppState) -> Result<u64> {
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
    use actix_web::test as actix_test;

    use super::{
        AuditAction, AuditRequestInfo, MAX_AUDIT_IP_ADDRESS_LEN, MAX_AUDIT_USER_AGENT_LEN,
        bounded_audit_value,
    };

    #[test]
    fn bounded_audit_value_truncates_without_splitting_utf8() {
        assert_eq!(bounded_audit_value("abcdef", 3), "abc");
        assert_eq!(bounded_audit_value("猫猫猫", 4), "猫");
    }

    #[test]
    fn request_audit_info_truncates_user_controlled_headers() {
        let long_ip = "1".repeat(MAX_AUDIT_IP_ADDRESS_LEN + 32);
        let long_user_agent = "a".repeat(MAX_AUDIT_USER_AGENT_LEN + 32);
        let req = actix_test::TestRequest::default()
            .peer_addr("127.0.0.1:12345".parse().unwrap())
            .insert_header(("X-Forwarded-For", long_ip.as_str()))
            .insert_header(("User-Agent", long_user_agent.as_str()))
            .to_http_request();

        let info = AuditRequestInfo::from_request(&req);

        assert_eq!(
            info.ip_address.as_deref(),
            Some(&long_ip[..MAX_AUDIT_IP_ADDRESS_LEN])
        );
        assert_eq!(
            info.user_agent.as_deref(),
            Some(&long_user_agent[..MAX_AUDIT_USER_AGENT_LEN])
        );
    }

    #[test]
    fn audit_action_strings_match_existing_contract() {
        let cases = [
            (AuditAction::AdminCreateUser, "admin_create_user"),
            (AuditAction::AdminForceDeleteUser, "admin_force_delete_user"),
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
            (AuditAction::ConfigActionExecute, "config_action_execute"),
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
            (AuditAction::TeamMemberAdd, "team_member_add"),
            (AuditAction::TeamMemberRemove, "team_member_remove"),
            (AuditAction::TeamMemberUpdate, "team_member_update"),
            (AuditAction::TeamRestore, "team_restore"),
            (AuditAction::TeamUpdate, "team_update"),
            (AuditAction::UserChangePassword, "user_change_password"),
            (
                AuditAction::UserConfirmPasswordReset,
                "user_confirm_password_reset",
            ),
            (
                AuditAction::UserConfirmEmailChange,
                "user_confirm_email_change",
            ),
            (
                AuditAction::UserConfirmRegistration,
                "user_confirm_registration",
            ),
            (AuditAction::UserLogin, "user_login"),
            (AuditAction::UserLogout, "user_logout"),
            (
                AuditAction::UserRefreshTokenReuseDetected,
                "user_refresh_token_reuse_detected",
            ),
            (
                AuditAction::UserRequestEmailChange,
                "user_request_email_change",
            ),
            (
                AuditAction::UserRequestPasswordReset,
                "user_request_password_reset",
            ),
            (AuditAction::UserRegister, "user_register"),
            (
                AuditAction::UserResendEmailChange,
                "user_resend_email_change",
            ),
            (
                AuditAction::UserResendRegistration,
                "user_resend_registration",
            ),
        ];

        for (action, expected) in cases {
            assert_eq!(action.as_str(), expected);
            assert_eq!(action.as_ref(), expected);
            assert_eq!(action.to_string(), expected);
        }
    }
}
