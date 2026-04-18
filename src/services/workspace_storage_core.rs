//! 工作空间文件主链路的稳定核心动作。
//!
//! 这里尽量只保留“上传方式无关、HTTP 接入无关”的底层语义，例如：
//! 策略解析、目录路径补齐、blob / 文件记录创建、配额读写和 upload session
//! 最终落账。这样不同入口才能共享同一套文件一致性规则。

use chrono::Utc;
use sea_orm::{ActiveModelTrait, ConnectionTrait, Set};

use crate::db::repository::{file_repo, folder_repo, team_repo, upload_session_repo, user_repo};
use crate::entities::{file, file_blob, folder, team, upload_session};
use crate::errors::{AsterError, MapAsterErr, Result};
use crate::runtime::AppState;
use crate::services::{
    storage_change_service,
    workspace_scope_service::{WorkspaceStorageScope, require_team_access, verify_folder_access},
};
use crate::types::{DriverType, parse_storage_policy_options};

const MAX_AUTO_NAME_RETRIES: usize = 32;

pub(crate) async fn load_storage_limits(
    state: &AppState,
    scope: WorkspaceStorageScope,
) -> Result<(i64, i64)> {
    match scope {
        WorkspaceStorageScope::Personal { user_id } => {
            let user = user_repo::find_by_id(&state.db, user_id).await?;
            Ok((user.storage_used, user.storage_quota))
        }
        WorkspaceStorageScope::Team { team_id, .. } => {
            let team = team_repo::find_active_by_id(&state.db, team_id).await?;
            Ok((team.storage_used, team.storage_quota))
        }
    }
}

pub(crate) fn local_content_dedup_enabled(policy: &crate::entities::storage_policy::Model) -> bool {
    policy.driver_type == DriverType::Local
        && parse_storage_policy_options(policy.options.as_ref())
            .content_dedup
            .unwrap_or(false)
}

pub(crate) async fn create_nondedup_blob_with_key<C: ConnectionTrait>(
    db: &C,
    size: i64,
    policy_id: i64,
    blob_key: &str,
    storage_path: &str,
) -> Result<file_blob::Model> {
    let now = Utc::now();

    file_repo::create_blob(
        db,
        file_blob::ActiveModel {
            hash: Set(blob_key.to_string()),
            size: Set(size),
            policy_id: Set(policy_id),
            storage_path: Set(storage_path.to_string()),
            ref_count: Set(1),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        },
    )
    .await
}

pub(crate) async fn create_nondedup_blob<C: ConnectionTrait>(
    db: &C,
    size: i64,
    policy_id: i64,
) -> Result<file_blob::Model> {
    let blob_key = crate::utils::id::new_short_token();
    let storage_path = crate::utils::storage_path_from_blob_key(&blob_key);

    create_nondedup_blob_with_key(db, size, policy_id, &blob_key, &storage_path).await
}

pub(crate) async fn create_s3_nondedup_blob<C: ConnectionTrait>(
    db: &C,
    size: i64,
    policy_id: i64,
    upload_id: &str,
) -> Result<file_blob::Model> {
    let now = Utc::now();
    let file_hash = format!("s3-{upload_id}");
    let storage_path = format!("files/{upload_id}");

    file_repo::create_blob(
        db,
        file_blob::ActiveModel {
            hash: Set(file_hash),
            size: Set(size),
            policy_id: Set(policy_id),
            storage_path: Set(storage_path),
            ref_count: Set(1),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        },
    )
    .await
}

pub(crate) async fn mark_upload_session_completed<C: ConnectionTrait>(
    db: &C,
    session_id: &str,
    file_id: i64,
) -> Result<()> {
    use crate::entities::upload_session::{Column, Entity as UploadSession};
    use sea_orm::{ActiveEnum, ColumnTrait, EntityTrait, QueryFilter, sea_query::Expr};

    let now = Utc::now();
    let result = UploadSession::update_many()
        .col_expr(
            Column::Status,
            Expr::value(crate::types::UploadSessionStatus::Completed.to_value()),
        )
        .col_expr(Column::FileId, Expr::value(Some(file_id)))
        .col_expr(Column::UpdatedAt, Expr::value(now))
        .filter(Column::Id.eq(session_id))
        .filter(Column::Status.eq(crate::types::UploadSessionStatus::Assembling))
        .exec(db)
        .await
        .map_aster_err(AsterError::database_operation)?;

    if result.rows_affected == 1 {
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

fn resolve_team_policy_group_id(team: &team::Model) -> Result<i64> {
    team.policy_group_id.ok_or_else(|| {
        AsterError::storage_policy_not_found(format!(
            "no storage policy group assigned to team #{}",
            team.id
        ))
    })
}

pub(crate) async fn resolve_policy_for_size(
    state: &AppState,
    scope: WorkspaceStorageScope,
    folder_id: Option<i64>,
    file_size: i64,
) -> Result<crate::entities::storage_policy::Model> {
    // 文件夹级策略覆盖优先级最高。
    // 只有目标文件夹没有显式绑定策略时，才回退到个人默认策略 / 团队策略组。
    if let Some(folder_id) = folder_id {
        let folder = verify_folder_access(state, scope, folder_id).await?;

        if let Some(policy_id) = folder.policy_id {
            return state.policy_snapshot.get_policy_or_err(policy_id);
        }
    }

    match scope {
        WorkspaceStorageScope::Personal { user_id } => state
            .policy_snapshot
            .resolve_user_policy_for_size(user_id, file_size),
        WorkspaceStorageScope::Team {
            team_id,
            actor_user_id,
        } => {
            let team = require_team_access(state, team_id, actor_user_id).await?;
            state
                .policy_snapshot
                .resolve_policy_in_group(resolve_team_policy_group_id(&team)?, file_size)
        }
    }
}

async fn ensure_folder_in_parent<C: ConnectionTrait>(
    db: &C,
    scope: WorkspaceStorageScope,
    parent_id: Option<i64>,
    name: &str,
) -> Result<folder::Model> {
    // 目录上传 / 解压导入会并发命中同一路径。
    // 这里先查后建，并在插入冲突后回读，保证“得到该目录”的语义是幂等的。
    crate::utils::validate_name(name)?;

    match scope {
        WorkspaceStorageScope::Personal { user_id } => {
            if let Some(existing) =
                folder_repo::find_by_name_in_parent(db, user_id, parent_id, name).await?
            {
                return Ok(existing);
            }

            let now = Utc::now();
            let model = folder::ActiveModel {
                name: Set(name.to_string()),
                parent_id: Set(parent_id),
                user_id: Set(user_id),
                policy_id: Set(None),
                created_at: Set(now),
                updated_at: Set(now),
                ..Default::default()
            };

            match folder_repo::create(db, model).await {
                Ok(created) => Ok(created),
                Err(err) => {
                    if let Some(existing) =
                        folder_repo::find_by_name_in_parent(db, user_id, parent_id, name).await?
                    {
                        Ok(existing)
                    } else {
                        Err(err)
                    }
                }
            }
        }
        WorkspaceStorageScope::Team {
            team_id,
            actor_user_id,
        } => {
            if let Some(existing) =
                folder_repo::find_by_name_in_team_parent(db, team_id, parent_id, name).await?
            {
                return Ok(existing);
            }

            let now = Utc::now();
            let model = folder::ActiveModel {
                name: Set(name.to_string()),
                parent_id: Set(parent_id),
                team_id: Set(Some(team_id)),
                user_id: Set(actor_user_id),
                policy_id: Set(None),
                created_at: Set(now),
                updated_at: Set(now),
                ..Default::default()
            };

            match folder_repo::create(db, model).await {
                Ok(created) => Ok(created),
                Err(err) => {
                    if let Some(existing) =
                        folder_repo::find_by_name_in_team_parent(db, team_id, parent_id, name)
                            .await?
                    {
                        Ok(existing)
                    } else {
                        Err(err)
                    }
                }
            }
        }
    }
}

pub(crate) struct ParsedUploadPath {
    pub base_folder_id: Option<i64>,
    pub parent_segments: Vec<String>,
    pub filename: String,
}

#[derive(Clone, Copy)]
pub(crate) enum NewFileNameMode {
    ResolveUnique,
    Exact,
}

pub(crate) async fn parse_relative_upload_path(
    state: &AppState,
    scope: WorkspaceStorageScope,
    base_folder_id: Option<i64>,
    relative_path: &str,
) -> Result<ParsedUploadPath> {
    if let Some(folder_id) = base_folder_id {
        verify_folder_access(state, scope, folder_id).await?;
    }

    if relative_path.split('/').any(|segment| segment.is_empty()) {
        return Err(AsterError::validation_error(
            "relative_path contains empty path segments",
        ));
    }

    let segments: Vec<&str> = relative_path.split('/').collect();
    let filename = segments
        .last()
        .ok_or_else(|| AsterError::validation_error("relative_path cannot be empty"))?;
    crate::utils::validate_name(filename)?;

    let parent_segments: Vec<String> = segments[..segments.len().saturating_sub(1)]
        .iter()
        .map(|segment| {
            crate::utils::validate_name(segment)?;
            Ok((*segment).to_string())
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(ParsedUploadPath {
        base_folder_id,
        parent_segments,
        filename: (*filename).to_string(),
    })
}

pub(crate) async fn ensure_upload_parent_path(
    state: &AppState,
    scope: WorkspaceStorageScope,
    parsed: &ParsedUploadPath,
) -> Result<Option<i64>> {
    if parsed.parent_segments.is_empty() {
        return Ok(parsed.base_folder_id);
    }

    let txn = crate::db::transaction::begin(&state.db).await?;
    let mut current_parent = parsed.base_folder_id;

    // 整条父路径在一个事务里补齐，避免目录上传时只创建出半截层级。
    for segment in &parsed.parent_segments {
        let folder = ensure_folder_in_parent(&txn, scope, current_parent, segment).await?;
        current_parent = Some(folder.id);
    }

    crate::db::transaction::commit(txn).await?;
    Ok(current_parent)
}

async fn create_file_from_blob_with_name_mode<C: ConnectionTrait>(
    db: &C,
    scope: WorkspaceStorageScope,
    folder_id: Option<i64>,
    filename: &str,
    blob: &file_blob::Model,
    now: chrono::DateTime<Utc>,
    name_mode: NewFileNameMode,
) -> Result<file::Model> {
    crate::utils::validate_name(filename)?;

    let (mut final_name, team_id) = match scope {
        WorkspaceStorageScope::Personal { user_id } => {
            let final_name = match name_mode {
                NewFileNameMode::ResolveUnique => {
                    file_repo::resolve_unique_filename(db, user_id, folder_id, filename).await?
                }
                NewFileNameMode::Exact => filename.to_string(),
            };
            (final_name, None)
        }
        WorkspaceStorageScope::Team { team_id, .. } => {
            let final_name = match name_mode {
                NewFileNameMode::ResolveUnique => {
                    file_repo::resolve_unique_team_filename(db, team_id, folder_id, filename)
                        .await?
                }
                NewFileNameMode::Exact => filename.to_string(),
            };
            (final_name, Some(team_id))
        }
    };
    let mime = mime_guess::from_path(&final_name)
        .first_or_octet_stream()
        .to_string();
    let max_attempts = match name_mode {
        NewFileNameMode::ResolveUnique => MAX_AUTO_NAME_RETRIES,
        NewFileNameMode::Exact => 1,
    };

    // `resolve_unique_*` 只能减少冲突，不能彻底消灭并发窗口。
    // 这里仍然依赖数据库唯一约束兜底，并在冲突时继续推进到下一个副本名。
    for attempt in 0..max_attempts {
        let created = file::ActiveModel {
            name: Set(final_name.clone()),
            folder_id: Set(folder_id),
            team_id: Set(team_id),
            blob_id: Set(blob.id),
            size: Set(blob.size),
            user_id: Set(scope.actor_user_id()),
            mime_type: Set(mime.clone()),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        }
        .insert(db)
        .await;

        match created {
            Ok(created) => return Ok(created),
            Err(err) if file_repo::is_name_conflict_db_err(&err) => {
                if matches!(name_mode, NewFileNameMode::Exact) {
                    return Err(file_repo::map_name_db_err(err, &final_name));
                }
                if attempt + 1 == max_attempts {
                    return Err(AsterError::validation_error(format!(
                        "failed to allocate a unique file name for '{}'",
                        filename
                    )));
                }
                final_name = crate::utils::next_copy_name(&final_name);
            }
            Err(err) => return Err(AsterError::from(err)),
        }
    }

    Err(AsterError::validation_error(format!(
        "failed to allocate a unique file name for '{}'",
        filename
    )))
}

pub(crate) async fn create_new_file_from_blob<C: ConnectionTrait>(
    db: &C,
    scope: WorkspaceStorageScope,
    folder_id: Option<i64>,
    filename: &str,
    blob: &file_blob::Model,
    now: chrono::DateTime<Utc>,
) -> Result<file::Model> {
    create_file_from_blob_with_name_mode(
        db,
        scope,
        folder_id,
        filename,
        blob,
        now,
        NewFileNameMode::ResolveUnique,
    )
    .await
}

pub(crate) async fn create_exact_file_from_blob<C: ConnectionTrait>(
    db: &C,
    scope: WorkspaceStorageScope,
    folder_id: Option<i64>,
    filename: &str,
    blob: &file_blob::Model,
    now: chrono::DateTime<Utc>,
) -> Result<file::Model> {
    create_file_from_blob_with_name_mode(
        db,
        scope,
        folder_id,
        filename,
        blob,
        now,
        NewFileNameMode::Exact,
    )
    .await
}

pub(crate) async fn check_quota<C: ConnectionTrait>(
    db: &C,
    scope: WorkspaceStorageScope,
    size: i64,
) -> Result<()> {
    match scope {
        WorkspaceStorageScope::Personal { user_id } => {
            user_repo::check_quota(db, user_id, size).await
        }
        WorkspaceStorageScope::Team { team_id, .. } => {
            team_repo::check_quota(db, team_id, size).await
        }
    }
}

pub(crate) async fn update_storage_used<C: ConnectionTrait>(
    db: &C,
    scope: WorkspaceStorageScope,
    delta: i64,
) -> Result<()> {
    match scope {
        WorkspaceStorageScope::Personal { user_id } => {
            user_repo::update_storage_used(db, user_id, delta).await
        }
        WorkspaceStorageScope::Team { team_id, .. } => {
            team_repo::update_storage_used(db, team_id, delta).await
        }
    }
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

#[allow(clippy::too_many_arguments)]
pub(crate) async fn finalize_upload_session_file(
    state: &AppState,
    session: &upload_session::Model,
    file_hash: &str,
    size: i64,
    policy_id: i64,
    storage_path: &str,
    now: chrono::DateTime<Utc>,
) -> Result<file::Model> {
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
