//! 文件服务子模块：`content`。

use actix_web::web::Bytes;
use sha2::{Digest, Sha256};

use crate::errors::{AsterError, MapAsterErr, Result};
use crate::runtime::AppState;
use crate::services::{
    policy_service::StoragePolicy,
    workspace_models::FileInfo,
    workspace_storage_service::{self, WorkspaceStorageScope},
};

use super::get_info_in_scope;
use crate::utils::numbers::usize_to_i64;

/// 从临时文件存储 blob 并创建文件记录
///
/// 公共函数，REST upload 和 WebDAV flush 都调用。
/// - local 开启 `content_dedup` 时流式计算 sha256（不加载全文件到内存）
/// - 策略检查 + 配额检查 + 按策略决定是否做 blob 去重
/// - `put_file` 零拷贝（LocalDriver rename）
/// - 创建/覆盖文件记录
///
/// `existing_file_id`: Some 时覆盖现有文件，None 时新建
///
/// 返回创建/更新的文件记录。临时文件可能被 put_file rename 走，调用方不要依赖它存在。
/// `skip_lock_check`: WebDAV 持锁者写入时为 true（WebDAV handler 已验证 lock token）
#[allow(clippy::too_many_arguments)]
pub async fn store_from_temp(
    state: &AppState,
    user_id: i64,
    folder_id: Option<i64>,
    filename: &str,
    temp_path: &str,
    size: i64,
    existing_file_id: Option<i64>,
    skip_lock_check: bool,
) -> Result<FileInfo> {
    workspace_storage_service::store_from_temp(
        state,
        WorkspaceStorageScope::Personal { user_id },
        folder_id,
        filename,
        temp_path,
        size,
        existing_file_id,
        skip_lock_check,
    )
    .await
    .map(Into::into)
}

/// 上传文件（REST API，multipart）
pub async fn upload(
    state: &AppState,
    user_id: i64,
    payload: &mut actix_multipart::Multipart,
    folder_id: Option<i64>,
    relative_path: Option<&str>,
    declared_size: Option<i64>,
) -> Result<FileInfo> {
    workspace_storage_service::upload(
        state,
        WorkspaceStorageScope::Personal { user_id },
        payload,
        folder_id,
        relative_path,
        declared_size,
    )
    .await
    .map(Into::into)
}

pub(crate) async fn update_content_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    file_id: i64,
    body: Bytes,
    if_match: Option<&str>,
) -> Result<(crate::entities::file::Model, String)> {
    let db = &state.db;
    tracing::debug!(
        scope = ?scope,
        file_id,
        content_size = body.len(),
        has_if_match = if_match.is_some(),
        "updating file content"
    );
    let f = get_info_in_scope(state, scope, file_id).await?;

    if f.is_locked {
        let lock = crate::db::repository::lock_repo::find_by_entity(
            db,
            crate::types::EntityType::File,
            file_id,
        )
        .await?;
        if let Some(lock) = lock
            && lock.owner_id != Some(scope.actor_user_id())
        {
            return Err(AsterError::resource_locked(
                "file is locked by another user",
            ));
        }
    }

    let current_blob = crate::db::repository::file_repo::find_blob_by_id(db, f.blob_id).await?;
    if let Some(etag) = if_match {
        let expected = etag.trim_matches('"');
        if !expected.eq_ignore_ascii_case(&current_blob.hash) {
            return Err(AsterError::precondition_failed(
                "file has been modified (ETag mismatch)",
            ));
        }
    }

    let size = usize_to_i64(body.len(), "body length")?;
    let resolved_policy =
        workspace_storage_service::resolve_policy_for_size(state, scope, f.folder_id, size).await?;
    let result = if resolved_policy.driver_type == crate::types::DriverType::Local {
        let should_dedup = workspace_storage_service::local_content_dedup_enabled(&resolved_policy);
        let staging_token = format!("{}.upload", uuid::Uuid::new_v4());
        let staging_path =
            crate::storage::drivers::local::upload_staging_path(&resolved_policy, &staging_token);
        if let Some(parent) = staging_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_aster_err(AsterError::storage_driver_error)?;
        }
        tokio::fs::write(&staging_path, &body)
            .await
            .map_aster_err(AsterError::storage_driver_error)?;

        let precomputed_hash = should_dedup.then(|| {
            let mut hasher = Sha256::new();
            hasher.update(&body);
            crate::utils::hash::sha256_digest_to_hex(&hasher.finalize())
        });
        let staging_path = staging_path.to_string_lossy().into_owned();
        let result = workspace_storage_service::store_from_temp_with_hints(
            state,
            scope,
            f.folder_id,
            &f.name,
            &staging_path,
            size,
            Some(file_id),
            true,
            Some(resolved_policy),
            precomputed_hash.as_deref(),
        )
        .await;
        crate::utils::cleanup_temp_file(&staging_path).await;
        result
    } else {
        let temp_dir = &state.config.server.temp_dir;
        let runtime_temp_dir = crate::utils::paths::runtime_temp_dir(temp_dir);
        let temp_path = crate::utils::paths::runtime_temp_file_path(
            temp_dir,
            &uuid::Uuid::new_v4().to_string(),
        );
        tokio::fs::create_dir_all(&runtime_temp_dir)
            .await
            .map_aster_err(AsterError::storage_driver_error)?;
        tokio::fs::write(&temp_path, &body)
            .await
            .map_aster_err(AsterError::storage_driver_error)?;

        let result = workspace_storage_service::store_from_temp(
            state,
            scope,
            f.folder_id,
            &f.name,
            &temp_path,
            size,
            Some(file_id),
            true,
        )
        .await;
        crate::utils::cleanup_temp_file(&temp_path).await;
        result
    };

    let updated = result?;
    let new_blob = crate::db::repository::file_repo::find_blob_by_id(db, updated.blob_id).await?;
    tracing::debug!(
        scope = ?scope,
        file_id = updated.id,
        blob_id = updated.blob_id,
        size = updated.size,
        "updated file content"
    );
    Ok((updated, new_blob.hash.clone()))
}

/// 覆盖文件内容（REST API 编辑入口）
///
/// 支持 ETag 乐观锁（If-Match）+ 悲观锁检查（is_locked）。
/// 自动创建版本历史。返回 (更新后的 file, 新 blob hash)。
pub async fn update_content(
    state: &AppState,
    file_id: i64,
    user_id: i64,
    body: Bytes,
    if_match: Option<&str>,
) -> Result<(FileInfo, String)> {
    update_content_in_scope(
        state,
        WorkspaceStorageScope::Personal { user_id },
        file_id,
        body,
        if_match,
    )
    .await
    .map(|(file, hash)| (file.into(), hash))
}

/// 根据优先级链解析存储策略：文件夹覆盖 → 用户绑定策略组
pub async fn resolve_policy(
    state: &AppState,
    user_id: i64,
    folder_id: Option<i64>,
) -> Result<StoragePolicy> {
    resolve_policy_for_size(state, user_id, folder_id, 0).await
}

pub async fn resolve_policy_for_size(
    state: &AppState,
    user_id: i64,
    folder_id: Option<i64>,
    file_size: i64,
) -> Result<StoragePolicy> {
    workspace_storage_service::resolve_policy_for_size(
        state,
        WorkspaceStorageScope::Personal { user_id },
        folder_id,
        file_size,
    )
    .await
    .map(StoragePolicy::from)
}

/// 直接创建空文件（0 字节），不走 multipart upload 流程。
///
/// - 校验文件名
/// - 解析存储策略
/// - 只有 local 显式开启 `content_dedup` 时才复用空文件固定 sha256
/// - 其余路径都为每个文件分配独立 blob
/// - 创建文件记录并更新配额（0 字节不影响配额）
pub async fn create_empty(
    state: &AppState,
    user_id: i64,
    folder_id: Option<i64>,
    filename: &str,
) -> Result<FileInfo> {
    workspace_storage_service::create_empty(
        state,
        WorkspaceStorageScope::Personal { user_id },
        folder_id,
        filename,
    )
    .await
    .map(Into::into)
}
