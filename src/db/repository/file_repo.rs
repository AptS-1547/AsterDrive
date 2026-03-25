use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, ExprTrait, PaginatorTrait,
    QueryFilter, QueryOrder, QuerySelect, Set, sea_query::Expr,
};

use crate::entities::{
    file::{self, Entity as File},
    file_blob::{self, Entity as FileBlob},
};
use crate::errors::{AsterError, Result};

pub async fn find_by_id<C: ConnectionTrait>(db: &C, id: i64) -> Result<file::Model> {
    File::find_by_id(id)
        .one(db)
        .await
        .map_err(AsterError::from)?
        .ok_or_else(|| AsterError::file_not_found(format!("file #{id}")))
}

/// 查询文件夹下的文件（排除已删除）
pub async fn find_by_folder<C: ConnectionTrait>(
    db: &C,
    user_id: i64,
    folder_id: Option<i64>,
) -> Result<Vec<file::Model>> {
    let mut q = File::find()
        .filter(file::Column::UserId.eq(user_id))
        .filter(file::Column::DeletedAt.is_null())
        .order_by_asc(file::Column::Name);
    q = match folder_id {
        Some(fid) => q.filter(file::Column::FolderId.eq(fid)),
        None => q.filter(file::Column::FolderId.is_null()),
    };
    q.all(db).await.map_err(AsterError::from)
}

/// 查询文件夹下的文件（排除已删除，分页）
pub async fn find_by_folder_paginated<C: ConnectionTrait>(
    db: &C,
    user_id: i64,
    folder_id: Option<i64>,
    limit: u64,
    offset: u64,
) -> Result<(Vec<file::Model>, u64)> {
    let mut cond = sea_orm::Condition::all()
        .add(file::Column::UserId.eq(user_id))
        .add(file::Column::DeletedAt.is_null());
    cond = match folder_id {
        Some(fid) => cond.add(file::Column::FolderId.eq(fid)),
        None => cond.add(file::Column::FolderId.is_null()),
    };

    let base = File::find().filter(cond);

    let total = base.clone().count(db).await.map_err(AsterError::from)?;
    if total == 0 || limit == 0 {
        return Ok((vec![], total));
    }

    let items = base
        .order_by_asc(file::Column::Name)
        .offset(offset)
        .limit(limit)
        .all(db)
        .await
        .map_err(AsterError::from)?;

    Ok((items, total))
}

/// 查询顶层已删除文件（分页），用 SQL 过滤而非内存过滤
pub async fn find_top_level_deleted_paginated<C: ConnectionTrait>(
    db: &C,
    user_id: i64,
    limit: u64,
    offset: u64,
) -> Result<(Vec<file::Model>, u64)> {
    use sea_orm::sea_query::{Alias, Expr, Query};

    // 顶层 = deleted_at IS NOT NULL 且 folder 要么 NULL，要么 folder 未被删除
    let folder_deleted_subquery = Query::select()
        .expr(Expr::val(1i32))
        .from_as(Alias::new("folders"), Alias::new("f2"))
        .and_where(
            Expr::col((Alias::new("f2"), Alias::new("id")))
                .equals((Alias::new("files"), file::Column::FolderId)),
        )
        .and_where(Expr::col((Alias::new("f2"), Alias::new("deleted_at"))).is_not_null())
        .to_owned();

    let cond = sea_orm::Condition::all()
        .add(file::Column::UserId.eq(user_id))
        .add(file::Column::DeletedAt.is_not_null())
        .add(
            sea_orm::Condition::any()
                .add(file::Column::FolderId.is_null())
                .add(Expr::exists(folder_deleted_subquery).not()),
        );

    let base = File::find().filter(cond);

    let total = base.clone().count(db).await.map_err(AsterError::from)?;
    if total == 0 || limit == 0 {
        return Ok((vec![], total));
    }

    let items = base
        .order_by_desc(file::Column::DeletedAt)
        .offset(offset)
        .limit(limit)
        .all(db)
        .await
        .map_err(AsterError::from)?;

    Ok((items, total))
}

/// 按名称查文件（排除已删除）
pub async fn find_by_name_in_folder<C: ConnectionTrait>(
    db: &C,
    user_id: i64,
    folder_id: Option<i64>,
    name: &str,
) -> Result<Option<file::Model>> {
    let mut q = File::find()
        .filter(file::Column::UserId.eq(user_id))
        .filter(file::Column::Name.eq(name))
        .filter(file::Column::DeletedAt.is_null());
    q = match folder_id {
        Some(fid) => q.filter(file::Column::FolderId.eq(fid)),
        None => q.filter(file::Column::FolderId.is_null()),
    };
    q.one(db).await.map_err(AsterError::from)
}

/// 查找不冲突的文件名：如果 name 已存在则递增 " (1)", " (2)" ...
pub async fn resolve_unique_filename<C: ConnectionTrait>(
    db: &C,
    user_id: i64,
    folder_id: Option<i64>,
    name: &str,
) -> Result<String> {
    let mut final_name = name.to_string();
    while find_by_name_in_folder(db, user_id, folder_id, &final_name)
        .await?
        .is_some()
    {
        final_name = crate::utils::next_copy_name(&final_name);
    }
    Ok(final_name)
}

pub async fn find_blob_by_hash<C: ConnectionTrait>(
    db: &C,
    hash: &str,
    policy_id: i64,
) -> Result<Option<file_blob::Model>> {
    FileBlob::find()
        .filter(file_blob::Column::Hash.eq(hash))
        .filter(file_blob::Column::PolicyId.eq(policy_id))
        .one(db)
        .await
        .map_err(AsterError::from)
}

pub async fn create_blob<C: ConnectionTrait>(
    db: &C,
    model: file_blob::ActiveModel,
) -> Result<file_blob::Model> {
    model.insert(db).await.map_err(AsterError::from)
}

/// Blob 去重：查找已有 blob 则原子递增 ref_count 并返回，否则新建 ref_count=1。
pub async fn find_or_create_blob<C: ConnectionTrait>(
    db: &C,
    hash: &str,
    size: i64,
    policy_id: i64,
    storage_path: &str,
) -> Result<file_blob::Model> {
    match find_blob_by_hash(db, hash, policy_id).await? {
        Some(existing) => {
            increment_blob_ref_count(db, existing.id).await?;
            Ok(existing)
        }
        None => {
            let now = Utc::now();
            create_blob(
                db,
                file_blob::ActiveModel {
                    hash: Set(hash.to_string()),
                    size: Set(size),
                    policy_id: Set(policy_id),
                    storage_path: Set(storage_path.to_string()),
                    ref_count: Set(1),
                    created_at: Set(now),
                    updated_at: Set(now),
                    ..Default::default()
                },
            )
            .await
        }
    }
}

/// 原子递增 blob ref_count（防止并发丢更新）
pub async fn increment_blob_ref_count<C: ConnectionTrait>(db: &C, id: i64) -> Result<()> {
    FileBlob::update_many()
        .col_expr(
            file_blob::Column::RefCount,
            Expr::cust_with_values("ref_count + ?", [1i32]),
        )
        .col_expr(file_blob::Column::UpdatedAt, Expr::value(Utc::now()))
        .filter(file_blob::Column::Id.eq(id))
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(())
}

/// 原子递减 blob ref_count（floor 0，防止并发丢更新）
pub async fn decrement_blob_ref_count<C: ConnectionTrait>(db: &C, id: i64) -> Result<()> {
    FileBlob::update_many()
        .col_expr(
            file_blob::Column::RefCount,
            Expr::cust_with_values(
                "CASE WHEN ref_count < ? THEN 0 ELSE ref_count - ? END",
                [1i32, 1i32],
            ),
        )
        .col_expr(file_blob::Column::UpdatedAt, Expr::value(Utc::now()))
        .filter(file_blob::Column::Id.eq(id))
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(())
}

/// 统计某存储策略下的 blob 数量（策略删除保护用）
pub async fn count_blobs_by_policy<C: ConnectionTrait>(db: &C, policy_id: i64) -> Result<u64> {
    FileBlob::find()
        .filter(file_blob::Column::PolicyId.eq(policy_id))
        .count(db)
        .await
        .map_err(AsterError::from)
}

pub async fn create<C: ConnectionTrait>(db: &C, model: file::ActiveModel) -> Result<file::Model> {
    model.insert(db).await.map_err(AsterError::from)
}

pub async fn find_blob_by_id<C: ConnectionTrait>(db: &C, id: i64) -> Result<file_blob::Model> {
    FileBlob::find_by_id(id)
        .one(db)
        .await
        .map_err(AsterError::from)?
        .ok_or_else(|| AsterError::record_not_found(format!("file_blob #{id}")))
}

/// 批量查询 blob，返回 id → Model 的映射
pub async fn find_blobs_by_ids<C: ConnectionTrait>(
    db: &C,
    ids: &[i64],
) -> Result<std::collections::HashMap<i64, file_blob::Model>> {
    if ids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let blobs = FileBlob::find()
        .filter(file_blob::Column::Id.is_in(ids.to_vec()))
        .all(db)
        .await
        .map_err(AsterError::from)?;
    Ok(blobs.into_iter().map(|b| (b.id, b)).collect())
}

/// 硬删除文件记录（回收站清理用）
pub async fn delete<C: ConnectionTrait>(db: &C, id: i64) -> Result<()> {
    File::delete_by_id(id)
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(())
}

/// 批量硬删除文件记录
pub async fn delete_many<C: ConnectionTrait>(db: &C, ids: &[i64]) -> Result<()> {
    if ids.is_empty() {
        return Ok(());
    }
    File::delete_many()
        .filter(file::Column::Id.is_in(ids.to_vec()))
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(())
}

/// 批量硬删除 blob 记录
pub async fn delete_blobs<C: ConnectionTrait>(db: &C, ids: &[i64]) -> Result<()> {
    if ids.is_empty() {
        return Ok(());
    }
    FileBlob::delete_many()
        .filter(file_blob::Column::Id.is_in(ids.to_vec()))
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(())
}

/// 批量原子递减 blob ref_count
pub async fn decrement_blob_ref_counts<C: ConnectionTrait>(db: &C, ids: &[i64]) -> Result<()> {
    if ids.is_empty() {
        return Ok(());
    }
    FileBlob::update_many()
        .col_expr(
            file_blob::Column::RefCount,
            Expr::cust_with_values(
                "CASE WHEN ref_count < ? THEN 0 ELSE ref_count - ? END",
                [1i32, 1i32],
            ),
        )
        .col_expr(file_blob::Column::UpdatedAt, Expr::value(Utc::now()))
        .filter(file_blob::Column::Id.is_in(ids.to_vec()))
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(())
}

pub async fn delete_blob<C: ConnectionTrait>(db: &C, id: i64) -> Result<()> {
    FileBlob::delete_by_id(id)
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(())
}

// ── 软删除 / 回收站 ─────────────────────────────────────────────────

/// 软删除：标记 deleted_at
pub async fn soft_delete<C: ConnectionTrait>(db: &C, id: i64) -> Result<()> {
    let f = find_by_id(db, id).await?;
    let mut active: file::ActiveModel = f.into();
    active.deleted_at = Set(Some(Utc::now()));
    active.update(db).await.map_err(AsterError::from)?;
    Ok(())
}

/// 批量软删除：一次 UPDATE 标记多个文件的 deleted_at
pub async fn soft_delete_many<C: ConnectionTrait>(
    db: &C,
    ids: &[i64],
    now: chrono::DateTime<Utc>,
) -> Result<()> {
    if ids.is_empty() {
        return Ok(());
    }
    File::update_many()
        .col_expr(file::Column::DeletedAt, Expr::value(Some(now)))
        .filter(file::Column::Id.is_in(ids.to_vec()))
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(())
}

/// 恢复：清除 deleted_at
pub async fn restore<C: ConnectionTrait>(db: &C, id: i64) -> Result<()> {
    let f = find_by_id(db, id).await?;
    let mut active: file::ActiveModel = f.into();
    active.deleted_at = Set(None);
    active.update(db).await.map_err(AsterError::from)?;
    Ok(())
}

/// 查询用户回收站中的文件
pub async fn find_deleted_by_user<C: ConnectionTrait>(
    db: &C,
    user_id: i64,
) -> Result<Vec<file::Model>> {
    File::find()
        .filter(file::Column::UserId.eq(user_id))
        .filter(file::Column::DeletedAt.is_not_null())
        .order_by_desc(file::Column::DeletedAt)
        .all(db)
        .await
        .map_err(AsterError::from)
}

/// 查询某文件夹下的已删除文件（递归恢复/清理用，避免 N+1）
pub async fn find_deleted_in_folder<C: ConnectionTrait>(
    db: &C,
    folder_id: i64,
) -> Result<Vec<file::Model>> {
    File::find()
        .filter(file::Column::FolderId.eq(folder_id))
        .filter(file::Column::DeletedAt.is_not_null())
        .all(db)
        .await
        .map_err(AsterError::from)
}

/// 查询过期的已删除文件（自动清理用）
pub async fn find_expired_deleted<C: ConnectionTrait>(
    db: &C,
    before: chrono::DateTime<Utc>,
) -> Result<Vec<file::Model>> {
    File::find()
        .filter(file::Column::DeletedAt.is_not_null())
        .filter(file::Column::DeletedAt.lt(before))
        .all(db)
        .await
        .map_err(AsterError::from)
}

/// 查询用户的所有文件（含已删除，force_delete 用）
pub async fn find_all_by_user<C: ConnectionTrait>(
    db: &C,
    user_id: i64,
) -> Result<Vec<file::Model>> {
    File::find()
        .filter(file::Column::UserId.eq(user_id))
        .all(db)
        .await
        .map_err(AsterError::from)
}
