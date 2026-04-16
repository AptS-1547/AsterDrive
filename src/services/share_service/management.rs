use std::collections::HashMap;

use chrono::Utc;
use sea_orm::{DatabaseConnection, Set, TransactionTrait};

use crate::api::pagination::{OffsetPage, load_offset_page};
use crate::db::repository::{file_repo, folder_repo, share_repo};
use crate::entities::share;
use crate::errors::{AsterError, Result};
use crate::runtime::AppState;
use crate::services::{
    batch_service,
    workspace_storage_service::{self, WorkspaceStorageScope},
};
use crate::utils::{hash, id};

use super::models::{
    MyShareInfo, ShareInfo, ShareTarget, ShareUpdateOutcome, share_info_from_model,
    share_target_for_share,
};
use super::shared::{
    load_share_in_scope, lock_share_resource_in_scope, remaining_downloads, resolve_share_resource,
    resolve_share_status, validate_max_downloads,
};

pub(crate) async fn create_share_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    target: ShareTarget,
    password: Option<String>,
    expires_at: Option<chrono::DateTime<Utc>>,
    max_downloads: i64,
) -> Result<ShareInfo> {
    let db = &state.db;
    let (file_id, folder_id) = target.into_ids();
    tracing::debug!(
        scope = ?scope,
        target = ?target,
        has_password = password.as_ref().is_some_and(|value| !value.is_empty()),
        has_expiry = expires_at.is_some(),
        max_downloads,
        "creating share"
    );
    workspace_storage_service::require_scope_access(state, scope).await?;

    validate_max_downloads(max_downloads)?;

    let password_hash = match password {
        Some(ref value) if !value.is_empty() => Some(hash::hash_password(value)?),
        _ => None,
    };

    let txn = db.begin().await.map_err(AsterError::from)?;
    lock_share_resource_in_scope(&txn, scope, file_id, folder_id).await?;

    let existing = match scope {
        WorkspaceStorageScope::Personal { user_id } => {
            share_repo::find_active_by_resource(&txn, user_id, file_id, folder_id).await?
        }
        WorkspaceStorageScope::Team { team_id, .. } => {
            share_repo::find_active_by_team_resource(&txn, team_id, file_id, folder_id).await?
        }
    };

    if let Some(existing) = existing {
        let is_expired = existing.expires_at.is_some_and(|exp| exp < Utc::now());
        if !is_expired {
            return Err(AsterError::validation_error(
                "an active share already exists for this resource",
            ));
        }
        share_repo::delete(&txn, existing.id).await?;
    }

    let now = Utc::now();
    let model = share::ActiveModel {
        token: Set(id::new_share_token()),
        user_id: Set(scope.actor_user_id()),
        team_id: Set(scope.team_id()),
        file_id: Set(file_id),
        folder_id: Set(folder_id),
        password: Set(password_hash),
        expires_at: Set(expires_at),
        max_downloads: Set(max_downloads),
        download_count: Set(0),
        view_count: Set(0),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    };
    let created = share_repo::create(&txn, model).await?;
    txn.commit().await.map_err(AsterError::from)?;
    tracing::debug!(
        scope = ?scope,
        share_id = created.id,
        target = ?target,
        "created share"
    );
    share_info_from_model(created)
}

pub async fn create_share(
    state: &AppState,
    user_id: i64,
    target: ShareTarget,
    password: Option<String>,
    expires_at: Option<chrono::DateTime<Utc>>,
    max_downloads: i64,
) -> Result<ShareInfo> {
    create_share_in_scope(
        state,
        WorkspaceStorageScope::Personal { user_id },
        target,
        password,
        expires_at,
        max_downloads,
    )
    .await
}

pub(crate) async fn list_shares_paginated_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    limit: u64,
    offset: u64,
) -> Result<OffsetPage<MyShareInfo>> {
    tracing::debug!(
        scope = ?scope,
        limit,
        offset,
        "listing paginated shares"
    );
    workspace_storage_service::require_scope_access(state, scope).await?;
    let page = load_offset_page(limit, offset, 100, |limit, offset| async move {
        let (shares, total) = match scope {
            WorkspaceStorageScope::Personal { user_id } => {
                share_repo::find_by_user_paginated(&state.db, user_id, limit, offset).await?
            }
            WorkspaceStorageScope::Team { team_id, .. } => {
                share_repo::find_by_team_paginated(&state.db, team_id, limit, offset).await?
            }
        };
        let items = build_my_share_infos(&state.db, shares).await?;
        Ok((items, total))
    })
    .await?;
    tracing::debug!(
        scope = ?scope,
        total = page.total,
        returned = page.items.len(),
        limit = page.limit,
        offset = page.offset,
        "listed paginated shares"
    );
    Ok(page)
}

pub(crate) async fn delete_share_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    share_id: i64,
) -> Result<()> {
    tracing::debug!(scope = ?scope, share_id, "deleting share");
    load_share_in_scope(state, scope, share_id).await?;
    share_repo::delete(&state.db, share_id).await?;
    tracing::debug!(scope = ?scope, share_id, "deleted share");
    Ok(())
}

pub(crate) async fn update_share_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    share_id: i64,
    password: Option<String>,
    expires_at: Option<chrono::DateTime<Utc>>,
    max_downloads: i64,
) -> Result<ShareUpdateOutcome> {
    tracing::debug!(
        scope = ?scope,
        share_id,
        update_password = password.is_some(),
        has_expiry = expires_at.is_some(),
        max_downloads,
        "updating share"
    );
    validate_max_downloads(max_downloads)?;

    let existing = load_share_in_scope(state, scope, share_id).await?;
    let has_password = match password.as_deref() {
        Some(value) => !value.is_empty(),
        None => existing.password.is_some(),
    };
    let mut active: share::ActiveModel = existing.into();

    if let Some(password) = password {
        active.password = if password.is_empty() {
            Set(None)
        } else {
            Set(Some(hash::hash_password(&password)?))
        };
    }

    active.expires_at = Set(expires_at);
    active.max_downloads = Set(max_downloads);
    active.updated_at = Set(Utc::now());

    let updated: ShareInfo = share_repo::update(&state.db, active)
        .await
        .and_then(share_info_from_model)?;
    tracing::debug!(
        scope = ?scope,
        share_id = updated.id,
        max_downloads = updated.max_downloads,
        has_expiry = updated.expires_at.is_some(),
        "updated share"
    );
    Ok(ShareUpdateOutcome {
        share: updated,
        has_password,
    })
}

pub(crate) async fn batch_delete_shares_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    share_ids: &[i64],
) -> Result<batch_service::BatchResult> {
    tracing::debug!(
        scope = ?scope,
        share_count = share_ids.len(),
        "batch deleting shares"
    );
    workspace_storage_service::require_scope_access(state, scope).await?;
    let mut result = batch_service::BatchResult {
        succeeded: 0,
        failed: 0,
        errors: vec![],
    };

    let scoped_shares = match scope {
        WorkspaceStorageScope::Personal { user_id } => {
            share_repo::find_by_ids_in_personal_scope(&state.db, user_id, share_ids).await?
        }
        WorkspaceStorageScope::Team { team_id, .. } => {
            share_repo::find_by_ids_in_team_scope(&state.db, team_id, share_ids).await?
        }
    };
    let share_map: HashMap<i64, share::Model> = scoped_shares
        .into_iter()
        .map(|share| (share.id, share))
        .collect();
    let mut ids_to_delete = Vec::new();
    let mut deleted_once = std::collections::HashSet::new();

    for &id in share_ids {
        if share_map.contains_key(&id) && deleted_once.insert(id) {
            result.succeeded += 1;
            ids_to_delete.push(id);
        } else {
            result.failed += 1;
            result.errors.push(batch_service::BatchItemError {
                entity_type: "share".to_string(),
                entity_id: id,
                error: AsterError::share_not_found(format!("share #{id}")).to_string(),
            });
        }
    }

    if !ids_to_delete.is_empty() {
        let txn = state.db.begin().await.map_err(AsterError::from)?;
        share_repo::delete_many(&txn, &ids_to_delete).await?;
        txn.commit().await.map_err(AsterError::from)?;
    }

    tracing::debug!(
        scope = ?scope,
        succeeded = result.succeeded,
        failed = result.failed,
        "batch deleted shares"
    );
    Ok(result)
}

pub async fn list_my_shares_paginated(
    state: &AppState,
    user_id: i64,
    limit: u64,
    offset: u64,
) -> Result<OffsetPage<MyShareInfo>> {
    list_shares_paginated_in_scope(
        state,
        WorkspaceStorageScope::Personal { user_id },
        limit,
        offset,
    )
    .await
}

pub async fn list_team_shares_paginated(
    state: &AppState,
    team_id: i64,
    user_id: i64,
    limit: u64,
    offset: u64,
) -> Result<OffsetPage<MyShareInfo>> {
    list_shares_paginated_in_scope(
        state,
        WorkspaceStorageScope::Team {
            team_id,
            actor_user_id: user_id,
        },
        limit,
        offset,
    )
    .await
}

pub async fn delete_share(state: &AppState, share_id: i64, user_id: i64) -> Result<()> {
    delete_share_in_scope(state, WorkspaceStorageScope::Personal { user_id }, share_id).await
}

pub async fn delete_team_share(
    state: &AppState,
    team_id: i64,
    share_id: i64,
    user_id: i64,
) -> Result<()> {
    delete_share_in_scope(
        state,
        WorkspaceStorageScope::Team {
            team_id,
            actor_user_id: user_id,
        },
        share_id,
    )
    .await
}

pub async fn update_share(
    state: &AppState,
    share_id: i64,
    user_id: i64,
    password: Option<String>,
    expires_at: Option<chrono::DateTime<Utc>>,
    max_downloads: i64,
) -> Result<ShareInfo> {
    update_share_in_scope(
        state,
        WorkspaceStorageScope::Personal { user_id },
        share_id,
        password,
        expires_at,
        max_downloads,
    )
    .await
    .map(|outcome| outcome.share)
}

pub async fn update_team_share(
    state: &AppState,
    team_id: i64,
    share_id: i64,
    user_id: i64,
    password: Option<String>,
    expires_at: Option<chrono::DateTime<Utc>>,
    max_downloads: i64,
) -> Result<ShareInfo> {
    update_share_in_scope(
        state,
        WorkspaceStorageScope::Team {
            team_id,
            actor_user_id: user_id,
        },
        share_id,
        password,
        expires_at,
        max_downloads,
    )
    .await
    .map(|outcome| outcome.share)
}

pub fn validate_batch_share_ids(share_ids: &[i64]) -> Result<()> {
    if share_ids.is_empty() {
        return Err(AsterError::validation_error(
            "at least one share ID is required",
        ));
    }
    if share_ids.len() > batch_service::MAX_BATCH_ITEMS {
        return Err(AsterError::validation_error(format!(
            "batch size cannot exceed {} items",
            batch_service::MAX_BATCH_ITEMS
        )));
    }
    Ok(())
}

pub async fn batch_delete_shares(
    state: &AppState,
    user_id: i64,
    share_ids: &[i64],
) -> Result<batch_service::BatchResult> {
    validate_batch_share_ids(share_ids)?;
    batch_delete_shares_in_scope(
        state,
        WorkspaceStorageScope::Personal { user_id },
        share_ids,
    )
    .await
}

pub async fn batch_delete_team_shares(
    state: &AppState,
    team_id: i64,
    user_id: i64,
    share_ids: &[i64],
) -> Result<batch_service::BatchResult> {
    validate_batch_share_ids(share_ids)?;
    batch_delete_shares_in_scope(
        state,
        WorkspaceStorageScope::Team {
            team_id,
            actor_user_id: user_id,
        },
        share_ids,
    )
    .await
}

pub async fn list_paginated(
    state: &AppState,
    limit: u64,
    offset: u64,
) -> Result<OffsetPage<ShareInfo>> {
    load_offset_page(limit, offset, 100, |limit, offset| async move {
        let (items, total) = share_repo::find_paginated(&state.db, limit, offset).await?;
        let items = items
            .into_iter()
            .map(share_info_from_model)
            .collect::<Result<Vec<_>>>()?;
        Ok((items, total))
    })
    .await
}

pub async fn admin_delete_share(state: &AppState, share_id: i64) -> Result<()> {
    share_repo::find_by_id(&state.db, share_id).await?;
    share_repo::delete(&state.db, share_id).await
}

async fn build_my_share_infos(
    db: &DatabaseConnection,
    shares: Vec<share::Model>,
) -> Result<Vec<MyShareInfo>> {
    let mut file_ids = Vec::new();
    let mut folder_ids = Vec::new();
    for share in &shares {
        match share_target_for_share(share)? {
            ShareTarget {
                r#type: crate::types::EntityType::File,
                id,
            } => file_ids.push(id),
            ShareTarget {
                r#type: crate::types::EntityType::Folder,
                id,
            } => folder_ids.push(id),
        }
    }

    let files = file_repo::find_by_ids(db, &file_ids).await?;
    let folders = folder_repo::find_by_ids(db, &folder_ids).await?;

    let file_map: HashMap<i64, crate::entities::file::Model> =
        files.into_iter().map(|file| (file.id, file)).collect();
    let folder_map: HashMap<i64, crate::entities::folder::Model> = folders
        .into_iter()
        .map(|folder| (folder.id, folder))
        .collect();

    let mut items = Vec::with_capacity(shares.len());
    for share in shares {
        let (resource_id, resource_name, resource_type, resource_deleted) =
            resolve_share_resource(&share, &file_map, &folder_map)?;
        let status = resolve_share_status(&share, resource_deleted);
        let remaining_downloads = remaining_downloads(share.max_downloads, share.download_count);

        items.push(MyShareInfo {
            id: share.id,
            token: share.token,
            resource_id,
            resource_name,
            resource_type,
            resource_deleted,
            has_password: share.password.is_some(),
            status,
            expires_at: share.expires_at,
            max_downloads: share.max_downloads,
            download_count: share.download_count,
            view_count: share.view_count,
            remaining_downloads,
            created_at: share.created_at,
            updated_at: share.updated_at,
        });
    }

    Ok(items)
}
