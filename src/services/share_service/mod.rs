mod access;
mod content;
mod management;
mod models;
mod shared;

use crate::errors::Result;
use crate::runtime::AppState;
use crate::services::audit_service::{self, AuditContext};
use crate::services::batch_service;
use crate::services::workspace_storage_service::WorkspaceStorageScope;

pub use access::{
    PasswordVerified, check_share_password_cookie, get_share_avatar_bytes, get_share_info,
    sign_share_cookie, verify_password, verify_password_and_sign, verify_share_cookie,
};
pub use content::{
    download_shared_file, download_shared_folder_file, get_shared_folder_file_thumbnail,
    get_shared_thumbnail, list_shared_folder, list_shared_subfolder,
};
pub use management::{
    admin_delete_share, batch_delete_shares, batch_delete_team_shares, create_share, delete_share,
    delete_team_share, list_my_shares_paginated, list_paginated, list_team_shares_paginated,
    update_share, update_team_share, validate_batch_share_ids,
};
pub use models::{
    MyShareInfo, ShareInfo, SharePublicInfo, SharePublicOwnerInfo, ShareStatus, ShareTarget,
};

pub(crate) use content::{load_preview_shared_file, load_preview_shared_folder_file};
pub(crate) use management::{
    batch_delete_shares_in_scope, create_share_in_scope, delete_share_in_scope,
    list_shares_paginated_in_scope, update_share_in_scope,
};

pub(crate) async fn create_share_in_scope_with_audit(
    state: &AppState,
    scope: WorkspaceStorageScope,
    target: ShareTarget,
    password: Option<String>,
    expires_at: Option<chrono::DateTime<chrono::Utc>>,
    max_downloads: i64,
    audit_ctx: &AuditContext,
) -> Result<ShareInfo> {
    let share =
        create_share_in_scope(state, scope, target, password, expires_at, max_downloads).await?;
    audit_service::log(
        state,
        audit_ctx,
        audit_service::AuditAction::ShareCreate,
        None,
        Some(share.id),
        None,
        None,
    )
    .await;
    Ok(share)
}

pub(crate) async fn update_share_in_scope_with_audit(
    state: &AppState,
    scope: WorkspaceStorageScope,
    share_id: i64,
    password: Option<String>,
    expires_at: Option<chrono::DateTime<chrono::Utc>>,
    max_downloads: i64,
    audit_ctx: &AuditContext,
) -> Result<ShareInfo> {
    let outcome =
        update_share_in_scope(state, scope, share_id, password, expires_at, max_downloads).await?;
    let share = outcome.share;
    audit_service::log(
        state,
        audit_ctx,
        audit_service::AuditAction::ShareUpdate,
        Some("share"),
        Some(share.id),
        Some(&share.token),
        audit_service::details(audit_service::ShareUpdateDetails {
            has_password: outcome.has_password,
            expires_at: share.expires_at,
            max_downloads: share.max_downloads,
        }),
    )
    .await;
    Ok(share)
}

pub(crate) async fn delete_share_in_scope_with_audit(
    state: &AppState,
    scope: WorkspaceStorageScope,
    share_id: i64,
    audit_ctx: &AuditContext,
) -> Result<()> {
    delete_share_in_scope(state, scope, share_id).await?;
    audit_service::log(
        state,
        audit_ctx,
        audit_service::AuditAction::ShareDelete,
        None,
        Some(share_id),
        None,
        None,
    )
    .await;
    Ok(())
}

pub(crate) async fn batch_delete_shares_in_scope_with_audit(
    state: &AppState,
    scope: WorkspaceStorageScope,
    share_ids: &[i64],
    audit_ctx: &AuditContext,
) -> Result<batch_service::BatchResult> {
    validate_batch_share_ids(share_ids)?;
    let result = batch_delete_shares_in_scope(state, scope, share_ids).await?;
    audit_service::log(
        state,
        audit_ctx,
        audit_service::AuditAction::ShareBatchDelete,
        None,
        None,
        None,
        audit_service::details(audit_service::ShareBatchDeleteDetails {
            share_ids,
            succeeded: result.succeeded,
            failed: result.failed,
        }),
    )
    .await;
    Ok(result)
}
