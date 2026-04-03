use actix_web::HttpResponse;
use chrono::Utc;
use futures::{StreamExt, stream};
use sea_orm::{ActiveModelTrait, Set, TransactionTrait};

use crate::db::repository::file_repo;
use crate::entities::{file, file_blob};
use crate::errors::{AsterError, MapAsterErr, Result};
use crate::runtime::AppState;
use crate::services::{
    thumbnail_service,
    workspace_storage_service::{self, WorkspaceStorageScope},
};
use crate::types::NullablePatch;

const BLOB_CLEANUP_CONCURRENCY: usize = 8;

pub(crate) fn ensure_personal_file_scope(file: &file::Model) -> Result<()> {
    workspace_storage_service::ensure_personal_file_scope(file)
}

pub(crate) async fn get_info_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    id: i64,
) -> Result<file::Model> {
    workspace_storage_service::verify_file_access(state, scope, id).await
}

pub(crate) async fn download_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    id: i64,
    if_none_match: Option<&str>,
) -> Result<HttpResponse> {
    let file = get_info_in_scope(state, scope, id).await?;
    let blob = file_repo::find_blob_by_id(&state.db, file.blob_id).await?;
    build_stream_response(state, &file, &blob, if_none_match).await
}

pub(crate) async fn delete_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    id: i64,
) -> Result<()> {
    let file = get_info_in_scope(state, scope, id).await?;
    if file.is_locked {
        return Err(AsterError::resource_locked("file is locked"));
    }
    file_repo::soft_delete(&state.db, id).await
}

pub(crate) async fn update_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    id: i64,
    name: Option<String>,
    folder_id: NullablePatch<i64>,
) -> Result<file::Model> {
    let db = &state.db;
    let f = get_info_in_scope(state, scope, id).await?;
    if f.is_locked {
        return Err(AsterError::resource_locked("file is locked"));
    }

    let target_folder = match folder_id {
        NullablePatch::Absent => f.folder_id,
        NullablePatch::Null => None,
        NullablePatch::Value(fid) => Some(fid),
    };
    if let NullablePatch::Value(fid) = folder_id {
        workspace_storage_service::verify_folder_access(state, scope, fid).await?;
    }

    if let Some(ref n) = name {
        crate::utils::validate_name(n)?;
    }

    let final_name = name.as_deref().unwrap_or(&f.name);
    let existing = match scope {
        WorkspaceStorageScope::Personal { user_id } => {
            file_repo::find_by_name_in_folder(db, user_id, target_folder, final_name).await?
        }
        WorkspaceStorageScope::Team { team_id, .. } => {
            file_repo::find_by_name_in_team_folder(db, team_id, target_folder, final_name).await?
        }
    };
    if let Some(existing) = existing
        && existing.id != id
    {
        return Err(AsterError::validation_error(format!(
            "file '{}' already exists in this folder",
            final_name
        )));
    }

    let mut active: file::ActiveModel = f.into();
    if let Some(n) = name {
        active.name = Set(n);
    }
    match folder_id {
        NullablePatch::Absent => {}
        NullablePatch::Null => active.folder_id = Set(None),
        NullablePatch::Value(fid) => active.folder_id = Set(Some(fid)),
    }
    active.updated_at = Set(Utc::now());
    active.update(db).await.map_err(AsterError::from)
}

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
/// `skip_lock_check`: WebDAV 持锁者写入时为 true（dav-server 已验证 lock token）
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
) -> Result<file::Model> {
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
}

/// 上传文件（REST API，multipart）
pub async fn upload(
    state: &AppState,
    user_id: i64,
    payload: &mut actix_multipart::Multipart,
    folder_id: Option<i64>,
    relative_path: Option<&str>,
) -> Result<file::Model> {
    workspace_storage_service::upload(
        state,
        WorkspaceStorageScope::Personal { user_id },
        payload,
        folder_id,
        relative_path,
    )
    .await
}

/// 获取文件信息
pub async fn get_info(state: &AppState, id: i64, user_id: i64) -> Result<file::Model> {
    get_info_in_scope(state, WorkspaceStorageScope::Personal { user_id }, id).await
}

/// 下载文件（流式，不全量缓冲）
pub async fn download(
    state: &AppState,
    id: i64,
    user_id: i64,
    if_none_match: Option<&str>,
) -> Result<HttpResponse> {
    download_in_scope(
        state,
        WorkspaceStorageScope::Personal { user_id },
        id,
        if_none_match,
    )
    .await
}

/// 下载文件（无用户校验，用于分享链接，流式）
pub async fn download_raw(
    state: &AppState,
    id: i64,
    if_none_match: Option<&str>,
) -> Result<HttpResponse> {
    let db = &state.db;
    let f = file_repo::find_by_id(db, id).await?;
    ensure_personal_file_scope(&f)?;
    download_raw_unchecked_with_file(state, f, if_none_match).await
}

#[allow(dead_code)]
pub(crate) async fn download_raw_unchecked(
    state: &AppState,
    id: i64,
    if_none_match: Option<&str>,
) -> Result<HttpResponse> {
    let f = file_repo::find_by_id(&state.db, id).await?;
    download_raw_unchecked_with_file(state, f, if_none_match).await
}

async fn download_raw_unchecked_with_file(
    state: &AppState,
    f: file::Model,
    if_none_match: Option<&str>,
) -> Result<HttpResponse> {
    let blob = file_repo::find_blob_by_id(&state.db, f.blob_id).await?;
    build_stream_response(state, &f, &blob, if_none_match).await
}

pub(crate) fn if_none_match_matches(if_none_match: &str, blob_hash: &str) -> bool {
    if_none_match.split(',').any(|value| {
        let candidate = value.trim();
        candidate == "*" || candidate.trim_matches('"').eq_ignore_ascii_case(blob_hash)
    })
}

/// 构建流式下载响应
pub(crate) async fn build_stream_response(
    state: &AppState,
    f: &file::Model,
    blob: &file_blob::Model,
    if_none_match: Option<&str>,
) -> Result<HttpResponse> {
    let etag = format!("\"{}\"", blob.hash);
    if let Some(if_none_match) = if_none_match
        && if_none_match_matches(if_none_match, &blob.hash)
    {
        return Ok(HttpResponse::NotModified()
            .insert_header(("ETag", etag))
            .insert_header(("Cache-Control", "private, max-age=0, must-revalidate"))
            .finish());
    }

    let policy = state.policy_snapshot.get_policy_or_err(blob.policy_id)?;
    let driver = state.driver_registry.get_driver(&policy)?;
    let stream = driver.get_stream(&blob.storage_path).await?;

    // 64KB buffer — 比默认 4KB 减少系统调用和分配开销
    let reader_stream = tokio_util::io::ReaderStream::with_capacity(stream, 64 * 1024);

    Ok(HttpResponse::Ok()
        .content_type(f.mime_type.clone())
        .insert_header(("Content-Length", blob.size.to_string()))
        .insert_header((
            "Content-Disposition",
            format!("attachment; filename=\"{}\"", f.name),
        ))
        .insert_header(("ETag", etag))
        .insert_header(("Cache-Control", "private, max-age=0, must-revalidate"))
        // 跳过全局 Compress 中间件，避免压缩编码器缓冲导致内存暴涨
        .insert_header(("Content-Encoding", "identity"))
        .streaming(reader_stream))
}

/// 删除文件（软删除 → 回收站）
pub async fn delete(state: &AppState, id: i64, user_id: i64) -> Result<()> {
    delete_in_scope(state, WorkspaceStorageScope::Personal { user_id }, id).await
}

pub(crate) async fn cleanup_unreferenced_blob(state: &AppState, blob: &file_blob::Model) -> bool {
    async fn restore_cleanup_claim(state: &AppState, blob_id: i64, reason: &str) {
        match file_repo::restore_blob_cleanup_claim(&state.db, blob_id).await {
            Ok(true) => {}
            Ok(false) => {
                tracing::warn!(
                    blob_id,
                    "blob cleanup claim was already released while handling {reason}"
                );
            }
            Err(e) => {
                tracing::warn!(
                    blob_id,
                    "failed to restore blob cleanup claim after {reason}: {e}"
                );
            }
        }
    }

    let current_blob = match file_repo::find_blob_by_id(&state.db, blob.id).await {
        Ok(current_blob) => current_blob,
        Err(e) if e.code() == "E006" => return true,
        Err(e) => {
            tracing::warn!(
                blob_id = blob.id,
                "failed to reload blob before cleanup: {e}"
            );
            return false;
        }
    };

    if current_blob.ref_count != 0 {
        tracing::warn!(
            blob_id = current_blob.id,
            ref_count = current_blob.ref_count,
            "skipping blob cleanup because blob is referenced again"
        );
        return false;
    }

    match file_repo::claim_blob_cleanup(&state.db, current_blob.id).await {
        Ok(true) => {}
        Ok(false) => {
            tracing::warn!(
                blob_id = current_blob.id,
                "skipping blob cleanup because another worker already claimed it or it was revived"
            );
            return false;
        }
        Err(e) => {
            tracing::warn!(
                blob_id = current_blob.id,
                "failed to claim blob cleanup: {e}"
            );
            return false;
        }
    }

    if let Err(e) = thumbnail_service::delete_thumbnail(state, &current_blob).await {
        tracing::warn!(
            blob_id = current_blob.id,
            "failed to delete thumbnail during blob cleanup: {e}"
        );
    }

    let Some(policy) = state.policy_snapshot.get_policy(current_blob.policy_id) else {
        tracing::warn!(
            blob_id = current_blob.id,
            policy_id = current_blob.policy_id,
            "failed to load storage policy during blob cleanup: policy missing from snapshot"
        );
        restore_cleanup_claim(state, current_blob.id, "policy lookup failure").await;
        return false;
    };

    let driver = match state.driver_registry.get_driver(&policy) {
        Ok(driver) => driver,
        Err(e) => {
            tracing::warn!(
                blob_id = current_blob.id,
                policy_id = current_blob.policy_id,
                "failed to resolve storage driver during blob cleanup: {e}"
            );
            restore_cleanup_claim(state, current_blob.id, "driver resolution failure").await;
            return false;
        }
    };

    let object_deleted = match driver.delete(&current_blob.storage_path).await {
        Ok(()) => true,
        Err(e) => match driver.exists(&current_blob.storage_path).await {
            Ok(false) => {
                tracing::warn!(
                    blob_id = current_blob.id,
                    path = %current_blob.storage_path,
                    "blob delete returned error but object is already absent: {e}"
                );
                true
            }
            Ok(true) => {
                tracing::warn!(
                    blob_id = current_blob.id,
                    path = %current_blob.storage_path,
                    "failed to delete blob object, keeping blob row for retry: {e}"
                );
                restore_cleanup_claim(state, current_blob.id, "delete error").await;
                false
            }
            Err(exists_err) => {
                tracing::warn!(
                    blob_id = current_blob.id,
                    path = %current_blob.storage_path,
                    "failed to delete blob object and verify existence, keeping blob row for retry: delete_error={e}, exists_error={exists_err}"
                );
                restore_cleanup_claim(state, current_blob.id, "delete verification error").await;
                false
            }
        },
    };

    if !object_deleted {
        return false;
    }

    match file_repo::delete_blob_if_cleanup_claimed(&state.db, current_blob.id).await {
        Ok(true) => true,
        Ok(false) => {
            tracing::warn!(
                blob_id = current_blob.id,
                "blob object is gone but cleanup claim was lost before deleting blob row"
            );
            restore_cleanup_claim(
                state,
                current_blob.id,
                "lost cleanup claim before row delete",
            )
            .await;
            false
        }
        Err(e) => {
            tracing::warn!(
                blob_id = current_blob.id,
                "blob object is gone but failed to delete blob row: {e}"
            );
            restore_cleanup_claim(state, current_blob.id, "row delete failure").await;
            false
        }
    }
}

pub(crate) async fn purge_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    id: i64,
) -> Result<()> {
    workspace_storage_service::require_scope_access(state, scope).await?;

    let file = file_repo::find_by_id(&state.db, id).await?;
    workspace_storage_service::ensure_file_scope(&file, scope)?;

    batch_purge_in_scope(state, scope, vec![file]).await?;
    Ok(())
}

/// 永久删除文件，处理 blob ref_count、物理文件、缩略图和配额。
pub async fn purge(state: &AppState, id: i64, user_id: i64) -> Result<()> {
    purge_in_scope(state, WorkspaceStorageScope::Personal { user_id }, id).await
}

/// 批量永久删除文件：一次事务处理所有 DB 操作，事务后并行清理物理文件
///
/// 比逐个调 `purge()` 快得多——N 个文件只需 ~10 次 DB 查询而非 ~12N 次。
pub(crate) async fn batch_purge_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    files: Vec<file::Model>,
) -> Result<u32> {
    if files.is_empty() {
        return Ok(0);
    }

    for file in &files {
        workspace_storage_service::ensure_file_scope(file, scope)?;
    }

    let file_ids: Vec<i64> = files.iter().map(|f| f.id).collect();
    let blob_ids: Vec<i64> = files.iter().map(|f| f.blob_id).collect();
    let count = files.len() as u32;

    // ── 单次事务：版本 → 属性 → 文件 → blob → 配额 ──
    let txn = state.db.begin().await.map_err(AsterError::from)?;

    // 1. 批量删除版本记录，收集版本 blob IDs
    let version_blob_ids =
        crate::db::repository::version_repo::delete_all_by_file_ids(&txn, &file_ids).await?;

    // 2. 批量删除文件属性
    crate::db::repository::property_repo::delete_all_for_entities(
        &txn,
        crate::types::EntityType::File,
        &file_ids,
    )
    .await?;

    // 3. 批量删除文件记录（先于 blob，解除 FK）
    file_repo::delete_many(&txn, &file_ids).await?;

    // 4. 处理 blob 引用计数
    //    合并主 blob 和版本 blob，按 blob_id 统计需要减少的引用数
    let mut blob_decrements: std::collections::HashMap<i64, i64> = std::collections::HashMap::new();
    for &bid in &blob_ids {
        *blob_decrements.entry(bid).or_default() += 1;
    }
    for &vbid in &version_blob_ids {
        *blob_decrements.entry(vbid).or_default() += 1;
    }

    let blob_ids: Vec<i64> = blob_decrements.keys().copied().collect();
    let blobs_by_id = file_repo::find_blobs_by_ids(&txn, &blob_ids).await?;
    let mut blobs_to_cleanup: Vec<file_blob::Model> = Vec::new();
    let mut total_freed_bytes = 0i64;

    for (&blob_id, &decrement) in &blob_decrements {
        if let Some(blob) = blobs_by_id.get(&blob_id) {
            let freed_bytes = blob.size.checked_mul(decrement).ok_or_else(|| {
                AsterError::internal_error(format!(
                    "freed byte count overflow for blob {blob_id} during batch purge"
                ))
            })?;
            total_freed_bytes = total_freed_bytes.checked_add(freed_bytes).ok_or_else(|| {
                AsterError::internal_error("total freed byte count overflow during batch purge")
            })?;
            let decrement_i32 = i32::try_from(decrement).map_err(|_| {
                AsterError::internal_error(format!(
                    "blob decrement overflow for blob {blob_id} during batch purge"
                ))
            })?;
            file_repo::decrement_blob_ref_count_by(&txn, blob_id, decrement_i32).await?;
            if i64::from(blob.ref_count) <= decrement {
                blobs_to_cleanup.push(blob.clone());
            }
        }
    }

    // 5. 配额一次性更新
    workspace_storage_service::update_storage_used(&txn, scope, -total_freed_bytes).await?;

    txn.commit().await.map_err(AsterError::from)?;

    // ── 事务后：并行物理清理，清理成功后再删 blob 元数据 ──
    stream::iter(blobs_to_cleanup.into_iter())
        .for_each_concurrent(BLOB_CLEANUP_CONCURRENCY, |blob| async move {
            if !cleanup_unreferenced_blob(state, &blob).await {
                tracing::warn!(
                    blob_id = blob.id,
                    "batch purge left blob row for retry because object cleanup was incomplete"
                );
            }
        })
        .await;

    Ok(count)
}

pub async fn batch_purge(state: &AppState, files: Vec<file::Model>, user_id: i64) -> Result<u32> {
    batch_purge_in_scope(state, WorkspaceStorageScope::Personal { user_id }, files).await
}

/// 更新文件（重命名/移动）
pub async fn update(
    state: &AppState,
    id: i64,
    user_id: i64,
    name: Option<String>,
    folder_id: NullablePatch<i64>,
) -> Result<file::Model> {
    update_in_scope(
        state,
        WorkspaceStorageScope::Personal { user_id },
        id,
        name,
        folder_id,
    )
    .await
}

/// 移动文件到指定文件夹（None = 根目录）
///
/// 与 `update()` 的区别：`update()` 用 `NullablePatch<i64>` 区分
/// “未传字段”和“显式传 null”，而本函数的 `target_folder_id: None`
/// 明确表示“移到根目录”。
pub async fn move_file(
    state: &AppState,
    id: i64,
    user_id: i64,
    target_folder_id: Option<i64>,
) -> Result<file::Model> {
    update_in_scope(
        state,
        WorkspaceStorageScope::Personal { user_id },
        id,
        None,
        match target_folder_id {
            Some(folder_id) => NullablePatch::Value(folder_id),
            None => NullablePatch::Null,
        },
    )
    .await
}

pub(crate) async fn copy_file_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    src_id: i64,
    dest_folder_id: Option<i64>,
) -> Result<file::Model> {
    let db = &state.db;
    let src = get_info_in_scope(state, scope, src_id).await?;

    if let Some(folder_id) = dest_folder_id {
        workspace_storage_service::verify_folder_access(state, scope, folder_id).await?;
    }

    let blob = file_repo::find_blob_by_id(db, src.blob_id).await?;
    workspace_storage_service::check_quota(db, scope, blob.size).await?;

    let copy_name = match scope {
        WorkspaceStorageScope::Personal { user_id } => {
            file_repo::resolve_unique_filename(db, user_id, dest_folder_id, &src.name).await?
        }
        WorkspaceStorageScope::Team { team_id, .. } => {
            file_repo::resolve_unique_team_filename(db, team_id, dest_folder_id, &src.name).await?
        }
    };

    duplicate_file_record_in_scope(state, scope, &src, dest_folder_id, &copy_name).await
}

/// 复制文件（REST API 入口，带权限检查 + 副本命名）
///
/// `dest_folder_id = None` 表示复制到根目录。
pub async fn copy_file(
    state: &AppState,
    src_id: i64,
    user_id: i64,
    dest_folder_id: Option<i64>,
) -> Result<file::Model> {
    copy_file_in_scope(
        state,
        WorkspaceStorageScope::Personal { user_id },
        src_id,
        dest_folder_id,
    )
    .await
}

#[derive(Clone)]
pub(crate) struct BatchDuplicateFileRecordSpec {
    pub src: file::Model,
    pub dest_name: String,
}

async fn batch_duplicate_file_records_with_specs_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    copy_specs: &[BatchDuplicateFileRecordSpec],
    dest_folder_id: Option<i64>,
) -> Result<()> {
    if copy_specs.is_empty() {
        return Ok(());
    }

    let total_size = copy_specs.iter().try_fold(0i64, |acc, spec| {
        acc.checked_add(spec.src.size).ok_or_else(|| {
            AsterError::internal_error("total copied byte count overflow during batch copy")
        })
    })?;
    let now = chrono::Utc::now();

    workspace_storage_service::check_quota(&state.db, scope, total_size).await?;

    let txn = state.db.begin().await.map_err(AsterError::from)?;
    workspace_storage_service::check_quota(&txn, scope, total_size).await?;

    let mut blob_counts: std::collections::HashMap<i64, i32> = std::collections::HashMap::new();
    for spec in copy_specs {
        let entry = blob_counts.entry(spec.src.blob_id).or_default();
        *entry = entry.checked_add(1).ok_or_else(|| {
            AsterError::internal_error(format!(
                "blob copy count overflow for blob {} during batch copy",
                spec.src.blob_id
            ))
        })?;
    }
    for (&blob_id, &count) in &blob_counts {
        file_repo::increment_blob_ref_count_by(&txn, blob_id, count).await?;
    }

    let models: Vec<file::ActiveModel> = copy_specs
        .iter()
        .map(|spec| file::ActiveModel {
            name: Set(spec.dest_name.clone()),
            folder_id: Set(dest_folder_id),
            team_id: Set(scope.team_id()),
            blob_id: Set(spec.src.blob_id),
            size: Set(spec.src.size),
            user_id: Set(scope.actor_user_id()),
            mime_type: Set(spec.src.mime_type.clone()),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        })
        .collect();
    file_repo::create_many(&txn, models).await?;

    workspace_storage_service::update_storage_used(&txn, scope, total_size).await?;

    txn.commit().await.map_err(AsterError::from)?;
    Ok(())
}

pub(crate) async fn duplicate_file_record_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    src: &file::Model,
    dest_folder_id: Option<i64>,
    dest_name: &str,
) -> Result<file::Model> {
    let blob = file_repo::find_blob_by_id(&state.db, src.blob_id).await?;
    let now = Utc::now();
    let blob_size = blob.size;

    let txn = state.db.begin().await.map_err(AsterError::from)?;
    workspace_storage_service::check_quota(&txn, scope, blob_size).await?;

    file_repo::increment_blob_ref_count(&txn, blob.id).await?;

    let new_file = file_repo::create(
        &txn,
        file::ActiveModel {
            name: Set(dest_name.to_string()),
            folder_id: Set(dest_folder_id),
            team_id: Set(scope.team_id()),
            blob_id: Set(src.blob_id),
            size: Set(src.size),
            user_id: Set(scope.actor_user_id()),
            mime_type: Set(src.mime_type.clone()),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        },
    )
    .await?;

    workspace_storage_service::update_storage_used(&txn, scope, blob_size).await?;

    txn.commit().await.map_err(AsterError::from)?;

    Ok(new_file)
}

/// 复制文件记录的核心逻辑（blob ref_count++ + 新文件记录 + 配额更新）
///
/// 无权限检查，供 REST copy、WebDAV COPY、recursive_copy_folder 共用。
pub async fn duplicate_file_record(
    state: &AppState,
    src: &file::Model,
    dest_folder_id: Option<i64>,
    dest_name: &str,
) -> Result<file::Model> {
    duplicate_file_record_in_scope(
        state,
        WorkspaceStorageScope::Personal {
            user_id: src.user_id,
        },
        src,
        dest_folder_id,
        dest_name,
    )
    .await
}

pub(crate) async fn batch_duplicate_file_records_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    src_files: &[file::Model],
    dest_folder_id: Option<i64>,
) -> Result<()> {
    let copy_specs: Vec<BatchDuplicateFileRecordSpec> = src_files
        .iter()
        .cloned()
        .map(|src| BatchDuplicateFileRecordSpec {
            dest_name: src.name.clone(),
            src,
        })
        .collect();

    batch_duplicate_file_records_with_specs_in_scope(state, scope, &copy_specs, dest_folder_id)
        .await
}

pub(crate) async fn batch_duplicate_file_records_with_names_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    copy_specs: &[BatchDuplicateFileRecordSpec],
    dest_folder_id: Option<i64>,
) -> Result<()> {
    batch_duplicate_file_records_with_specs_in_scope(state, scope, copy_specs, dest_folder_id).await
}

/// 批量复制文件记录：一次事务处理 blob ref_count + 文件创建 + 配额
///
/// 与 `duplicate_file_record` 的区别：N 个文件只开 1 次事务，
/// blob ref_count 按 blob_id 合并递增，配额只更新一次。
/// 不返回创建的 Model（递归复制场景不需要）。
pub async fn batch_duplicate_file_records(
    state: &AppState,
    src_files: &[file::Model],
    dest_folder_id: Option<i64>,
) -> Result<()> {
    if src_files.is_empty() {
        return Ok(());
    }

    batch_duplicate_file_records_in_scope(
        state,
        WorkspaceStorageScope::Personal {
            user_id: src_files[0].user_id,
        },
        src_files,
        dest_folder_id,
    )
    .await
}

pub(crate) async fn update_content_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    file_id: i64,
    body: actix_web::web::Bytes,
    if_match: Option<&str>,
) -> Result<(file::Model, String)> {
    let db = &state.db;
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

    let current_blob = file_repo::find_blob_by_id(db, f.blob_id).await?;
    if let Some(etag) = if_match {
        let expected = etag.trim_matches('"');
        if !expected.eq_ignore_ascii_case(&current_blob.hash) {
            return Err(AsterError::precondition_failed(
                "file has been modified (ETag mismatch)",
            ));
        }
    }

    let temp_dir = &state.config.server.temp_dir;
    let temp_path =
        crate::utils::paths::temp_file_path(temp_dir, &uuid::Uuid::new_v4().to_string());
    tokio::fs::create_dir_all(temp_dir)
        .await
        .map_aster_err(AsterError::storage_driver_error)?;
    tokio::fs::write(&temp_path, &body)
        .await
        .map_aster_err(AsterError::storage_driver_error)?;

    let size = body.len() as i64;
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

    let updated = result?;
    let new_blob = file_repo::find_blob_by_id(db, updated.blob_id).await?;
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
    body: actix_web::web::Bytes,
    if_match: Option<&str>,
) -> Result<(file::Model, String)> {
    update_content_in_scope(
        state,
        WorkspaceStorageScope::Personal { user_id },
        file_id,
        body,
        if_match,
    )
    .await
}

/// 根据优先级链解析存储策略：文件夹覆盖 → 用户绑定策略组
pub async fn resolve_policy(
    state: &AppState,
    user_id: i64,
    folder_id: Option<i64>,
) -> Result<crate::entities::storage_policy::Model> {
    resolve_policy_for_size(state, user_id, folder_id, 0).await
}

pub async fn resolve_policy_for_size(
    state: &AppState,
    user_id: i64,
    folder_id: Option<i64>,
    file_size: i64,
) -> Result<crate::entities::storage_policy::Model> {
    workspace_storage_service::resolve_policy_for_size(
        state,
        WorkspaceStorageScope::Personal { user_id },
        folder_id,
        file_size,
    )
    .await
}

pub(crate) async fn set_lock_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    file_id: i64,
    locked: bool,
) -> Result<file::Model> {
    use crate::services::lock_service;
    use crate::types::EntityType;

    get_info_in_scope(state, scope, file_id).await?;

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

    get_info_in_scope(state, scope, file_id).await
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
) -> Result<file::Model> {
    workspace_storage_service::create_empty(
        state,
        WorkspaceStorageScope::Personal { user_id },
        folder_id,
        filename,
    )
    .await
}

// ── Lock ─────────────────────────────────────────────────────────────

/// 设置/解除文件锁，返回更新后的文件信息
pub async fn set_lock(
    state: &AppState,
    file_id: i64,
    user_id: i64,
    locked: bool,
) -> Result<file::Model> {
    set_lock_in_scope(
        state,
        WorkspaceStorageScope::Personal { user_id },
        file_id,
        locked,
    )
    .await
}

// ── Thumbnail ────────────────────────────────────────────────────────

/// 缩略图查询结果：有数据直接返回，正在生成则标记 pending
pub struct ThumbnailResult {
    pub data: Vec<u8>,
}

pub(crate) async fn get_thumbnail_data_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    file_id: i64,
) -> Result<Option<ThumbnailResult>> {
    let f = get_info_in_scope(state, scope, file_id).await?;
    if !thumbnail_service::is_supported_mime(&f.mime_type) {
        return Err(AsterError::thumbnail_generation_failed(
            "unsupported image type",
        ));
    }
    let blob = file_repo::find_blob_by_id(&state.db, f.blob_id).await?;
    match thumbnail_service::get_or_enqueue(state, &blob).await? {
        Some(data) => Ok(Some(ThumbnailResult { data })),
        None => Ok(None),
    }
}

/// 获取文件缩略图。返回 `Ok(Some(data))` 直接有图；`Ok(None)` 表示正在后台生成。
pub async fn get_thumbnail_data(
    state: &AppState,
    file_id: i64,
    user_id: i64,
) -> Result<Option<ThumbnailResult>> {
    get_thumbnail_data_in_scope(state, WorkspaceStorageScope::Personal { user_id }, file_id).await
}
