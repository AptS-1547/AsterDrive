//! 文件夹服务子模块：`hierarchy`。

use std::collections::{HashMap, HashSet};

use sea_orm::ConnectionTrait;

use crate::db::repository::folder_repo;
use crate::entities::folder;
use crate::errors::{AsterError, Result};
use crate::runtime::PrimaryAppState;
use crate::services::workspace_storage_service::{self, WorkspaceStorageScope};

use super::{FolderAncestorItem, ensure_folder_model_in_scope};

pub(super) async fn load_folder_chain_map<C: ConnectionTrait>(
    db: &C,
    folder_ids: &[i64],
) -> Result<HashMap<i64, folder::Model>> {
    let mut loaded = HashMap::new();
    let mut frontier: Vec<i64> = folder_ids.to_vec();

    while !frontier.is_empty() {
        frontier.retain(|id| !loaded.contains_key(id));
        frontier.sort_unstable();
        frontier.dedup();
        if frontier.is_empty() {
            break;
        }

        let rows = folder_repo::find_by_ids(db, &frontier).await?;
        let mut found = HashSet::with_capacity(rows.len());
        let mut next = Vec::new();

        for row in rows {
            found.insert(row.id);
            if let Some(pid) = row.parent_id
                && !loaded.contains_key(&pid)
            {
                next.push(pid);
            }
            loaded.insert(row.id, row);
        }

        if let Some(missing) = frontier.iter().find(|id| !found.contains(id)) {
            return Err(AsterError::record_not_found(format!("folder #{missing}")));
        }

        frontier = next;
    }

    Ok(loaded)
}

pub async fn build_folder_paths<C: ConnectionTrait>(
    db: &C,
    folder_ids: &[i64],
) -> Result<HashMap<i64, String>> {
    let chain_map = load_folder_chain_map(db, folder_ids).await?;
    let mut paths = HashMap::with_capacity(folder_ids.len());

    for &folder_id in folder_ids {
        let mut parts = Vec::new();
        let mut current_id = Some(folder_id);
        while let Some(id) = current_id {
            let folder = chain_map
                .get(&id)
                .ok_or_else(|| AsterError::record_not_found(format!("folder #{id}")))?;
            parts.push(folder.name.clone());
            current_id = folder.parent_id;
        }
        parts.reverse();
        paths.insert(folder_id, format!("/{}", parts.join("/")));
    }

    Ok(paths)
}

pub(crate) async fn get_ancestors_in_scope(
    state: &PrimaryAppState,
    scope: WorkspaceStorageScope,
    folder_id: i64,
) -> Result<Vec<FolderAncestorItem>> {
    let folder = workspace_storage_service::verify_folder_access(state, scope, folder_id).await?;
    ensure_folder_model_in_scope(&folder, scope)?;

    let ancestors = match scope {
        WorkspaceStorageScope::Personal { user_id } => {
            folder_repo::find_ancestor_models(&state.db, user_id, folder_id).await?
        }
        WorkspaceStorageScope::Team { team_id, .. } => {
            folder_repo::find_team_ancestor_models(&state.db, team_id, folder_id).await?
        }
    };

    Ok(ancestors
        .into_iter()
        .map(|folder| FolderAncestorItem {
            id: folder.id,
            name: folder.name,
        })
        .collect())
}

/// 获取文件夹的祖先链（从根下第一层到当前文件夹）
pub async fn get_ancestors(
    state: &PrimaryAppState,
    user_id: i64,
    folder_id: i64,
) -> Result<Vec<FolderAncestorItem>> {
    get_ancestors_in_scope(
        state,
        WorkspaceStorageScope::Personal { user_id },
        folder_id,
    )
    .await
}
