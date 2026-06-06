//! 文件服务子模块：`lock`。

use crate::entities::file;
use crate::errors::Result;
use crate::runtime::SharedRuntimeState;
use crate::services::{
    lock_service, workspace_models::FileInfo, workspace_storage_service::WorkspaceStorageScope,
};
use crate::types::EntityType;

pub(crate) async fn set_lock_in_scope(
    state: &impl SharedRuntimeState,
    scope: WorkspaceStorageScope,
    file_id: i64,
    locked: bool,
) -> Result<file::Model> {
    tracing::debug!(
        scope = ?scope,
        file_id,
        locked,
        "setting file lock state"
    );
    crate::services::workspace_storage_service::verify_file_access(state, scope, file_id).await?;

    if locked {
        lock_service::lock(
            state,
            EntityType::File,
            file_id,
            Some(scope.actor_user_id()),
            None,
            None,
        )
        .await?;
    } else {
        lock_service::unlock(state, EntityType::File, file_id, scope.actor_user_id()).await?;
    }

    let file =
        crate::services::workspace_storage_service::verify_file_access(state, scope, file_id)
            .await?;
    tracing::debug!(
        scope = ?scope,
        file_id = file.id,
        locked = file.is_locked,
        "updated file lock state"
    );
    Ok(file)
}

/// 设置/解除文件锁，返回更新后的文件信息
pub async fn set_lock(
    state: &impl SharedRuntimeState,
    file_id: i64,
    user_id: i64,
    locked: bool,
) -> Result<FileInfo> {
    set_lock_in_scope(
        state,
        WorkspaceStorageScope::Personal { user_id },
        file_id,
        locked,
    )
    .await
    .map(Into::into)
}
