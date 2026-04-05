use actix_multipart::Multipart;
use chrono::Utc;
use futures::StreamExt;
use sea_orm::{ActiveModelTrait, ConnectionTrait, Set, TransactionTrait};
use tokio::io::AsyncWriteExt;

use crate::db::repository::{
    file_repo, folder_repo, team_member_repo, team_repo, upload_session_repo, user_repo,
};
use crate::entities::{file, file_blob, folder, team, upload_session};
use crate::errors::{AsterError, MapAsterErr, Result};
use crate::runtime::AppState;
use crate::types::{
    DriverType, S3UploadStrategy, UploadSessionStatus, effective_s3_multipart_chunk_size,
    parse_storage_policy_options,
};

const HASH_BUF_SIZE: usize = 65536;

#[derive(Clone, Copy, Debug)]
pub(crate) enum WorkspaceStorageScope {
    Personal { user_id: i64 },
    Team { team_id: i64, actor_user_id: i64 },
}

impl WorkspaceStorageScope {
    pub(crate) fn actor_user_id(self) -> i64 {
        match self {
            Self::Personal { user_id } => user_id,
            Self::Team { actor_user_id, .. } => actor_user_id,
        }
    }

    pub(crate) fn team_id(self) -> Option<i64> {
        match self {
            Self::Personal { .. } => None,
            Self::Team { team_id, .. } => Some(team_id),
        }
    }
}

pub(crate) async fn require_scope_access(
    state: &AppState,
    scope: WorkspaceStorageScope,
) -> Result<()> {
    if let WorkspaceStorageScope::Team {
        team_id,
        actor_user_id,
    } = scope
    {
        require_team_access(state, team_id, actor_user_id).await?;
    }

    Ok(())
}

pub(crate) fn ensure_personal_file_scope(file: &file::Model) -> Result<()> {
    if file.team_id.is_some() {
        return Err(AsterError::auth_forbidden(
            "file belongs to a team workspace",
        ));
    }
    Ok(())
}

pub(crate) fn ensure_personal_folder_scope(folder: &folder::Model) -> Result<()> {
    if folder.team_id.is_some() {
        return Err(AsterError::auth_forbidden(
            "folder belongs to a team workspace",
        ));
    }
    Ok(())
}

pub(crate) fn ensure_file_scope(file: &file::Model, scope: WorkspaceStorageScope) -> Result<()> {
    match scope {
        WorkspaceStorageScope::Personal { user_id } => {
            ensure_personal_file_scope(file)?;
            crate::utils::verify_owner(file.user_id, user_id, "file")?;
        }
        WorkspaceStorageScope::Team { team_id, .. } => {
            if file.team_id != Some(team_id) {
                return Err(AsterError::auth_forbidden("file is outside team workspace"));
            }
        }
    }

    Ok(())
}

pub(crate) fn ensure_active_file_scope(
    file: &file::Model,
    scope: WorkspaceStorageScope,
) -> Result<()> {
    ensure_file_scope(file, scope)?;

    if file.deleted_at.is_some() {
        return Err(AsterError::file_not_found(format!(
            "file #{} is in trash",
            file.id
        )));
    }

    Ok(())
}

pub(crate) fn ensure_folder_scope(
    folder: &folder::Model,
    scope: WorkspaceStorageScope,
) -> Result<()> {
    match scope {
        WorkspaceStorageScope::Personal { user_id } => {
            ensure_personal_folder_scope(folder)?;
            crate::utils::verify_owner(folder.user_id, user_id, "folder")?;
        }
        WorkspaceStorageScope::Team { team_id, .. } => {
            if folder.team_id != Some(team_id) {
                return Err(AsterError::auth_forbidden(
                    "folder is outside team workspace",
                ));
            }
        }
    }

    Ok(())
}

pub(crate) fn ensure_active_folder_scope(
    folder: &folder::Model,
    scope: WorkspaceStorageScope,
) -> Result<()> {
    ensure_folder_scope(folder, scope)?;

    if folder.deleted_at.is_some() {
        return Err(AsterError::file_not_found(format!(
            "folder #{} is in trash",
            folder.id
        )));
    }

    Ok(())
}

pub(crate) async fn require_team_access(
    state: &AppState,
    team_id: i64,
    user_id: i64,
) -> Result<team::Model> {
    let team = team_repo::find_active_by_id(&state.db, team_id).await?;
    if team_member_repo::find_by_team_and_user(&state.db, team_id, user_id)
        .await?
        .is_none()
    {
        return Err(AsterError::auth_forbidden("not a member of this team"));
    }
    Ok(team)
}

pub(crate) async fn require_team_management_access(
    state: &AppState,
    team_id: i64,
    user_id: i64,
) -> Result<team::Model> {
    let team = team_repo::find_active_by_id(&state.db, team_id).await?;
    let membership = team_member_repo::find_by_team_and_user(&state.db, team_id, user_id)
        .await?
        .ok_or_else(|| AsterError::auth_forbidden("not a member of this team"))?;
    if !membership.role.can_manage_team() {
        return Err(AsterError::auth_forbidden(
            "team owner or admin role is required",
        ));
    }
    Ok(team)
}

pub(crate) async fn verify_folder_access(
    state: &AppState,
    scope: WorkspaceStorageScope,
    folder_id: i64,
) -> Result<folder::Model> {
    require_scope_access(state, scope).await?;
    let folder = folder_repo::find_by_id(&state.db, folder_id).await?;
    ensure_active_folder_scope(&folder, scope)?;

    Ok(folder)
}

pub(crate) async fn verify_file_access(
    state: &AppState,
    scope: WorkspaceStorageScope,
    file_id: i64,
) -> Result<file::Model> {
    require_scope_access(state, scope).await?;
    let file = file_repo::find_by_id(&state.db, file_id).await?;
    ensure_active_file_scope(&file, scope)?;

    Ok(file)
}

pub(crate) async fn list_files_in_folder(
    state: &AppState,
    scope: WorkspaceStorageScope,
    folder_id: Option<i64>,
) -> Result<Vec<file::Model>> {
    match scope {
        WorkspaceStorageScope::Personal { user_id } => {
            file_repo::find_by_folder(&state.db, user_id, folder_id).await
        }
        WorkspaceStorageScope::Team { team_id, .. } => {
            file_repo::find_by_team_folder(&state.db, team_id, folder_id).await
        }
    }
}

pub(crate) async fn list_folders_in_parent(
    state: &AppState,
    scope: WorkspaceStorageScope,
    parent_id: Option<i64>,
) -> Result<Vec<folder::Model>> {
    match scope {
        WorkspaceStorageScope::Personal { user_id } => {
            folder_repo::find_children(&state.db, user_id, parent_id).await
        }
        WorkspaceStorageScope::Team { team_id, .. } => {
            folder_repo::find_team_children(&state.db, team_id, parent_id).await
        }
    }
}

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
    policy.driver_type == crate::types::DriverType::Local
        && parse_storage_policy_options(&policy.options)
            .content_dedup
            .unwrap_or(false)
}

fn relay_stream_direct_upload_eligible(
    policy: &crate::entities::storage_policy::Model,
    declared_size: i64,
) -> bool {
    if declared_size <= 0 || policy.driver_type != DriverType::S3 {
        return false;
    }

    let options = parse_storage_policy_options(&policy.options);
    if options.effective_s3_upload_strategy() != S3UploadStrategy::RelayStream {
        return false;
    }

    policy.chunk_size == 0 || declared_size <= effective_s3_multipart_chunk_size(policy.chunk_size)
}

pub(crate) async fn create_nondedup_blob<C: ConnectionTrait>(
    db: &C,
    size: i64,
    policy_id: i64,
) -> Result<file_blob::Model> {
    let blob_key = crate::utils::id::new_short_token();
    let storage_path = crate::utils::storage_path_from_blob_key(&blob_key);
    let now = Utc::now();

    file_repo::create_blob(
        db,
        file_blob::ActiveModel {
            hash: Set(blob_key),
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
    let session_fresh = upload_session_repo::find_by_id(db, session_id).await?;
    let mut active: upload_session::ActiveModel = session_fresh.into();
    active.status = Set(UploadSessionStatus::Completed);
    active.file_id = Set(Some(file_id));
    active.updated_at = Set(Utc::now());
    upload_session_repo::update(db, active).await?;
    Ok(())
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

pub(crate) async fn resolve_upload_path(
    state: &AppState,
    scope: WorkspaceStorageScope,
    base_folder_id: Option<i64>,
    relative_path: &str,
) -> Result<(Option<i64>, String)> {
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

    if segments.len() == 1 {
        return Ok((base_folder_id, (*filename).to_string()));
    }

    let txn = state.db.begin().await.map_err(AsterError::from)?;
    let mut current_parent = base_folder_id;

    for segment in &segments[..segments.len() - 1] {
        let folder = ensure_folder_in_parent(&txn, scope, current_parent, segment).await?;
        current_parent = Some(folder.id);
    }

    txn.commit().await.map_err(AsterError::from)?;
    Ok((current_parent, (*filename).to_string()))
}

pub(crate) async fn create_new_file_from_blob<C: ConnectionTrait>(
    db: &C,
    scope: WorkspaceStorageScope,
    folder_id: Option<i64>,
    filename: &str,
    blob: &file_blob::Model,
    now: chrono::DateTime<Utc>,
) -> Result<file::Model> {
    let (final_name, team_id) = match scope {
        WorkspaceStorageScope::Personal { user_id } => (
            file_repo::resolve_unique_filename(db, user_id, folder_id, filename).await?,
            None,
        ),
        WorkspaceStorageScope::Team { team_id, .. } => (
            file_repo::resolve_unique_team_filename(db, team_id, folder_id, filename).await?,
            Some(team_id),
        ),
    };
    let mime = mime_guess::from_path(&final_name)
        .first_or_octet_stream()
        .to_string();

    file_repo::create(
        db,
        file::ActiveModel {
            name: Set(final_name),
            folder_id: Set(folder_id),
            team_id: Set(team_id),
            blob_id: Set(blob.id),
            size: Set(blob.size),
            user_id: Set(scope.actor_user_id()),
            mime_type: Set(mime),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        },
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
    let txn = state.db.begin().await.map_err(AsterError::from)?;
    check_quota(&txn, scope, size).await?;

    let blob =
        file_repo::find_or_create_blob(&txn, file_hash, size, policy_id, storage_path).await?;
    let created = finalize_upload_session_blob(&txn, session, &blob.model, now).await?;

    txn.commit().await.map_err(AsterError::from)?;
    Ok(created)
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn store_from_temp(
    state: &AppState,
    scope: WorkspaceStorageScope,
    folder_id: Option<i64>,
    filename: &str,
    temp_path: &str,
    size: i64,
    existing_file_id: Option<i64>,
    skip_lock_check: bool,
) -> Result<file::Model> {
    let db = &state.db;

    crate::utils::validate_name(filename)?;

    let policy = resolve_policy_for_size(state, scope, folder_id, size).await?;
    let should_dedup = local_content_dedup_enabled(&policy);

    if policy.max_file_size > 0 && size > policy.max_file_size {
        return Err(AsterError::file_too_large(format!(
            "file size {} exceeds limit {}",
            size, policy.max_file_size
        )));
    }

    check_quota(db, scope, size).await?;

    let now = Utc::now();
    let driver = state.driver_registry.get_driver(&policy)?;

    let dedup_target = if should_dedup {
        use sha2::{Digest, Sha256};
        use tokio::io::AsyncReadExt;

        let mut hasher = Sha256::new();
        let mut reader = tokio::fs::File::open(temp_path)
            .await
            .map_aster_err_ctx("open temp", AsterError::file_upload_failed)?;
        let mut buf = vec![0u8; HASH_BUF_SIZE];
        loop {
            let n = reader
                .read(&mut buf)
                .await
                .map_aster_err_ctx("read temp", AsterError::file_upload_failed)?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }
        let file_hash = crate::utils::hash::sha256_digest_to_hex(&hasher.finalize());
        let storage_path = crate::utils::storage_path_from_hash(&file_hash);
        Some((file_hash, storage_path))
    } else {
        None
    };

    let overwrite_ctx = if let Some(existing_id) = existing_file_id {
        let old_file = verify_file_access(state, scope, existing_id).await?;
        if old_file.is_locked && !skip_lock_check {
            return Err(AsterError::resource_locked("file is locked"));
        }
        let old_blob = file_repo::find_blob_by_id(db, old_file.blob_id).await?;
        if let Err(err) =
            crate::services::thumbnail_service::delete_thumbnail(state, &old_blob).await
        {
            tracing::warn!("failed to delete thumbnail for blob {}: {err}", old_blob.id);
        }
        Some((old_file, old_blob))
    } else {
        None
    };

    let mime = mime_guess::from_path(filename)
        .first_or_octet_stream()
        .to_string();

    let txn = state.db.begin().await.map_err(AsterError::from)?;
    check_quota(&txn, scope, size).await?;

    let blob = if let Some((file_hash, storage_path)) = dedup_target.as_ref() {
        let blob =
            file_repo::find_or_create_blob(&txn, file_hash, size, policy.id, storage_path).await?;
        if blob.inserted {
            driver.put_file(storage_path, temp_path).await?;
        }
        blob.model
    } else if policy.driver_type == crate::types::DriverType::S3 {
        let upload_id = crate::utils::id::new_uuid();
        let blob = create_s3_nondedup_blob(&txn, size, policy.id, &upload_id).await?;
        driver.put_file(&blob.storage_path, temp_path).await?;
        blob
    } else {
        let blob = create_nondedup_blob(&txn, size, policy.id).await?;
        driver.put_file(&blob.storage_path, temp_path).await?;
        blob
    };

    let result = if let Some((old_file, old_blob)) = overwrite_ctx {
        let existing_id = old_file.id;
        let mut active: file::ActiveModel = old_file.into();
        active.blob_id = Set(blob.id);
        active.size = Set(blob.size);
        active.mime_type = Set(mime);
        active.updated_at = Set(now);
        let updated = active.update(&txn).await.map_err(AsterError::from)?;

        let next_ver = crate::db::repository::version_repo::next_version(&txn, existing_id).await?;
        crate::db::repository::version_repo::create(
            &txn,
            crate::entities::file_version::ActiveModel {
                file_id: Set(existing_id),
                blob_id: Set(old_blob.id),
                version: Set(next_ver),
                size: Set(old_blob.size),
                created_at: Set(now),
                ..Default::default()
            },
        )
        .await?;

        update_storage_used(&txn, scope, size).await?;
        updated
    } else {
        let created =
            create_new_file_from_blob(&txn, scope, folder_id, filename, &blob, now).await?;
        update_storage_used(&txn, scope, size).await?;
        created
    };

    txn.commit().await.map_err(AsterError::from)?;

    if let Some(existing_id) = existing_file_id {
        crate::services::version_service::cleanup_excess(state, existing_id).await?;
    }

    Ok(result)
}

async fn upload_s3_relay_direct(
    state: &AppState,
    scope: WorkspaceStorageScope,
    payload: &mut Multipart,
    folder_id: Option<i64>,
    relative_path: Option<&str>,
    resolved_filename: &str,
    policy: &crate::entities::storage_policy::Model,
    declared_size: i64,
) -> Result<file::Model> {
    const RELAY_DIRECT_BUFFER_SIZE: usize = 64 * 1024;

    if policy.max_file_size > 0 && declared_size > policy.max_file_size {
        return Err(AsterError::file_too_large(format!(
            "file size {} exceeds limit {}",
            declared_size, policy.max_file_size
        )));
    }

    check_quota(&state.db, scope, declared_size).await?;
    let driver = state.driver_registry.get_driver(policy)?;

    while let Some(field) = payload.next().await {
        let mut field = field.map_aster_err(AsterError::file_upload_failed)?;
        let is_file = field
            .content_disposition()
            .and_then(|content| content.get_filename().map(|name| name.to_string()));

        if let Some(name) = is_file {
            let filename = if relative_path.is_some() {
                resolved_filename.to_string()
            } else {
                name
            };
            crate::utils::validate_name(&filename)?;

            let upload_id = crate::utils::id::new_uuid();
            let storage_path = format!("files/{upload_id}");
            let (writer, reader) = tokio::io::duplex(RELAY_DIRECT_BUFFER_SIZE);
            let upload_driver = driver.clone();
            let upload_storage_path = storage_path.clone();
            let (upload_result, relay_result) = tokio::task::LocalSet::new()
                .run_until(async move {
                    let relay_task = tokio::task::spawn_local(async move {
                        let mut writer = writer;
                        while let Some(chunk) = field.next().await {
                            let chunk = chunk.map_aster_err(AsterError::file_upload_failed)?;
                            writer.write_all(&chunk).await.map_aster_err_ctx(
                                "relay direct write",
                                AsterError::file_upload_failed,
                            )?;
                        }
                        writer.shutdown().await.map_aster_err_ctx(
                            "relay direct shutdown",
                            AsterError::file_upload_failed,
                        )?;
                        Ok::<(), AsterError>(())
                    });

                    let upload_result = upload_driver
                        .put_reader(&upload_storage_path, Box::new(reader), declared_size)
                        .await;
                    let relay_result = relay_task.await.map_err(|err| {
                        AsterError::file_upload_failed(format!("relay direct task failed: {err}"))
                    })?;

                    Ok::<(Result<String>, Result<()>), AsterError>((upload_result, relay_result))
                })
                .await?;

            if let Err(err) = upload_result {
                if let Err(cleanup_err) = driver.delete(&storage_path).await {
                    tracing::warn!(
                        "failed to cleanup relay direct object {} after upload error: {cleanup_err}",
                        storage_path
                    );
                }
                return Err(err);
            }

            if let Err(err) = relay_result {
                if let Err(cleanup_err) = driver.delete(&storage_path).await {
                    tracing::warn!(
                        "failed to cleanup relay direct object {} after relay error: {cleanup_err}",
                        storage_path
                    );
                }
                return Err(err);
            }

            let now = Utc::now();
            let txn = state.db.begin().await.map_err(AsterError::from)?;
            let create_result = async {
                check_quota(&txn, scope, declared_size).await?;
                let blob =
                    create_s3_nondedup_blob(&txn, declared_size, policy.id, &upload_id).await?;
                let created =
                    create_new_file_from_blob(&txn, scope, folder_id, &filename, &blob, now)
                        .await?;
                update_storage_used(&txn, scope, declared_size).await?;
                txn.commit().await.map_err(AsterError::from)?;
                Ok::<file::Model, AsterError>(created)
            }
            .await;

            return match create_result {
                Ok(file) => Ok(file),
                Err(err) => {
                    if let Err(cleanup_err) = driver.delete(&storage_path).await {
                        tracing::warn!(
                            "failed to cleanup relay direct object {} after DB error: {cleanup_err}",
                            storage_path
                        );
                    }
                    Err(err)
                }
            };
        }
    }

    Err(AsterError::validation_error("empty file"))
}

pub(crate) async fn upload(
    state: &AppState,
    scope: WorkspaceStorageScope,
    payload: &mut Multipart,
    folder_id: Option<i64>,
    relative_path: Option<&str>,
    declared_size: Option<i64>,
) -> Result<file::Model> {
    if let Some(declared_size) = declared_size
        && declared_size < 0
    {
        return Err(AsterError::validation_error(
            "declared_size cannot be negative",
        ));
    }

    let (resolved_folder_id, resolved_filename) = match relative_path {
        Some(path) => resolve_upload_path(state, scope, folder_id, path).await?,
        None => {
            if let Some(folder_id) = folder_id {
                verify_folder_access(state, scope, folder_id).await?;
            }
            (folder_id, String::new())
        }
    };

    let effective_folder_id = if relative_path.is_some() {
        resolved_folder_id
    } else {
        folder_id
    };

    // relay_stream 的真正无暂存 fast path 需要先知道文件大小，避免在未解析策略前就开始写远端对象。
    if let Some(declared_size) = declared_size {
        let policy =
            resolve_policy_for_size(state, scope, effective_folder_id, declared_size).await?;
        if relay_stream_direct_upload_eligible(&policy, declared_size) {
            return upload_s3_relay_direct(
                state,
                scope,
                payload,
                effective_folder_id,
                relative_path,
                &resolved_filename,
                &policy,
                declared_size,
            )
            .await;
        }
    }

    let mut filename = String::from("unnamed");
    let temp_dir = &state.config.server.temp_dir;
    let temp_path =
        crate::utils::paths::temp_file_path(temp_dir, &uuid::Uuid::new_v4().to_string());
    tokio::fs::create_dir_all(temp_dir)
        .await
        .map_aster_err_ctx("create temp dir", AsterError::file_upload_failed)?;

    let mut temp_file = tokio::fs::File::create(&temp_path)
        .await
        .map_aster_err_ctx("create temp", AsterError::file_upload_failed)?;
    let mut size: i64 = 0;

    while let Some(field) = payload.next().await {
        let mut field = field.map_aster_err(AsterError::file_upload_failed)?;
        let is_file = field
            .content_disposition()
            .and_then(|content| content.get_filename().map(|name| name.to_string()));

        if let Some(name) = is_file {
            filename = if relative_path.is_some() {
                resolved_filename.clone()
            } else {
                name
            };

            while let Some(chunk) = field.next().await {
                let chunk = chunk.map_aster_err(AsterError::file_upload_failed)?;
                temp_file
                    .write_all(&chunk)
                    .await
                    .map_aster_err_ctx("write temp", AsterError::file_upload_failed)?;
                size += chunk.len() as i64;
            }
            break;
        }
    }

    temp_file
        .flush()
        .await
        .map_aster_err_ctx("flush temp", AsterError::file_upload_failed)?;
    drop(temp_file);

    if size == 0 {
        crate::utils::cleanup_temp_file(&temp_path).await;
        return Err(AsterError::validation_error("empty file"));
    }

    let result = store_from_temp(
        state,
        scope,
        effective_folder_id,
        &filename,
        &temp_path,
        size,
        None,
        false,
    )
    .await;

    crate::utils::cleanup_temp_file(&temp_path).await;
    result
}

pub(crate) async fn create_empty(
    state: &AppState,
    scope: WorkspaceStorageScope,
    folder_id: Option<i64>,
    filename: &str,
) -> Result<file::Model> {
    if let Some(folder_id) = folder_id {
        verify_folder_access(state, scope, folder_id).await?;
    }
    crate::utils::validate_name(filename)?;

    const EMPTY_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    const EMPTY_SIZE: i64 = 0;

    let policy = resolve_policy_for_size(state, scope, folder_id, EMPTY_SIZE).await?;
    let driver = state.driver_registry.get_driver(&policy)?;
    let should_dedup = local_content_dedup_enabled(&policy);
    let now = Utc::now();

    let txn = state.db.begin().await.map_err(AsterError::from)?;
    let blob = if should_dedup {
        let storage_path = crate::utils::storage_path_from_hash(EMPTY_SHA256);
        let blob = file_repo::find_or_create_blob(
            &txn,
            EMPTY_SHA256,
            EMPTY_SIZE,
            policy.id,
            &storage_path,
        )
        .await?;
        if blob.inserted {
            driver.put(&storage_path, &[]).await?;
        }
        blob.model
    } else if policy.driver_type == crate::types::DriverType::S3 {
        let upload_id = crate::utils::id::new_uuid();
        let blob = create_s3_nondedup_blob(&txn, EMPTY_SIZE, policy.id, &upload_id).await?;
        driver.put(&blob.storage_path, &[]).await?;
        blob
    } else {
        let blob = create_nondedup_blob(&txn, EMPTY_SIZE, policy.id).await?;
        driver.put(&blob.storage_path, &[]).await?;
        blob
    };

    let created = create_new_file_from_blob(&txn, scope, folder_id, filename, &blob, now).await?;
    txn.commit().await.map_err(AsterError::from)?;
    Ok(created)
}
