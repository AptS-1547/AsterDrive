use chrono::Utc;
use sea_orm::{ActiveModelTrait, Set, TransactionSession, TransactionTrait};

use crate::api::pagination::{OffsetPage, load_offset_page};
use crate::db::repository::{policy_group_repo, policy_repo, team_repo, user_repo};
use crate::entities::{storage_policy_group, storage_policy_group_item, user};
use crate::errors::{AsterError, Result};
use crate::runtime::AppState;

use super::models::{
    CreateStoragePolicyGroupInput, PolicyGroupUserMigrationResult, StoragePolicyGroupInfo,
    UpdateStoragePolicyGroupInput,
};
use super::shared::{
    build_group_info, format_group_assignment_blocker, lock_default_group_assignment,
    replace_group_items, validate_group_items,
};

pub async fn ensure_policy_groups_seeded<C>(db: &C) -> Result<()>
where
    C: sea_orm::ConnectionTrait + TransactionTrait,
{
    let default_policy = match policy_repo::find_default(db).await? {
        Some(policy) => policy,
        None => return Ok(()),
    };

    let txn = db.begin().await.map_err(AsterError::from)?;
    let result = async {
        let default_group = match policy_group_repo::find_default_group(&txn).await? {
            Some(group) => {
                let items = policy_group_repo::find_group_items(&txn, group.id).await?;
                if items.is_empty() {
                    policy_group_repo::create_group_item(
                        &txn,
                        storage_policy_group_item::ActiveModel {
                            group_id: Set(group.id),
                            policy_id: Set(default_policy.id),
                            priority: Set(1),
                            min_file_size: Set(0),
                            max_file_size: Set(0),
                            created_at: Set(Utc::now()),
                            ..Default::default()
                        },
                    )
                    .await?;
                }
                group
            }
            None => {
                let now = Utc::now();
                let group = policy_group_repo::create_group(
                    &txn,
                    storage_policy_group::ActiveModel {
                        name: Set("Default Policy Group".to_string()),
                        description: Set(
                            "System default storage policy group created automatically".to_string(),
                        ),
                        is_enabled: Set(true),
                        is_default: Set(false),
                        created_at: Set(now),
                        updated_at: Set(now),
                        ..Default::default()
                    },
                )
                .await?;
                policy_group_repo::create_group_item(
                    &txn,
                    storage_policy_group_item::ActiveModel {
                        group_id: Set(group.id),
                        policy_id: Set(default_policy.id),
                        priority: Set(1),
                        min_file_size: Set(0),
                        max_file_size: Set(0),
                        created_at: Set(now),
                        ..Default::default()
                    },
                )
                .await?;
                group
            }
        };
        lock_default_group_assignment(&txn).await?;
        policy_group_repo::set_only_default_group(&txn, default_group.id).await?;

        let users_without_group = user_repo::find_all(&txn).await?;
        let users_without_group = users_without_group
            .into_iter()
            .filter(|user| user.policy_group_id.is_none())
            .collect::<Vec<_>>();
        if users_without_group.is_empty() {
            return Ok(());
        }

        for user_model in users_without_group {
            let mut active: user::ActiveModel = user_model.into();
            active.policy_group_id = Set(Some(default_group.id));
            active.updated_at = Set(Utc::now());
            active.update(&txn).await.map_err(AsterError::from)?;
        }

        Ok(())
    }
    .await;

    match result {
        Ok(()) => txn.commit().await.map_err(AsterError::from),
        Err(err) => {
            txn.rollback().await.map_err(AsterError::from)?;
            Err(err)
        }
    }
}

pub async fn list_groups_paginated(
    state: &AppState,
    limit: u64,
    offset: u64,
) -> Result<OffsetPage<StoragePolicyGroupInfo>> {
    let page = load_offset_page(limit, offset, 100, |limit, offset| async move {
        policy_group_repo::find_groups_paginated(&state.db, limit, offset).await
    })
    .await?;
    Ok(OffsetPage {
        items: page
            .items
            .iter()
            .map(|group| build_group_info(state, group))
            .collect(),
        total: page.total,
        limit: page.limit,
        offset: page.offset,
    })
}

pub async fn get_group(state: &AppState, id: i64) -> Result<StoragePolicyGroupInfo> {
    let group = policy_group_repo::find_group_by_id(&state.db, id).await?;
    Ok(build_group_info(state, &group))
}

pub async fn create_group(
    state: &AppState,
    input: CreateStoragePolicyGroupInput,
) -> Result<StoragePolicyGroupInfo> {
    let CreateStoragePolicyGroupInput {
        name,
        description,
        is_enabled,
        is_default,
        items,
    } = input;
    if is_default && !is_enabled {
        return Err(AsterError::validation_error(
            "default storage policy group must be enabled",
        ));
    }

    validate_group_items(&state.db, &items).await?;

    let txn = state.db.begin().await.map_err(AsterError::from)?;
    let now = Utc::now();
    let group = policy_group_repo::create_group(
        &txn,
        storage_policy_group::ActiveModel {
            name: Set(name),
            description: Set(description.unwrap_or_default()),
            is_enabled: Set(is_enabled),
            is_default: Set(false),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        },
    )
    .await?;
    replace_group_items(&txn, group.id, &items).await?;
    if is_default {
        lock_default_group_assignment(&txn).await?;
        policy_group_repo::set_only_default_group(&txn, group.id).await?;
    }
    txn.commit().await.map_err(AsterError::from)?;
    state.policy_snapshot.reload(&state.db).await?;
    let group = policy_group_repo::find_group_by_id(&state.db, group.id).await?;
    Ok(build_group_info(state, &group))
}

pub async fn update_group(
    state: &AppState,
    id: i64,
    input: UpdateStoragePolicyGroupInput,
) -> Result<StoragePolicyGroupInfo> {
    let UpdateStoragePolicyGroupInput {
        name,
        description,
        is_enabled,
        is_default,
        items,
    } = input;
    let txn = state.db.begin().await.map_err(AsterError::from)?;
    let existing = policy_group_repo::find_group_by_id(&txn, id).await?;
    let next_is_enabled = is_enabled.unwrap_or(existing.is_enabled);
    let next_is_default = is_default.unwrap_or(existing.is_default);

    if let Some(false) = is_enabled {
        if next_is_default {
            return Err(AsterError::validation_error(
                "cannot disable the default storage policy group; set another group as default first",
            ));
        }

        if existing.is_enabled {
            let user_assignment_count =
                policy_group_repo::count_user_group_assignments(&txn, id).await?;
            let team_assignment_count = team_repo::count_active_by_policy_group(&txn, id).await?;
            if let Some(message) = format_group_assignment_blocker(
                "disable",
                user_assignment_count,
                team_assignment_count,
            ) {
                return Err(AsterError::validation_error(message));
            }
        }
    }

    if let Some(true) = is_default
        && !next_is_enabled
    {
        return Err(AsterError::validation_error(
            "default storage policy group must be enabled",
        ));
    }

    if let Some(false) = is_default
        && existing.is_default
    {
        let all = policy_group_repo::find_all_groups(&txn).await?;
        let default_count = all.iter().filter(|group| group.is_default).count();
        if default_count <= 1 {
            return Err(AsterError::validation_error(
                "cannot unset the only default storage policy group",
            ));
        }
    }

    if let Some(ref updated_items) = items {
        validate_group_items(&txn, updated_items).await?;
    }

    let mut active: storage_policy_group::ActiveModel = existing.into();
    if let Some(value) = name {
        active.name = Set(value);
    }
    if let Some(value) = description {
        active.description = Set(value);
    }
    if let Some(value) = is_enabled {
        active.is_enabled = Set(value);
    }
    if let Some(value) = is_default {
        active.is_default = Set(value);
    }
    active.updated_at = Set(Utc::now());
    let group = policy_group_repo::update_group(&txn, active).await?;

    if let Some(updated_items) = items {
        replace_group_items(&txn, group.id, &updated_items).await?;
    }

    if is_default == Some(true) {
        lock_default_group_assignment(&txn).await?;
        policy_group_repo::set_only_default_group(&txn, group.id).await?;
    }

    txn.commit().await.map_err(AsterError::from)?;
    state.policy_snapshot.reload(&state.db).await?;
    let group = policy_group_repo::find_group_by_id(&state.db, group.id).await?;
    Ok(build_group_info(state, &group))
}

pub async fn delete_group(state: &AppState, id: i64) -> Result<()> {
    let group = policy_group_repo::find_group_by_id(&state.db, id).await?;

    if group.is_default {
        let all = policy_group_repo::find_all_groups(&state.db).await?;
        let default_count = all.iter().filter(|item| item.is_default).count();
        if default_count <= 1 {
            return Err(AsterError::validation_error(
                "cannot delete the only default storage policy group",
            ));
        }
    }

    let user_assignment_count =
        policy_group_repo::count_user_group_assignments(&state.db, id).await?;
    let team_assignment_count = team_repo::count_active_by_policy_group(&state.db, id).await?;
    if let Some(message) =
        format_group_assignment_blocker("delete", user_assignment_count, team_assignment_count)
    {
        return Err(AsterError::validation_error(message));
    }

    policy_group_repo::delete_group(&state.db, id).await?;
    state.policy_snapshot.reload(&state.db).await?;
    Ok(())
}

pub async fn migrate_group_users(
    state: &AppState,
    source_group_id: i64,
    target_group_id: i64,
) -> Result<PolicyGroupUserMigrationResult> {
    if source_group_id == target_group_id {
        return Err(AsterError::validation_error(
            "source and target storage policy groups must be different",
        ));
    }

    policy_group_repo::find_group_by_id(&state.db, source_group_id).await?;
    let target_group = policy_group_repo::find_group_by_id(&state.db, target_group_id).await?;
    if !target_group.is_enabled {
        return Err(AsterError::validation_error(
            "cannot migrate users to a disabled storage policy group",
        ));
    }
    if policy_group_repo::find_group_items(&state.db, target_group_id)
        .await?
        .is_empty()
    {
        return Err(AsterError::validation_error(
            "cannot migrate users to a storage policy group without policies",
        ));
    }

    let source_users = user_repo::find_by_policy_group(&state.db, source_group_id).await?;
    if source_users.is_empty() {
        return Ok(PolicyGroupUserMigrationResult {
            source_group_id,
            target_group_id,
            affected_users: 0,
            migrated_assignments: 0,
        });
    }

    let txn = state.db.begin().await.map_err(AsterError::from)?;
    let migrated_assignments = source_users.len() as u64;
    for source_user in source_users {
        let mut active: user::ActiveModel = source_user.into();
        active.policy_group_id = Set(Some(target_group_id));
        active.updated_at = Set(Utc::now());
        active.update(&txn).await.map_err(AsterError::from)?;
    }

    txn.commit().await.map_err(AsterError::from)?;
    state.policy_snapshot.reload(&state.db).await?;

    Ok(PolicyGroupUserMigrationResult {
        source_group_id,
        target_group_id,
        affected_users: migrated_assignments,
        migrated_assignments,
    })
}
