use chrono::{Duration, Utc};
use sea_orm::Set;

use crate::db::repository::{file_repo, folder_repo, lock_repo};
use crate::entities::resource_lock;
use crate::errors::{AsterError, Result};
use crate::runtime::AppState;
use crate::types::EntityType;

/// 锁定资源（REST/WebDAV/Web Editor 统一入口）
pub async fn lock(
    state: &AppState,
    entity_type: EntityType,
    entity_id: i64,
    owner_id: Option<i64>,
    owner_info: Option<String>,
    timeout: Option<Duration>,
) -> Result<resource_lock::Model> {
    let db = &state.db;

    // 检查是否已锁
    if let Some(existing) = lock_repo::find_by_entity(db, entity_type, entity_id).await? {
        // 过期锁自动清理
        if let Some(timeout_at) = existing.timeout_at {
            if timeout_at < Utc::now() {
                do_unlock_by_entity(state, entity_type, entity_id).await?;
            } else {
                return Err(AsterError::resource_locked("resource is already locked"));
            }
        } else {
            return Err(AsterError::resource_locked("resource is already locked"));
        }
    }

    let now = Utc::now();
    let token = format!("urn:uuid:{}", uuid::Uuid::new_v4());
    let timeout_at = timeout.map(|d| now + d);
    let path = resolve_entity_path(db, entity_type, entity_id).await?;

    let model = resource_lock::ActiveModel {
        token: Set(token),
        entity_type: Set(entity_type),
        entity_id: Set(entity_id),
        path: Set(path),
        owner_id: Set(owner_id),
        owner_info: Set(owner_info),
        timeout_at: Set(timeout_at),
        shared: Set(false),
        deep: Set(false),
        created_at: Set(now),
        ..Default::default()
    };

    let lock = lock_repo::create(db, model).await?;

    // 同步 is_locked boolean 缓存
    set_entity_locked(db, entity_type, entity_id, true).await?;

    Ok(lock)
}

/// 解锁资源（用户主动解锁）
pub async fn unlock(
    state: &AppState,
    entity_type: EntityType,
    entity_id: i64,
    user_id: i64,
) -> Result<()> {
    let db = &state.db;

    // 校验归属：只有锁持有者或文件所有者可以解锁
    if let Some(existing) = lock_repo::find_by_entity(db, entity_type, entity_id).await? {
        let is_owner = existing.owner_id == Some(user_id);
        let is_entity_owner = check_entity_ownership(db, entity_type, entity_id, user_id).await?;
        if !is_owner && !is_entity_owner {
            return Err(AsterError::auth_forbidden("not the lock owner"));
        }
    }

    do_unlock_by_entity(state, entity_type, entity_id).await
}

/// 按 token 解锁（WebDAV UNLOCK 用）
pub async fn unlock_by_token(state: &AppState, token: &str) -> Result<()> {
    let db = &state.db;
    let lock = lock_repo::find_by_token(db, token)
        .await?
        .ok_or_else(|| AsterError::record_not_found("lock not found"))?;

    lock_repo::delete_by_token(db, token).await?;
    set_entity_locked(db, lock.entity_type, lock.entity_id, false).await?;
    Ok(())
}

/// 强制解锁（admin 用）
pub async fn force_unlock(state: &AppState, lock_id: i64) -> Result<()> {
    let db = &state.db;
    let lock = lock_repo::find_by_id(db, lock_id)
        .await?
        .ok_or_else(|| AsterError::record_not_found("lock not found"))?;

    lock_repo::delete_by_id(db, lock_id).await?;
    set_entity_locked(db, lock.entity_type, lock.entity_id, false).await?;
    Ok(())
}

/// 清理过期锁（后台任务用）
pub async fn cleanup_expired(state: &AppState) -> Result<u64> {
    let db = &state.db;

    // 先查出过期锁的 entity 信息（需要重置 is_locked）
    let expired = lock_repo::find_expired(db).await?;
    if expired.is_empty() {
        return Ok(0);
    }

    let count = expired.len() as u64;

    // 批量重置 is_locked
    for lock in &expired {
        let _ = set_entity_locked(db, lock.entity_type, lock.entity_id, false).await;
    }

    // 批量删除
    lock_repo::delete_expired(db).await?;

    Ok(count)
}

// ── Internal helpers ────────────────────────────────────────────────

async fn do_unlock_by_entity(
    state: &AppState,
    entity_type: EntityType,
    entity_id: i64,
) -> Result<()> {
    lock_repo::delete_by_entity(&state.db, entity_type, entity_id).await?;
    set_entity_locked(&state.db, entity_type, entity_id, false).await?;
    Ok(())
}

/// 同步 is_locked boolean 缓存（pub 给 db_lock_system 调用）
pub async fn set_entity_locked(
    db: &sea_orm::DatabaseConnection,
    entity_type: EntityType,
    entity_id: i64,
    locked: bool,
) -> Result<()> {
    use sea_orm::ActiveModelTrait;
    let now = Utc::now();

    match entity_type {
        EntityType::File => {
            let f = file_repo::find_by_id(db, entity_id).await?;
            let mut active: crate::entities::file::ActiveModel = f.into();
            active.is_locked = Set(locked);
            active.updated_at = Set(now);
            active.update(db).await.map_err(|e| {
                tracing::error!("failed to sync is_locked for file #{entity_id}: {e}");
                AsterError::from(e)
            })?;
        }
        EntityType::Folder => {
            let f = folder_repo::find_by_id(db, entity_id).await?;
            let mut active: crate::entities::folder::ActiveModel = f.into();
            active.is_locked = Set(locked);
            active.updated_at = Set(now);
            active.update(db).await.map_err(|e| {
                tracing::error!("failed to sync is_locked for folder #{entity_id}: {e}");
                AsterError::from(e)
            })?;
        }
    }
    Ok(())
}

/// 校验资源归属
async fn check_entity_ownership(
    db: &sea_orm::DatabaseConnection,
    entity_type: EntityType,
    entity_id: i64,
    user_id: i64,
) -> Result<bool> {
    match entity_type {
        EntityType::File => {
            let f = file_repo::find_by_id(db, entity_id).await?;
            Ok(f.user_id == user_id)
        }
        EntityType::Folder => {
            let f = folder_repo::find_by_id(db, entity_id).await?;
            Ok(f.user_id == user_id)
        }
    }
}

/// 从 entity 反查 WebDAV 路径
pub async fn resolve_entity_path(
    db: &sea_orm::DatabaseConnection,
    entity_type: EntityType,
    entity_id: i64,
) -> Result<String> {
    match entity_type {
        EntityType::File => {
            let f = file_repo::find_by_id(db, entity_id).await?;
            let folder_path = resolve_folder_path(db, f.folder_id).await?;
            Ok(format!("{}{}", folder_path, f.name))
        }
        EntityType::Folder => {
            let f = folder_repo::find_by_id(db, entity_id).await?;
            let parent_path = resolve_folder_path(db, Some(f.id)).await?;
            Ok(parent_path)
        }
    }
}

/// 递归拼出文件夹路径
async fn resolve_folder_path(
    db: &sea_orm::DatabaseConnection,
    folder_id: Option<i64>,
) -> Result<String> {
    let mut segments = Vec::new();
    let mut current = folder_id;

    while let Some(fid) = current {
        let f = folder_repo::find_by_id(db, fid).await?;
        segments.push(f.name);
        current = f.parent_id;
    }

    segments.reverse();
    if segments.is_empty() {
        Ok("/".to_string())
    } else {
        Ok(format!("/{}/", segments.join("/")))
    }
}
