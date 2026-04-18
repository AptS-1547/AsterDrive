//! 服务模块：`lock_service`。

use std::io::Cursor;

use chrono::{Duration, Utc};
use sea_orm::{ConnectionTrait, Set};
use serde::{Deserialize, Serialize};
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::ToSchema;

use crate::api::pagination::{OffsetPage, load_offset_page};
use crate::db::repository::{file_repo, folder_repo, lock_repo};
use crate::entities::resource_lock;
use crate::errors::{AsterError, Result};
use crate::runtime::AppState;
use crate::services::folder_service;
use crate::types::{EntityType, StoredLockOwnerInfo};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct WopiLockOwnerInfo {
    pub app_key: String,
    pub lock: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct WebdavLockOwnerInfo {
    pub xml: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct TextLockOwnerInfo {
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LegacyWopiLockOwnerPayload {
    kind: String,
    app_key: String,
    lock: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ResourceLockOwnerInfo {
    Wopi(WopiLockOwnerInfo),
    Webdav(WebdavLockOwnerInfo),
    Text(TextLockOwnerInfo),
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct ResourceLock {
    pub id: i64,
    pub token: String,
    pub entity_type: EntityType,
    pub entity_id: i64,
    pub path: String,
    pub owner_id: Option<i64>,
    pub owner_info: Option<ResourceLockOwnerInfo>,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = Option<String>))]
    pub timeout_at: Option<chrono::DateTime<chrono::Utc>>,
    pub shared: bool,
    pub deep: bool,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = String))]
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl TryFrom<resource_lock::Model> for ResourceLock {
    type Error = AsterError;

    fn try_from(model: resource_lock::Model) -> Result<Self> {
        let owner_info = deserialize_resource_lock_owner_info(&model)?;

        Ok(Self {
            id: model.id,
            token: model.token,
            entity_type: model.entity_type,
            entity_id: model.entity_id,
            path: model.path,
            owner_id: model.owner_id,
            owner_info,
            timeout_at: model.timeout_at,
            shared: model.shared,
            deep: model.deep,
            created_at: model.created_at,
        })
    }
}

/// 锁定资源（REST/WebDAV/Web Editor 统一入口）
pub async fn lock(
    state: &AppState,
    entity_type: EntityType,
    entity_id: i64,
    owner_id: Option<i64>,
    owner_info: Option<ResourceLockOwnerInfo>,
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
        owner_info: Set(serialize_resource_lock_owner_info(owner_info.as_ref())?),
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

pub async fn list_paginated(
    state: &AppState,
    limit: u64,
    offset: u64,
) -> Result<OffsetPage<ResourceLock>> {
    load_offset_page(limit, offset, 100, |limit, offset| async move {
        let (items, total) =
            crate::db::repository::lock_repo::find_paginated(&state.db, limit, offset).await?;
        let items = items
            .into_iter()
            .map(ResourceLock::try_from)
            .collect::<Result<Vec<_>>>()?;
        Ok((items, total))
    })
    .await
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
        if let Err(e) = set_entity_locked(db, lock.entity_type, lock.entity_id, false).await {
            tracing::warn!(lock_id = lock.id, "failed to unlock expired lock: {e}");
        }
    }

    // 批量删除
    lock_repo::delete_expired(db).await?;

    Ok(count)
}

// ── Internal helpers ────────────────────────────────────────────────

pub(crate) fn serialize_resource_lock_owner_info(
    owner_info: Option<&ResourceLockOwnerInfo>,
) -> Result<Option<StoredLockOwnerInfo>> {
    let Some(owner_info) = owner_info else {
        return Ok(None);
    };

    let raw = match owner_info {
        ResourceLockOwnerInfo::Wopi(payload) => {
            serde_json::to_string(&LegacyWopiLockOwnerPayload {
                kind: "wopi".to_string(),
                app_key: payload.app_key.clone(),
                lock: payload.lock.clone(),
            })
            .map_err(|error| {
                AsterError::internal_error(format!(
                    "serialize resource lock WOPI owner payload: {error}"
                ))
            })?
        }
        ResourceLockOwnerInfo::Webdav(payload) => payload.xml.clone(),
        ResourceLockOwnerInfo::Text(payload) => payload.value.clone(),
    };

    Ok(Some(StoredLockOwnerInfo(raw)))
}

pub(crate) fn deserialize_resource_lock_owner_info(
    lock: &resource_lock::Model,
) -> Result<Option<ResourceLockOwnerInfo>> {
    let Some(raw) = lock.owner_info.as_ref() else {
        return Ok(None);
    };
    let raw = raw.as_ref();

    if let Some(payload) = parse_wopi_owner_payload(raw) {
        return Ok(Some(ResourceLockOwnerInfo::Wopi(payload)));
    }

    if xmltree::Element::parse(Cursor::new(raw.as_bytes())).is_ok() {
        return Ok(Some(ResourceLockOwnerInfo::Webdav(WebdavLockOwnerInfo {
            xml: raw.to_string(),
        })));
    }

    Ok(Some(ResourceLockOwnerInfo::Text(TextLockOwnerInfo {
        value: raw.to_string(),
    })))
}

fn parse_wopi_owner_payload(raw: &str) -> Option<WopiLockOwnerInfo> {
    let payload = serde_json::from_str::<LegacyWopiLockOwnerPayload>(raw).ok()?;
    (payload.kind == "wopi").then_some(WopiLockOwnerInfo {
        app_key: payload.app_key,
        lock: payload.lock,
    })
}

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
    db: &impl ConnectionTrait,
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
            let folder_path = match f.folder_id {
                Some(folder_id) => {
                    let mut folder_paths =
                        folder_service::build_folder_paths(db, &[folder_id]).await?;
                    let path = folder_paths.remove(&folder_id).ok_or_else(|| {
                        AsterError::record_not_found(format!("folder #{folder_id}"))
                    })?;
                    format!("{path}/")
                }
                None => String::new(),
            };
            if let Some(team_id) = f.team_id {
                let prefix = if folder_path.is_empty() {
                    format!("/teams/{team_id}/")
                } else {
                    format!("/teams/{team_id}{folder_path}")
                };
                Ok(format!("{prefix}{}", f.name))
            } else {
                let prefix = if folder_path.is_empty() {
                    "/"
                } else {
                    &folder_path
                };
                Ok(format!("{}{}", prefix, f.name))
            }
        }
        EntityType::Folder => {
            let f = folder_repo::find_by_id(db, entity_id).await?;
            let path = folder_service::build_folder_paths(db, &[f.id])
                .await?
                .remove(&f.id)
                .ok_or_else(|| AsterError::record_not_found(format!("folder #{}", f.id)))?;
            if let Some(team_id) = f.team_id {
                Ok(format!("/teams/{team_id}{path}/"))
            } else {
                Ok(format!("{path}/"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_lock(owner_info: Option<StoredLockOwnerInfo>) -> resource_lock::Model {
        resource_lock::Model {
            id: 42,
            token: "urn:uuid:test".to_string(),
            entity_type: EntityType::File,
            entity_id: 7,
            path: "/docs/report.txt".to_string(),
            owner_id: Some(9),
            owner_info,
            timeout_at: None,
            shared: false,
            deep: false,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn serializes_and_deserializes_wopi_owner_payload() {
        let owner_info = ResourceLockOwnerInfo::Wopi(WopiLockOwnerInfo {
            app_key: "collabora".to_string(),
            lock: "lock-123".to_string(),
        });
        let stored = serialize_resource_lock_owner_info(Some(&owner_info))
            .expect("wopi payload should serialize")
            .expect("stored owner info should exist");
        let parsed = deserialize_resource_lock_owner_info(&sample_lock(Some(stored)))
            .expect("wopi payload should deserialize");

        assert_eq!(parsed, Some(owner_info));
    }

    #[test]
    fn deserializes_webdav_xml_owner_payload() {
        let parsed = deserialize_resource_lock_owner_info(&sample_lock(Some(StoredLockOwnerInfo(
            "<D:owner xmlns:D=\"DAV:\"><D:href>mailto:test@example.com</D:href></D:owner>"
                .to_string(),
        ))))
        .expect("xml owner payload should deserialize");

        assert_eq!(
            parsed,
            Some(ResourceLockOwnerInfo::Webdav(WebdavLockOwnerInfo {
                xml: "<D:owner xmlns:D=\"DAV:\"><D:href>mailto:test@example.com</D:href></D:owner>"
                    .to_string(),
            }))
        );
    }

    #[test]
    fn falls_back_to_text_owner_payload() {
        let parsed = deserialize_resource_lock_owner_info(&sample_lock(Some(StoredLockOwnerInfo(
            "user@example.com".to_string(),
        ))))
        .expect("text owner payload should deserialize");

        assert_eq!(
            parsed,
            Some(ResourceLockOwnerInfo::Text(TextLockOwnerInfo {
                value: "user@example.com".to_string(),
            }))
        );
    }
}
