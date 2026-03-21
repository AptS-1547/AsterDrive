use std::future::Future;
use std::pin::Pin;

use chrono::Utc;
use sea_orm::Set;

use crate::db::repository::{file_repo, folder_repo};
use crate::entities::folder;
use crate::errors::Result;
use crate::runtime::AppState;
use crate::services::file_service;

/// 递归软删除文件夹及其所有内容（→ 回收站）
pub fn recursive_soft_delete<'a>(
    state: &'a AppState,
    user_id: i64,
    folder_id: i64,
) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
    Box::pin(async move {
        let db = &state.db;

        // 软删除该文件夹下所有文件
        let files = file_repo::find_by_folder(db, user_id, Some(folder_id)).await?;
        for f in files {
            file_repo::soft_delete(db, f.id).await?;
        }

        // 递归软删除子文件夹
        let children = folder_repo::find_children(db, user_id, Some(folder_id)).await?;
        for child in children {
            recursive_soft_delete(state, user_id, child.id).await?;
        }

        // 软���除当前文件夹
        folder_repo::soft_delete(db, folder_id).await?;
        Ok(())
    })
}

/// 递归永久删除文件夹及其所有内容（回收站清理用）
pub fn recursive_purge_folder<'a>(
    state: &'a AppState,
    user_id: i64,
    folder_id: i64,
) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
    Box::pin(async move {
        let db = &state.db;

        // 永久删除文件（含 blob cleanup）
        let files = file_repo::find_by_folder(db, user_id, Some(folder_id)).await?;
        for f in files {
            file_service::purge(state, f.id, user_id).await?;
        }

        // 也要处理已软删除但还未清理的文件（精确查询该文件夹）
        let deleted_files = file_repo::find_deleted_in_folder(db, folder_id).await?;
        for f in deleted_files {
            file_service::purge(state, f.id, user_id).await?;
        }

        // 递归子文件夹
        let children = folder_repo::find_children(db, user_id, Some(folder_id)).await?;
        for child in children {
            recursive_purge_folder(state, user_id, child.id).await?;
        }

        // 清理属性
        crate::db::repository::property_repo::delete_all_for_entity(
            db,
            crate::types::EntityType::Folder,
            folder_id,
        )
        .await?;

        // 硬删除文件夹
        folder_repo::delete(db, folder_id).await?;
        Ok(())
    })
}

/// 递归复制文件夹及其所有内容到新位置
///
/// 利用 blob 去重：只增加 ref_count，不复制物理数据
pub fn recursive_copy_folder<'a>(
    state: &'a AppState,
    user_id: i64,
    src_folder_id: i64,
    dest_parent_id: Option<i64>,
    dest_name: &'a str,
) -> Pin<Box<dyn Future<Output = Result<folder::Model>> + Send + 'a>> {
    Box::pin(async move {
        let db = &state.db;
        let now = Utc::now();

        // 创建目标文件夹
        let new_folder = folder_repo::create(
            db,
            folder::ActiveModel {
                name: Set(dest_name.to_string()),
                parent_id: Set(dest_parent_id),
                user_id: Set(user_id),
                policy_id: Set(None),
                created_at: Set(now),
                updated_at: Set(now),
                ..Default::default()
            },
        )
        .await?;

        // 复制文件：用 duplicate_file_record 统一处理
        let files = file_repo::find_by_folder(db, user_id, Some(src_folder_id)).await?;
        for f in &files {
            file_service::duplicate_file_record(state, f, Some(new_folder.id), &f.name).await?;
        }

        // 递归复制子文件夹
        let children = folder_repo::find_children(db, user_id, Some(src_folder_id)).await?;
        for child in children {
            recursive_copy_folder(state, user_id, child.id, Some(new_folder.id), &child.name)
                .await?;
        }

        Ok(new_folder)
    })
}
