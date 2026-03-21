use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};

use crate::db::repository::user_repo;
use crate::entities::user;
use crate::errors::{AsterError, Result};
use crate::runtime::AppState;
use crate::types::{UserRole, UserStatus};

pub async fn list_all(state: &AppState) -> Result<Vec<user::Model>> {
    user_repo::find_all(&state.db).await
}

pub async fn get(state: &AppState, id: i64) -> Result<user::Model> {
    user_repo::find_by_id(&state.db, id).await
}

pub async fn update(
    state: &AppState,
    id: i64,
    role: Option<UserRole>,
    status: Option<UserStatus>,
    storage_quota: Option<i64>,
) -> Result<user::Model> {
    let existing = user_repo::find_by_id(&state.db, id).await?;
    let mut active: user::ActiveModel = existing.into();
    if let Some(r) = role {
        active.role = Set(r);
    }
    if let Some(s) = status {
        active.status = Set(s);
    }
    if let Some(q) = storage_quota {
        active.storage_quota = Set(q);
    }
    active.updated_at = Set(Utc::now());
    active.update(&state.db).await.map_err(AsterError::from)
}

/// 强制删除用户及其所有数据（不可逆）
///
/// 级联清理顺序：
/// 1. 永久删除所有文件（blob cleanup + 版本 + 缩略图 + 属性）
/// 2. 删除所有文件夹（+ 属性）
/// 3. 删除所有分享链接
/// 4. 删除所有 WebDAV 账号
/// 5. 删除用户存储策略分配
/// 6. 清理上传 session 和临时文件
/// 7. 清理资源锁
/// 8. 删除用户记录
pub async fn force_delete(state: &AppState, target_user_id: i64) -> Result<()> {
    let db = &state.db;
    let user = user_repo::find_by_id(db, target_user_id).await?;

    // id=1 初始管理员绝对不可删除
    if target_user_id == 1 {
        return Err(AsterError::validation_error(
            "cannot delete the initial admin account",
        ));
    }

    // 其他 admin 也不可删（需要先降级为 user 再删除）
    if user.role.is_admin() {
        return Err(AsterError::validation_error(
            "cannot force-delete an admin user, demote to user first",
        ));
    }

    tracing::warn!(
        "force-deleting user #{} ({}), cascading all data",
        user.id,
        user.username
    );

    // 1. 永久删除所有文件（含软删除的）
    let all_files = {
        use crate::entities::file::Entity as File;
        File::find()
            .filter(crate::entities::file::Column::UserId.eq(target_user_id))
            .all(db)
            .await
            .map_err(AsterError::from)?
    };
    for f in &all_files {
        if let Err(e) = crate::services::file_service::purge(state, f.id, target_user_id).await {
            tracing::warn!("failed to purge file #{}: {e}", f.id);
        }
    }

    // 2. 删除所有文件夹（+ 属性）
    let all_folders = {
        use crate::entities::folder::Entity as Folder;
        Folder::find()
            .filter(crate::entities::folder::Column::UserId.eq(target_user_id))
            .all(db)
            .await
            .map_err(AsterError::from)?
    };
    for f in &all_folders {
        // 清理属性
        if let Err(e) = crate::db::repository::property_repo::delete_all_for_entity(
            db,
            crate::types::EntityType::Folder,
            f.id,
        )
        .await
        {
            tracing::warn!("failed to delete properties for folder #{}: {e}", f.id);
        }
        // 硬删除
        if let Err(e) = crate::db::repository::folder_repo::delete(db, f.id).await {
            tracing::warn!("failed to delete folder #{}: {e}", f.id);
        }
    }

    // 3. 删除所有分享链接
    {
        use crate::entities::share::Entity as Share;
        Share::delete_many()
            .filter(crate::entities::share::Column::UserId.eq(target_user_id))
            .exec(db)
            .await
            .map_err(AsterError::from)?;
    }

    // 4. 删除所有 WebDAV 账号
    {
        use crate::entities::webdav_account::Entity as WA;
        WA::delete_many()
            .filter(crate::entities::webdav_account::Column::UserId.eq(target_user_id))
            .exec(db)
            .await
            .map_err(AsterError::from)?;
    }

    // 5. 删除用户存储策略分配
    {
        use crate::entities::user_storage_policy::Entity as USP;
        USP::delete_many()
            .filter(crate::entities::user_storage_policy::Column::UserId.eq(target_user_id))
            .exec(db)
            .await
            .map_err(AsterError::from)?;
    }

    // 6. 清理上传 session
    {
        use crate::entities::upload_session::Entity as US;
        US::delete_many()
            .filter(crate::entities::upload_session::Column::UserId.eq(target_user_id))
            .exec(db)
            .await
            .map_err(AsterError::from)?;
    }

    // 7. 清理用户持有的资源锁
    {
        use crate::entities::resource_lock::Entity as RL;
        let locks = RL::find()
            .filter(crate::entities::resource_lock::Column::OwnerId.eq(target_user_id))
            .all(db)
            .await
            .map_err(AsterError::from)?;
        for lock in &locks {
            let _ = crate::services::lock_service::set_entity_locked(
                db,
                lock.entity_type,
                lock.entity_id,
                false,
            )
            .await;
        }
        RL::delete_many()
            .filter(crate::entities::resource_lock::Column::OwnerId.eq(target_user_id))
            .exec(db)
            .await
            .map_err(AsterError::from)?;
    }

    // 8. 删除用户记录
    user::Entity::delete_by_id(target_user_id)
        .exec(db)
        .await
        .map_err(AsterError::from)?;

    // 清理缓存
    state
        .cache
        .delete(&format!("user_default_policy:{target_user_id}"))
        .await;

    tracing::info!(
        "force-deleted user #{} ({}) and all associated data ({} files, {} folders)",
        user.id,
        user.username,
        all_files.len(),
        all_folders.len(),
    );

    Ok(())
}
