use chrono::{DateTime, Utc};
use sea_orm::Set;

use crate::db::repository::upload_session_repo;
use crate::entities::{storage_policy, upload_session};
use crate::errors::{AsterError, Result};
use crate::runtime::AppState;
use crate::services::upload_service::responses::InitUploadResponse;
use crate::services::workspace_storage_service::{self, WorkspaceStorageScope};
use crate::types::{UploadMode, UploadSessionStatus};

#[derive(Debug)]
pub(super) struct ResolvedUploadTarget {
    pub(super) folder_id: Option<i64>,
    pub(super) filename: String,
}

pub(super) struct InitUploadContext {
    pub(super) scope: WorkspaceStorageScope,
    pub(super) target: ResolvedUploadTarget,
    pub(super) total_size: i64,
    pub(super) policy: storage_policy::Model,
}

pub(super) struct UploadSessionRecordParams {
    pub(super) upload_id: String,
    pub(super) scope: WorkspaceStorageScope,
    pub(super) filename: String,
    pub(super) total_size: i64,
    pub(super) chunk_size: i64,
    pub(super) total_chunks: i32,
    pub(super) folder_id: Option<i64>,
    pub(super) policy_id: i64,
    pub(super) status: UploadSessionStatus,
    pub(super) s3_temp_key: Option<String>,
    pub(super) s3_multipart_id: Option<String>,
    pub(super) expires_at: DateTime<Utc>,
}

pub(super) async fn resolve_init_upload_context(
    state: &AppState,
    scope: WorkspaceStorageScope,
    filename: &str,
    total_size: i64,
    folder_id: Option<i64>,
    relative_path: Option<&str>,
) -> Result<InitUploadContext> {
    let target = resolve_upload_target(state, scope, filename, folder_id, relative_path).await?;

    tracing::debug!(
        scope = ?scope,
        folder_id = target.folder_id,
        filename = %target.filename,
        "resolved upload session target"
    );

    let policy = resolve_init_upload_policy(state, scope, target.folder_id, total_size).await?;

    tracing::debug!(
        scope = ?scope,
        policy_id = policy.id,
        driver_type = ?policy.driver_type,
        chunk_size = policy.chunk_size,
        total_size,
        "resolved upload storage policy"
    );

    Ok(InitUploadContext {
        scope,
        target,
        total_size,
        policy,
    })
}

async fn resolve_upload_target(
    state: &AppState,
    scope: WorkspaceStorageScope,
    filename: &str,
    folder_id: Option<i64>,
    relative_path: Option<&str>,
) -> Result<ResolvedUploadTarget> {
    match relative_path {
        Some(path) => {
            // 目录上传会把 `relative_path` 拆成“父目录链 + 最终文件名”。
            // 这里就把目录路径补齐，后续模式选择和 session 记录都只看解析后的最终目标。
            let parsed = workspace_storage_service::parse_relative_upload_path(
                state, scope, folder_id, path,
            )
            .await?;
            let resolved_folder_id =
                workspace_storage_service::ensure_upload_parent_path(state, scope, &parsed).await?;
            Ok(ResolvedUploadTarget {
                folder_id: resolved_folder_id,
                filename: parsed.filename,
            })
        }
        None => {
            crate::utils::validate_name(filename)?;
            if let Some(folder_id) = folder_id {
                workspace_storage_service::verify_folder_access(state, scope, folder_id).await?;
            }
            Ok(ResolvedUploadTarget {
                folder_id,
                filename: filename.to_string(),
            })
        }
    }
}

async fn resolve_init_upload_policy(
    state: &AppState,
    scope: WorkspaceStorageScope,
    folder_id: Option<i64>,
    total_size: i64,
) -> Result<storage_policy::Model> {
    // upload 模式协商建立在“最终会写到哪条策略”之上，而不是客户端自己传 mode。
    let policy =
        workspace_storage_service::resolve_policy_for_size(state, scope, folder_id, total_size)
            .await?;
    validate_policy_upload_size(&policy, total_size)?;
    workspace_storage_service::check_quota(&state.db, scope, total_size).await?;
    Ok(policy)
}

fn validate_policy_upload_size(policy: &storage_policy::Model, total_size: i64) -> Result<()> {
    if policy.max_file_size > 0 && total_size > policy.max_file_size {
        return Err(AsterError::file_too_large(format!(
            "file size {} exceeds limit {}",
            total_size, policy.max_file_size
        )));
    }
    Ok(())
}

pub(super) async fn persist_upload_session(
    db: &sea_orm::DatabaseConnection,
    params: UploadSessionRecordParams,
) -> Result<()> {
    let UploadSessionRecordParams {
        upload_id,
        scope,
        filename,
        total_size,
        chunk_size,
        total_chunks,
        folder_id,
        policy_id,
        status,
        s3_temp_key,
        s3_multipart_id,
        expires_at,
    } = params;
    let now = Utc::now();

    let session = upload_session::ActiveModel {
        id: Set(upload_id),
        user_id: Set(scope.actor_user_id()),
        team_id: Set(scope.team_id()),
        filename: Set(filename),
        total_size: Set(total_size),
        chunk_size: Set(chunk_size),
        total_chunks: Set(total_chunks),
        received_count: Set(0),
        folder_id: Set(folder_id),
        policy_id: Set(policy_id),
        status: Set(status),
        s3_temp_key: Set(s3_temp_key),
        s3_multipart_id: Set(s3_multipart_id),
        file_id: Set(None),
        created_at: Set(now),
        expires_at: Set(expires_at),
        updated_at: Set(now),
    };
    upload_session_repo::create(db, session).await?;
    Ok(())
}

pub(super) fn upload_fits_single_request(total_size: i64, chunk_size: i64) -> bool {
    chunk_size == 0 || total_size <= chunk_size
}

pub(super) fn direct_upload_response() -> InitUploadResponse {
    InitUploadResponse {
        mode: UploadMode::Direct,
        upload_id: None,
        chunk_size: None,
        total_chunks: None,
        presigned_url: None,
    }
}

pub(super) fn chunked_upload_response(
    mode: UploadMode,
    upload_id: String,
    chunk_size: i64,
    total_chunks: i32,
) -> InitUploadResponse {
    InitUploadResponse {
        mode,
        upload_id: Some(upload_id),
        chunk_size: Some(chunk_size),
        total_chunks: Some(total_chunks),
        presigned_url: None,
    }
}
