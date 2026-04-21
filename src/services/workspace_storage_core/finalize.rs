use chrono::Utc;
use sea_orm::ConnectionTrait;

use crate::db::repository::{file_repo, upload_session_repo};
use crate::entities::{file, file_blob, upload_session};
use crate::errors::{AsterError, Result};
use crate::runtime::PrimaryAppState;
use crate::services::storage_change_service;
use crate::services::workspace_scope_service::WorkspaceStorageScope;

use super::file_record::create_new_file_from_blob;
use super::quota::{check_quota, update_storage_used};

pub(crate) async fn finalize_upload_session_blob<C: ConnectionTrait>(
    db: &C,
    session: &upload_session::Model,
    blob: &file_blob::Model,
    now: chrono::DateTime<Utc>,
) -> Result<file::Model> {
    // “最终完成一个 upload session”在数据库侧必须保持固定顺序：
    // 先建文件，再记配额，最后把 session 状态切到 completed。
    // 这样调用方只要看到 completed，就能推定文件记录已经可见且额度已落账。
    let scope = scope_from_session(session);
    let created =
        create_new_file_from_blob(db, scope, session.folder_id, &session.filename, blob, now)
            .await?;

    update_storage_used(db, scope, blob.size).await?;
    mark_upload_session_completed(db, &session.id, created.id).await?;
    Ok(created)
}

pub(crate) struct FinalizeUploadSessionFileParams<'a> {
    pub session: &'a upload_session::Model,
    pub file_hash: &'a str,
    pub size: i64,
    pub policy_id: i64,
    pub storage_path: &'a str,
    pub now: chrono::DateTime<Utc>,
}

pub(crate) async fn finalize_upload_session_file(
    state: &PrimaryAppState,
    params: FinalizeUploadSessionFileParams<'_>,
) -> Result<file::Model> {
    let FinalizeUploadSessionFileParams {
        session,
        file_hash,
        size,
        policy_id,
        storage_path,
        now,
    } = params;
    let scope = scope_from_session(session);
    let txn = crate::db::transaction::begin(&state.db).await?;
    check_quota(&txn, scope, size).await?;

    let blob =
        file_repo::find_or_create_blob(&txn, file_hash, size, policy_id, storage_path).await?;
    let created = finalize_upload_session_blob(&txn, session, &blob.model, now).await?;

    crate::db::transaction::commit(txn).await?;
    storage_change_service::publish(
        state,
        storage_change_service::StorageChangeEvent::new(
            storage_change_service::StorageChangeKind::FileCreated,
            scope,
            vec![created.id],
            vec![],
            vec![created.folder_id],
        ),
    );
    Ok(created)
}

async fn mark_upload_session_completed<C: ConnectionTrait>(
    db: &C,
    session_id: &str,
    file_id: i64,
) -> Result<()> {
    if upload_session_repo::complete_if_assembling(db, session_id, file_id).await? {
        return Ok(());
    }

    let session_fresh = upload_session_repo::find_by_id(db, session_id).await?;
    if session_fresh.status == crate::types::UploadSessionStatus::Failed {
        return Err(AsterError::upload_assembly_failed(
            "upload was canceled during assembly",
        ));
    }

    Err(AsterError::upload_assembly_failed(format!(
        "session status is '{:?}', expected 'assembling'",
        session_fresh.status
    )))
}

fn scope_from_session(session: &upload_session::Model) -> WorkspaceStorageScope {
    // upload session 已经把“文件最终归属到个人还是团队”持久化下来了，
    // 因此最终装配阶段不需要再回看 route 层上下文。
    match session.team_id {
        Some(team_id) => WorkspaceStorageScope::Team {
            team_id,
            actor_user_id: session.user_id,
        },
        None => WorkspaceStorageScope::Personal {
            user_id: session.user_id,
        },
    }
}
