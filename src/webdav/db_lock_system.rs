use std::io::Cursor;
use std::time::{Duration, SystemTime};

use chrono::Utc;
use dav_server::davpath::DavPath;
use dav_server::ls::{DavLock, DavLockSystem};
use sea_orm::DatabaseConnection;
use xmltree::Element;

use crate::db::repository::lock_repo;
use crate::entities::resource_lock;
use crate::types::EntityType;
use crate::webdav::path_resolver::{self, ResolvedNode};

/// 数据库支持的 WebDAV 锁系统
///
/// Per-request 创建（需要 user_id 做 path → entity_id 解析）
#[derive(Debug, Clone)]
pub struct DbLockSystem {
    db: DatabaseConnection,
    user_id: i64,
    root_folder_id: Option<i64>,
}

impl DbLockSystem {
    pub fn new(db: DatabaseConnection, user_id: i64, root_folder_id: Option<i64>) -> Box<Self> {
        Box::new(Self {
            db,
            user_id,
            root_folder_id,
        })
    }
}

type LsFuture<'a, T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send + 'a>>;

impl DavLockSystem for DbLockSystem {
    fn lock(
        &self,
        path: &DavPath,
        principal: Option<&str>,
        owner: Option<&Element>,
        timeout: Option<Duration>,
        shared: bool,
        deep: bool,
    ) -> LsFuture<'_, Result<DavLock, DavLock>> {
        let path_str = normalize_path(path);
        let path_owned = path.clone();
        let principal_owned = principal.map(|s| s.to_string());
        let owner_xml = owner.map(serialize_element);
        let owner_clone = owner.cloned();
        let timeout_dur = timeout;

        Box::pin(async move {
            let now = Utc::now();

            // 解析路径到 entity
            let (entity_type, entity_id) =
                resolve_path_to_entity(&self.db, self.user_id, self.root_folder_id, &path_str)
                    .await
                    .map_err(|_| empty_dav_lock(&path_owned))?;

            // 检查是否已锁
            if let Some(existing) = lock_repo::find_by_entity(&self.db, entity_type, entity_id)
                .await
                .unwrap_or(None)
            {
                let is_expired = existing.timeout_at.is_some_and(|t| t < now);
                if !is_expired {
                    return Err(model_to_dav_lock(&existing));
                }
                // 过期锁：清理
                let _ = lock_repo::delete_by_entity(&self.db, entity_type, entity_id).await;
                // 重置 is_locked
                let _ = crate::services::lock_service::set_entity_locked(
                    &self.db,
                    entity_type,
                    entity_id,
                    false,
                )
                .await;
            }

            let token = format!("urn:uuid:{}", uuid::Uuid::new_v4());
            let timeout_at = timeout_dur.map(|d| now + chrono::Duration::from_std(d).unwrap());

            let model = resource_lock::ActiveModel {
                token: sea_orm::Set(token.clone()),
                entity_type: sea_orm::Set(entity_type),
                entity_id: sea_orm::Set(entity_id),
                path: sea_orm::Set(path_str),
                owner_id: sea_orm::Set(None), // WebDAV 没有 user_id（用 principal 代替）
                owner_info: sea_orm::Set(owner_xml),
                timeout_at: sea_orm::Set(timeout_at),
                shared: sea_orm::Set(shared),
                deep: sea_orm::Set(deep),
                created_at: sea_orm::Set(now),
                ..Default::default()
            };

            lock_repo::create(&self.db, model)
                .await
                .map_err(|_| empty_dav_lock(&path_owned))?;

            // 同步 is_locked
            let _ = crate::services::lock_service::set_entity_locked(
                &self.db,
                entity_type,
                entity_id,
                true,
            )
            .await;

            Ok(DavLock {
                token,
                path: Box::new(path_owned),
                principal: principal_owned,
                owner: owner_clone.map(Box::new),
                timeout_at: timeout_dur.map(|d| SystemTime::now() + d),
                timeout: timeout_dur,
                shared,
                deep,
            })
        })
    }

    fn unlock(&self, _path: &DavPath, token: &str) -> LsFuture<'_, Result<(), ()>> {
        let token_owned = token.to_string();
        Box::pin(async move {
            // 查锁拿 entity 信息
            let lock = lock_repo::find_by_token(&self.db, &token_owned)
                .await
                .map_err(|_| ())?
                .ok_or(())?;

            lock_repo::delete_by_token(&self.db, &token_owned)
                .await
                .map_err(|_| ())?;

            // 同步 is_locked
            let _ = crate::services::lock_service::set_entity_locked(
                &self.db,
                lock.entity_type,
                lock.entity_id,
                false,
            )
            .await;
            Ok(())
        })
    }

    fn refresh(
        &self,
        path: &DavPath,
        token: &str,
        timeout: Option<Duration>,
    ) -> LsFuture<'_, Result<DavLock, ()>> {
        let token_owned = token.to_string();
        let path_clone = path.clone();
        let timeout_dur = timeout;

        Box::pin(async move {
            let now = Utc::now();
            let new_timeout_at = timeout_dur.map(|d| now + chrono::Duration::from_std(d).unwrap());

            let lock = lock_repo::refresh(&self.db, &token_owned, new_timeout_at)
                .await
                .map_err(|_| ())?
                .ok_or(())?;

            Ok(DavLock {
                token: lock.token,
                path: Box::new(path_clone),
                principal: None,
                owner: lock
                    .owner_info
                    .as_deref()
                    .and_then(deserialize_element)
                    .map(Box::new),
                timeout_at: timeout_dur.map(|d| SystemTime::now() + d),
                timeout: timeout_dur,
                shared: lock.shared,
                deep: lock.deep,
            })
        })
    }

    fn check(
        &self,
        path: &DavPath,
        _principal: Option<&str>,
        ignore_principal: bool,
        deep: bool,
        submitted_tokens: &[String],
    ) -> LsFuture<'_, Result<(), DavLock>> {
        let path_str = normalize_path(path);
        let tokens: Vec<String> = submitted_tokens.to_vec();
        let _ = ignore_principal; // 简化：统一用 token 匹配

        Box::pin(async move {
            let now = Utc::now();

            // 查祖先路径的锁
            let ancestor_paths = path_ancestors(&path_str);
            let mut all_locks = lock_repo::find_ancestors(&self.db, &ancestor_paths)
                .await
                .unwrap_or_default();

            // deep check：查后代路径的锁
            if deep {
                let descendants = lock_repo::find_by_path_prefix(&self.db, &path_str)
                    .await
                    .unwrap_or_default();
                all_locks.extend(descendants);
            }

            all_locks.sort_by_key(|l| l.id);
            all_locks.dedup_by_key(|l| l.id);

            // 持有任何一个锁就通过
            let holds_any = all_locks.iter().any(|lock| {
                if lock.timeout_at.is_some_and(|t| t < now) {
                    return false;
                }
                tokens.contains(&lock.token)
            });

            if holds_any {
                return Ok(());
            }

            // 检查冲突
            for lock in &all_locks {
                if lock.timeout_at.is_some_and(|t| t < now) {
                    continue;
                }

                let is_ancestor = lock.path != path_str;
                if is_ancestor && !lock.deep {
                    continue;
                }

                let is_descendant = lock.path.starts_with(&path_str) && lock.path != path_str;
                if is_descendant && !deep {
                    continue;
                }

                return Err(model_to_dav_lock(lock));
            }

            Ok(())
        })
    }

    fn discover(&self, path: &DavPath) -> LsFuture<'_, Vec<DavLock>> {
        let path_str = normalize_path(path);

        Box::pin(async move {
            let now = Utc::now();
            let ancestor_paths = path_ancestors(&path_str);
            let locks = lock_repo::find_ancestors(&self.db, &ancestor_paths)
                .await
                .unwrap_or_default();

            locks
                .iter()
                .filter(|l| l.timeout_at.is_none_or(|t| t >= now))
                .map(model_to_dav_lock)
                .collect()
        })
    }

    fn delete(&self, path: &DavPath) -> LsFuture<'_, Result<(), ()>> {
        let path_str = normalize_path(path);
        Box::pin(async move {
            // 查出所有要删的锁（需要重置 is_locked）
            let locks = lock_repo::find_by_path_prefix(&self.db, &path_str)
                .await
                .unwrap_or_default();

            for lock in &locks {
                let _ = crate::services::lock_service::set_entity_locked(
                    &self.db,
                    lock.entity_type,
                    lock.entity_id,
                    false,
                )
                .await;
            }

            lock_repo::delete_by_path_prefix(&self.db, &path_str)
                .await
                .map(|_| ())
                .map_err(|_| ())
        })
    }
}

// ── Helpers ─────────────────────────────────────────────────────────

fn normalize_path(path: &DavPath) -> String {
    let raw = String::from_utf8_lossy(path.as_bytes()).to_string();
    if raw.is_empty() || raw == "/" {
        "/".to_string()
    } else {
        raw
    }
}

fn path_ancestors(path: &str) -> Vec<String> {
    let mut ancestors = vec!["/".to_string()];
    let trimmed = path.trim_start_matches('/');
    let mut current = String::from("/");
    for seg in trimmed.split('/') {
        if seg.is_empty() {
            continue;
        }
        current.push_str(seg);
        current.push('/');
        if current != "/" {
            ancestors.push(current.clone());
        }
    }
    if path != "/" && !path.ends_with('/') {
        ancestors.push(path.to_string());
    }
    ancestors.dedup();
    ancestors
}

/// 从 WebDAV 路径解析出 (entity_type, entity_id)
async fn resolve_path_to_entity(
    db: &sea_orm::DatabaseConnection,
    user_id: i64,
    root_folder_id: Option<i64>,
    path: &str,
) -> Result<(EntityType, i64), ()> {
    let dav_path = DavPath::new(path).map_err(|_| ())?;
    match path_resolver::resolve_path(db, user_id, &dav_path, root_folder_id).await {
        Ok(ResolvedNode::File(f)) => Ok((EntityType::File, f.id)),
        Ok(ResolvedNode::Folder(f)) => Ok((EntityType::Folder, f.id)),
        _ => Err(()),
    }
}

fn empty_dav_lock(path: &DavPath) -> DavLock {
    DavLock {
        token: String::new(),
        path: Box::new(path.clone()),
        principal: None,
        owner: None,
        timeout_at: None,
        timeout: None,
        shared: false,
        deep: false,
    }
}

fn model_to_dav_lock(lock: &resource_lock::Model) -> DavLock {
    let dav_path = DavPath::new(&lock.path).unwrap_or_else(|_| DavPath::new("/").unwrap());

    DavLock {
        token: lock.token.clone(),
        path: Box::new(dav_path),
        principal: lock.owner_id.map(|id| id.to_string()),
        owner: lock
            .owner_info
            .as_deref()
            .and_then(deserialize_element)
            .map(Box::new),
        timeout_at: lock.timeout_at.map(|t| {
            let dur = (t - Utc::now()).to_std().unwrap_or(Duration::ZERO);
            SystemTime::now() + dur
        }),
        timeout: lock
            .timeout_at
            .map(|t| (t - Utc::now()).to_std().unwrap_or(Duration::ZERO)),
        shared: lock.shared,
        deep: lock.deep,
    }
}

fn serialize_element(elem: &Element) -> String {
    let mut buf = Vec::new();
    elem.write(&mut buf).unwrap_or_default();
    String::from_utf8_lossy(&buf).to_string()
}

fn deserialize_element(xml: &str) -> Option<Element> {
    Element::parse(Cursor::new(xml.as_bytes())).ok()
}
