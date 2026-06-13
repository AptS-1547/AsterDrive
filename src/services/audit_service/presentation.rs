use std::collections::BTreeMap;

use serde_json::Value;

use crate::types::{AuditAction, AuditEntityType};

use super::models::{AuditPresentation, AuditPresentationMessage};

pub fn build_audit_presentation(
    action: AuditAction,
    entity_type: AuditEntityType,
    entity_id: Option<i64>,
    entity_name: Option<&str>,
    details: Option<&str>,
) -> Option<AuditPresentation> {
    // Presentation is additive API metadata. Malformed legacy details must degrade to
    // summary/target fallback instead of making the audit entry unreadable.
    let parsed_details = details.and_then(parse_details);
    let summary = Some(summary_message(
        action,
        entity_name,
        parsed_details.as_ref(),
    ));
    let target = match action {
        AuditAction::ServerStart | AuditAction::ServerShutdown => Some(server_target()),
        _ => target_message(entity_type, entity_id, entity_name),
    };
    let detail = detail_message(action, parsed_details.as_ref());

    Some(AuditPresentation {
        summary,
        target,
        detail,
    })
}

fn parse_details(raw: &str) -> Option<Value> {
    serde_json::from_str(raw).ok()
}

fn summary_message(
    action: AuditAction,
    entity_name: Option<&str>,
    details: Option<&Value>,
) -> AuditPresentationMessage {
    let mut params = BTreeMap::new();
    if let Some(name) = entity_name {
        params.insert("name".to_string(), Value::String(name.to_string()));
    }

    match action {
        AuditAction::ConfigUpdate | AuditAction::AdminDeleteConfig => {
            if let Some(name) = entity_name {
                params.insert("key".to_string(), Value::String(name.to_string()));
            }
        }
        AuditAction::TeamMemberAdd
        | AuditAction::TeamMemberRemove
        | AuditAction::TeamMemberUpdate => {
            copy_string_param(details, &mut params, "member_username");
        }
        _ => {}
    }

    AuditPresentationMessage {
        code: action.as_str().to_string(),
        params,
    }
}

fn target_message(
    entity_type: AuditEntityType,
    entity_id: Option<i64>,
    entity_name: Option<&str>,
) -> Option<AuditPresentationMessage> {
    if entity_id.is_none() && entity_name.is_none() {
        return None;
    }

    let mut params = BTreeMap::new();
    if let Some(id) = entity_id {
        params.insert("id".to_string(), Value::Number(id.into()));
    }
    if let Some(name) = entity_name {
        params.insert("name".to_string(), Value::String(name.to_string()));
    }

    Some(AuditPresentationMessage {
        code: entity_type.as_str().to_string(),
        params,
    })
}

fn server_target() -> AuditPresentationMessage {
    AuditPresentationMessage {
        code: "server".to_string(),
        params: BTreeMap::new(),
    }
}

fn detail_message(
    action: AuditAction,
    details: Option<&Value>,
) -> Option<AuditPresentationMessage> {
    let details = details?;
    let mut params = BTreeMap::new();

    match action {
        AuditAction::ConfigUpdate => {
            copy_param(details, &mut params, "value");
            Some(message("config_value_updated", params))
        }
        AuditAction::ConfigActionExecute => {
            copy_param(details, &mut params, "action");
            copy_param(details, &mut params, "target_email");
            Some(message("config_action_executed", params))
        }
        AuditAction::MailSend => {
            copy_param(details, &mut params, "to_address");
            copy_param(details, &mut params, "template_code");
            copy_param(details, &mut params, "outbox_id");
            Some(message("mail_sent", params))
        }
        AuditAction::MailDeliveryFailed => {
            copy_param(details, &mut params, "to_address");
            copy_param(details, &mut params, "template_code");
            copy_param(details, &mut params, "outbox_id");
            copy_param(details, &mut params, "attempt_count");
            copy_param(details, &mut params, "error");
            Some(message("mail_delivery_failed", params))
        }
        AuditAction::AdminCreateUser => {
            copy_params(
                details,
                &mut params,
                &[
                    "email",
                    "email_verified",
                    "role",
                    "status",
                    "must_change_password",
                    "temporary_password_generated",
                    "storage_quota",
                    "policy_group_id",
                ],
            );
            Some(message("admin_user_created_snapshot", params))
        }
        AuditAction::AdminUpdateUser => {
            copy_params(
                details,
                &mut params,
                &[
                    "email_verified",
                    "role",
                    "status",
                    "must_change_password",
                    "storage_quota",
                    "policy_group_id",
                ],
            );
            Some(message("admin_user_updated_snapshot", params))
        }
        AuditAction::AdminForceDeleteUser => {
            copy_params(
                details,
                &mut params,
                &[
                    "file_count",
                    "folder_count",
                    "share_count",
                    "webdav_account_count",
                    "upload_session_count",
                    "lock_count",
                ],
            );
            Some(message("admin_force_delete_user_finished", params))
        }
        AuditAction::AdminCreatePolicy
        | AuditAction::AdminUpdatePolicy
        | AuditAction::AdminDeletePolicy => {
            copy_params(
                details,
                &mut params,
                &[
                    "driver_type",
                    "remote_node_id",
                    "max_file_size",
                    "chunk_size",
                    "is_default",
                ],
            );
            Some(message("storage_policy_snapshot", params))
        }
        AuditAction::AdminCreatePolicyGroup
        | AuditAction::AdminUpdatePolicyGroup
        | AuditAction::AdminDeletePolicyGroup => {
            copy_params(
                details,
                &mut params,
                &["is_default", "is_enabled", "item_count"],
            );
            Some(message("policy_group_snapshot", params))
        }
        AuditAction::AdminMigratePolicyGroupUsers => {
            copy_params(
                details,
                &mut params,
                &[
                    "source_group_id",
                    "source_group_name",
                    "target_group_id",
                    "target_group_name",
                    "affected_users",
                    "affected_teams",
                    "migrated_assignments",
                ],
            );
            Some(message("policy_group_migration_finished", params))
        }
        AuditAction::AdminCreateTeam
        | AuditAction::AdminUpdateTeam
        | AuditAction::AdminArchiveTeam
        | AuditAction::AdminRestoreTeam
        | AuditAction::TeamCreate
        | AuditAction::TeamUpdate
        | AuditAction::TeamArchive
        | AuditAction::TeamRestore => {
            copy_params(
                details,
                &mut params,
                &[
                    "description",
                    "member_count",
                    "storage_quota",
                    "policy_group_id",
                    "archived_at",
                    "actor_role",
                ],
            );
            Some(message("team_snapshot", params))
        }
        AuditAction::TeamCleanupExpired => {
            copy_params(details, &mut params, &["archived_at", "retention_days"]);
            Some(message("team_cleanup_expired_finished", params))
        }
        AuditAction::TeamMemberAdd => {
            copy_param(details, &mut params, "member_user_id");
            copy_param(details, &mut params, "member_username");
            copy_param(details, &mut params, "role");
            copy_param(details, &mut params, "actor_role");
            Some(message("team_member_added", params))
        }
        AuditAction::TeamMemberUpdate => {
            copy_param(details, &mut params, "member_user_id");
            copy_param(details, &mut params, "member_username");
            copy_param(details, &mut params, "previous_role");
            copy_param(details, &mut params, "next_role");
            copy_param(details, &mut params, "actor_role");
            Some(message("team_member_updated", params))
        }
        AuditAction::TeamMemberRemove => {
            copy_param(details, &mut params, "member_user_id");
            copy_param(details, &mut params, "member_username");
            copy_param(details, &mut params, "removed_role");
            copy_param(details, &mut params, "actor_role");
            Some(message("team_member_removed", params))
        }
        AuditAction::UserRevokeSession => {
            copy_params(details, &mut params, &["session_id", "revoked_current"]);
            Some(message("auth_session_revoked", params))
        }
        AuditAction::UserRevokeOtherSessions => {
            copy_params(
                details,
                &mut params,
                &["session_id", "removed", "revoked_current"],
            );
            Some(message("other_auth_sessions_revoked", params))
        }
        AuditAction::UserUpdateProfile => {
            copy_param(details, &mut params, "display_name");
            Some(message("user_profile_updated", params))
        }
        AuditAction::UserSetAvatarSource => {
            copy_param(details, &mut params, "source");
            Some(message("user_avatar_source_changed", params))
        }
        AuditAction::AdminCreateRemoteNode
        | AuditAction::AdminUpdateRemoteNode
        | AuditAction::AdminDeleteRemoteNode
        | AuditAction::AdminTestRemoteNode => {
            copy_params(
                details,
                &mut params,
                &["base_url", "is_enabled", "enrollment_status"],
            );
            Some(message("remote_node_snapshot", params))
        }
        AuditAction::AdminCreateRemoteNodeEnrollmentToken => {
            copy_param(details, &mut params, "expires_at");
            Some(message("remote_node_enrollment_token_created", params))
        }
        AuditAction::AdminCreateRemoteIngressProfile
        | AuditAction::AdminUpdateRemoteIngressProfile
        | AuditAction::AdminDeleteRemoteIngressProfile => {
            copy_params(
                details,
                &mut params,
                &["profile_key", "driver_type", "is_default"],
            );
            Some(message("remote_ingress_profile_snapshot", params))
        }
        AuditAction::AdminCreateExternalAuthProvider
        | AuditAction::AdminUpdateExternalAuthProvider
        | AuditAction::AdminDeleteExternalAuthProvider => {
            copy_params(
                details,
                &mut params,
                &[
                    "key",
                    "issuer_url",
                    "enabled",
                    "auto_provision_enabled",
                    "auto_link_verified_email_enabled",
                    "require_email_verified",
                ],
            );
            Some(message("external_auth_provider_snapshot", params))
        }
        AuditAction::AdminTestExternalAuthProvider => {
            copy_params(
                details,
                &mut params,
                &["provider_kind", "key", "success", "issuer_url", "enabled"],
            );
            Some(message("external_auth_provider_tested", params))
        }
        AuditAction::UserExternalAuthLogin => {
            copy_params(
                details,
                &mut params,
                &[
                    "provider_key",
                    "issuer",
                    "subject",
                    "linked",
                    "auto_provisioned",
                ],
            );
            Some(message("external_auth_login_completed", params))
        }
        AuditAction::WebdavAccountToggle => {
            copy_param(details, &mut params, "is_active");
            Some(message("webdav_account_status_changed", params))
        }
        AuditAction::TeamWebdavAccountToggle => {
            copy_params(details, &mut params, &["team_id", "is_active"]);
            Some(message("team_webdav_account_status_changed", params))
        }
        AuditAction::TeamWebdavAccountCreate | AuditAction::TeamWebdavAccountDelete => {
            copy_param(details, &mut params, "team_id");
            Some(message("team_webdav_account_changed", params))
        }
        AuditAction::AdminForceUnlock => {
            copy_params(details, &mut params, &["entity_type", "entity_id"]);
            Some(message("resource_force_unlocked", params))
        }
        AuditAction::AdminCreateBlobMaintenanceTask => {
            copy_param(details, &mut params, "action");
            if let Some(count) = array_len_value(details, "blob_ids") {
                params.insert("blob_count".to_string(), count);
            }
            Some(message("blob_maintenance_task_created", params))
        }
        AuditAction::AdminCleanupExpiredLocks => {
            copy_param(details, &mut params, "removed");
            Some(message("locks_cleanup_finished", params))
        }
        AuditAction::AdminCleanupTasks => {
            copy_param(details, &mut params, "removed");
            copy_param(details, &mut params, "finished_before");
            copy_param(details, &mut params, "kind");
            copy_param(details, &mut params, "status");
            Some(message("tasks_cleanup_finished", params))
        }
        AuditAction::TaskRetry => {
            copy_param(details, &mut params, "kind");
            copy_param(details, &mut params, "previous_attempt_count");
            Some(message("task_retry_scheduled", params))
        }
        AuditAction::FileUploadCancel => {
            copy_param(details, &mut params, "upload_id");
            Some(message("upload_cancelled", params))
        }
        AuditAction::FileDirectLinkCreate | AuditAction::FilePreviewLinkCreate => {
            copy_param(details, &mut params, "source");
            copy_param(details, &mut params, "app_key");
            Some(message("file_access_token_created", params))
        }
        AuditAction::FileVersionRestore | AuditAction::FileVersionDelete => {
            copy_param(details, &mut params, "version_id");
            Some(message("file_version_changed", params))
        }
        AuditAction::FolderPolicyChange => {
            copy_param(details, &mut params, "previous_policy_id");
            copy_param(details, &mut params, "policy_id");
            Some(message("folder_policy_changed", params))
        }
        AuditAction::BatchDelete => {
            copy_param(details, &mut params, "succeeded");
            copy_param(details, &mut params, "failed");
            Some(message("batch_delete_finished", params))
        }
        AuditAction::BatchCopy | AuditAction::BatchMove => {
            copy_param(details, &mut params, "target_folder_id");
            copy_param(details, &mut params, "succeeded");
            copy_param(details, &mut params, "failed");
            Some(message("batch_transfer_finished", params))
        }
        AuditAction::ShareBatchDelete => {
            copy_param(details, &mut params, "succeeded");
            copy_param(details, &mut params, "failed");
            Some(message("share_batch_delete_finished", params))
        }
        AuditAction::ShareUpdate => {
            copy_param(details, &mut params, "has_password");
            copy_param(details, &mut params, "expires_at");
            copy_param(details, &mut params, "max_downloads");
            Some(message("share_updated", params))
        }
        AuditAction::PropertySet | AuditAction::PropertyDelete => {
            copy_param(details, &mut params, "entity_type");
            copy_param(details, &mut params, "namespace");
            copy_param(details, &mut params, "name");
            Some(message("property_changed", params))
        }
        AuditAction::TrashPurgeAll => {
            copy_param(details, &mut params, "purged");
            Some(message("trash_purge_finished", params))
        }
        AuditAction::ArchiveCompress
        | AuditAction::ArchiveExtract
        | AuditAction::ArchiveDownload => {
            copy_param(details, &mut params, "archive_name");
            copy_param(details, &mut params, "target_folder_id");
            Some(message("archive_selection_created", params))
        }
        AuditAction::OfflineDownload => {
            copy_param(details, &mut params, "source");
            copy_param(details, &mut params, "target_folder_id");
            Some(message("offline_download_created", params))
        }
        _ => None,
    }
}

fn message(code: &str, params: BTreeMap<String, Value>) -> AuditPresentationMessage {
    AuditPresentationMessage {
        code: code.to_string(),
        params,
    }
}

fn copy_string_param(source: Option<&Value>, params: &mut BTreeMap<String, Value>, key: &str) {
    let Some(value) = source
        .and_then(|source| source.get(key))
        .and_then(Value::as_str)
    else {
        return;
    };
    params.insert(key.to_string(), Value::String(value.to_string()));
}

fn copy_param(source: &Value, params: &mut BTreeMap<String, Value>, key: &str) {
    let Some(value) = source.get(key) else {
        return;
    };
    if value.is_null() {
        return;
    }
    params.insert(key.to_string(), value.clone());
}

fn copy_params(source: &Value, params: &mut BTreeMap<String, Value>, keys: &[&str]) {
    for key in keys {
        copy_param(source, params, key);
    }
}

fn array_len_value(source: &Value, key: &str) -> Option<Value> {
    let len = source.get(key)?.as_array()?.len();
    Some(Value::Number(u64::try_from(len).ok()?.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presentation_includes_config_key_and_value_detail() {
        let presentation = build_audit_presentation(
            AuditAction::ConfigUpdate,
            AuditEntityType::SystemConfig,
            Some(42),
            Some("audit_log_recorded_actions"),
            Some(r#"{"value":"[\"user_login\"]"}"#),
        )
        .expect("presentation should be built");

        assert_eq!(presentation.summary.as_ref().unwrap().code, "config_update");
        assert_eq!(
            presentation.summary.as_ref().unwrap().params.get("key"),
            Some(&Value::String("audit_log_recorded_actions".to_string()))
        );
        assert_eq!(
            presentation.detail.as_ref().unwrap().code,
            "config_value_updated"
        );
    }

    #[test]
    fn presentation_handles_malformed_details_with_safe_fallback_fields() {
        let presentation = build_audit_presentation(
            AuditAction::FileDownload,
            AuditEntityType::File,
            Some(7),
            Some("report.txt"),
            Some("not json"),
        )
        .expect("presentation should be built");

        assert_eq!(presentation.summary.as_ref().unwrap().code, "file_download");
        assert!(presentation.detail.is_none());
        assert_eq!(presentation.target.as_ref().unwrap().code, "file");
    }

    #[test]
    fn presentation_includes_folder_policy_change_detail() {
        let presentation = build_audit_presentation(
            AuditAction::FolderPolicyChange,
            AuditEntityType::Folder,
            Some(7),
            Some("Projects"),
            Some(r#"{"previous_policy_id":2,"policy_id":5}"#),
        )
        .expect("presentation should be built");

        let detail = presentation.detail.as_ref().unwrap();
        assert_eq!(detail.code, "folder_policy_changed");
        assert_eq!(detail.params.get("previous_policy_id"), Some(&2.into()));
        assert_eq!(detail.params.get("policy_id"), Some(&5.into()));
    }

    #[test]
    fn presentation_includes_admin_user_snapshot_detail() {
        let presentation = build_audit_presentation(
            AuditAction::AdminCreateUser,
            AuditEntityType::User,
            Some(7),
            Some("alice"),
            Some(
                r#"{"email":"alice@example.com","email_verified":true,"role":"admin","status":"active","must_change_password":false,"temporary_password_generated":true,"storage_quota":1073741824,"policy_group_id":3}"#,
            ),
        )
        .expect("presentation should be built");

        let detail = presentation.detail.as_ref().unwrap();
        assert_eq!(detail.code, "admin_user_created_snapshot");
        assert_eq!(
            detail.params.get("email"),
            Some(&Value::String("alice@example.com".to_string()))
        );
        assert_eq!(detail.params.get("policy_group_id"), Some(&3.into()));
    }

    #[test]
    fn presentation_includes_policy_group_migration_detail() {
        let presentation = build_audit_presentation(
            AuditAction::AdminMigratePolicyGroupUsers,
            AuditEntityType::PolicyGroup,
            Some(1),
            Some("Default"),
            Some(
                r#"{"source_group_id":1,"source_group_name":"Default","target_group_id":2,"target_group_name":"Archive","affected_users":4,"affected_teams":2,"migrated_assignments":6}"#,
            ),
        )
        .expect("presentation should be built");

        let detail = presentation.detail.as_ref().unwrap();
        assert_eq!(detail.code, "policy_group_migration_finished");
        assert_eq!(
            detail.params.get("target_group_name"),
            Some(&Value::String("Archive".to_string()))
        );
        assert_eq!(detail.params.get("migrated_assignments"), Some(&6.into()));
    }

    #[test]
    fn presentation_includes_external_auth_login_detail() {
        let presentation = build_audit_presentation(
            AuditAction::UserExternalAuthLogin,
            AuditEntityType::ExternalAuthIdentity,
            Some(9),
            Some("oidc"),
            Some(
                r#"{"provider_key":"oidc","issuer":"https://idp.example.com","subject":"sub-1","linked":true,"auto_provisioned":false}"#,
            ),
        )
        .expect("presentation should be built");

        let detail = presentation.detail.as_ref().unwrap();
        assert_eq!(detail.code, "external_auth_login_completed");
        assert_eq!(
            detail.params.get("provider_key"),
            Some(&Value::String("oidc".to_string()))
        );
        assert_eq!(detail.params.get("linked"), Some(&Value::Bool(true)));
    }

    #[test]
    fn presentation_counts_blob_ids_for_blob_maintenance_detail() {
        let presentation = build_audit_presentation(
            AuditAction::AdminCreateBlobMaintenanceTask,
            AuditEntityType::Task,
            Some(10),
            Some("blob maintenance"),
            Some(r#"{"action":"verify","blob_ids":[1,2,3]}"#),
        )
        .expect("presentation should be built");

        let detail = presentation.detail.as_ref().unwrap();
        assert_eq!(detail.code, "blob_maintenance_task_created");
        assert_eq!(
            detail.params.get("action"),
            Some(&Value::String("verify".to_string()))
        );
        assert_eq!(detail.params.get("blob_count"), Some(&3.into()));
    }

    #[test]
    fn presentation_uses_server_target_for_server_lifecycle_actions() {
        let presentation = build_audit_presentation(
            AuditAction::ServerStart,
            AuditEntityType::SystemConfig,
            None,
            None,
            None,
        )
        .expect("presentation should be built");

        assert_eq!(presentation.summary.as_ref().unwrap().code, "server_start");
        assert_eq!(presentation.target.as_ref().unwrap().code, "server");
        assert!(presentation.detail.is_none());
    }
}
