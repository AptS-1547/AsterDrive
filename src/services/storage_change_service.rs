//! 服务模块：`storage_change_service`。

use std::collections::{BTreeSet, HashSet};

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::runtime::AppState;
use crate::services::workspace_storage_service::WorkspaceStorageScope;

pub const STORAGE_CHANGE_CHANNEL_CAPACITY: usize = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageChangeAudience {
    User(i64),
    Team(i64),
    Any,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub enum StorageChangeKind {
    #[serde(rename = "file.created")]
    FileCreated,
    #[serde(rename = "file.updated")]
    FileUpdated,
    #[serde(rename = "file.deleted")]
    FileDeleted,
    #[serde(rename = "file.restored")]
    FileRestored,
    #[serde(rename = "folder.created")]
    FolderCreated,
    #[serde(rename = "folder.updated")]
    FolderUpdated,
    #[serde(rename = "folder.deleted")]
    FolderDeleted,
    #[serde(rename = "folder.restored")]
    FolderRestored,
    #[serde(rename = "sync.required")]
    SyncRequired,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StorageChangeWorkspace {
    Personal,
    Team { team_id: i64 },
}

#[derive(Debug, Clone, Serialize)]
pub struct StorageChangeEvent {
    #[serde(skip_serializing)]
    audience: StorageChangeAudience,
    pub kind: StorageChangeKind,
    pub workspace: Option<StorageChangeWorkspace>,
    pub file_ids: Vec<i64>,
    pub folder_ids: Vec<i64>,
    pub affected_parent_ids: Vec<i64>,
    pub root_affected: bool,
    pub at: DateTime<Utc>,
}

impl StorageChangeEvent {
    pub(crate) fn new(
        kind: StorageChangeKind,
        scope: WorkspaceStorageScope,
        file_ids: Vec<i64>,
        folder_ids: Vec<i64>,
        affected_parent_ids: Vec<Option<i64>>,
    ) -> Self {
        let (audience, workspace) = match scope {
            WorkspaceStorageScope::Personal { user_id } => (
                StorageChangeAudience::User(user_id),
                StorageChangeWorkspace::Personal,
            ),
            WorkspaceStorageScope::Team { team_id, .. } => (
                StorageChangeAudience::Team(team_id),
                StorageChangeWorkspace::Team { team_id },
            ),
        };
        let (affected_parent_ids, root_affected) =
            normalize_parent_ids(affected_parent_ids.into_iter());

        Self {
            audience,
            kind,
            workspace: Some(workspace),
            file_ids: normalize_ids(file_ids.into_iter()),
            folder_ids: normalize_ids(folder_ids.into_iter()),
            affected_parent_ids,
            root_affected,
            at: Utc::now(),
        }
    }

    pub fn sync_required() -> Self {
        Self {
            audience: StorageChangeAudience::Any,
            kind: StorageChangeKind::SyncRequired,
            workspace: None,
            file_ids: Vec::new(),
            folder_ids: Vec::new(),
            affected_parent_ids: Vec::new(),
            root_affected: false,
            at: Utc::now(),
        }
    }

    pub fn is_visible_to(&self, user_id: i64, team_ids: &HashSet<i64>) -> bool {
        match self.audience {
            StorageChangeAudience::Any => true,
            StorageChangeAudience::User(target_user_id) => target_user_id == user_id,
            StorageChangeAudience::Team(team_id) => team_ids.contains(&team_id),
        }
    }
}

pub fn publish(state: &AppState, event: StorageChangeEvent) {
    if let Err(e) = state.storage_change_tx.send(event) {
        tracing::debug!("skip storage change broadcast without listeners: {e}");
    }
}

fn normalize_ids(ids: impl Iterator<Item = i64>) -> Vec<i64> {
    BTreeSet::from_iter(ids).into_iter().collect()
}

fn normalize_parent_ids(parent_ids: impl Iterator<Item = Option<i64>>) -> (Vec<i64>, bool) {
    let mut normalized = BTreeSet::new();
    let mut root_affected = false;

    for parent_id in parent_ids {
        match parent_id {
            Some(parent_id) => {
                normalized.insert(parent_id);
            }
            None => root_affected = true,
        }
    }

    (normalized.into_iter().collect(), root_affected)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{StorageChangeEvent, StorageChangeKind, StorageChangeWorkspace};
    use crate::services::workspace_storage_service::WorkspaceStorageScope;

    #[test]
    fn storage_change_event_normalizes_ids_and_root_flag() {
        let event = StorageChangeEvent::new(
            StorageChangeKind::FileUpdated,
            WorkspaceStorageScope::Personal { user_id: 7 },
            vec![5, 3, 5],
            vec![9, 8, 9],
            vec![Some(2), None, Some(2)],
        );

        assert_eq!(event.file_ids, vec![3, 5]);
        assert_eq!(event.folder_ids, vec![8, 9]);
        assert_eq!(event.affected_parent_ids, vec![2]);
        assert!(event.root_affected);
        assert!(matches!(
            event.workspace,
            Some(StorageChangeWorkspace::Personal)
        ));
    }

    #[test]
    fn storage_change_event_filters_personal_and_team_visibility() {
        let personal = StorageChangeEvent::new(
            StorageChangeKind::FileDeleted,
            WorkspaceStorageScope::Personal { user_id: 11 },
            vec![1],
            vec![],
            vec![None],
        );
        let team = StorageChangeEvent::new(
            StorageChangeKind::FolderUpdated,
            WorkspaceStorageScope::Team {
                team_id: 42,
                actor_user_id: 11,
            },
            vec![],
            vec![7],
            vec![Some(3)],
        );

        assert!(personal.is_visible_to(11, &HashSet::new()));
        assert!(!personal.is_visible_to(12, &HashSet::new()));

        let visible_teams = HashSet::from([42]);
        assert!(team.is_visible_to(11, &visible_teams));
        assert!(!team.is_visible_to(11, &HashSet::new()));
    }
}
