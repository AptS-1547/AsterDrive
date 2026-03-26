use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, ConnectionTrait, EntityTrait, ExprTrait,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Set,
};

use crate::entities::folder::{self, Entity as Folder};
use crate::errors::{AsterError, Result};

pub async fn find_by_id<C: ConnectionTrait>(db: &C, id: i64) -> Result<folder::Model> {
    Folder::find_by_id(id)
        .one(db)
        .await
        .map_err(AsterError::from)?
        .ok_or_else(|| AsterError::record_not_found(format!("folder #{id}")))
}

/// 查询子文件夹（排除已删除）
pub async fn find_children<C: ConnectionTrait>(
    db: &C,
    user_id: i64,
    parent_id: Option<i64>,
) -> Result<Vec<folder::Model>> {
    let mut q = Folder::find()
        .filter(folder::Column::UserId.eq(user_id))
        .filter(folder::Column::DeletedAt.is_null())
        .order_by_asc(folder::Column::Name);
    q = match parent_id {
        Some(pid) => q.filter(folder::Column::ParentId.eq(pid)),
        None => q.filter(folder::Column::ParentId.is_null()),
    };
    q.all(db).await.map_err(AsterError::from)
}

/// 查询子文件夹（排除已删除，分页）
pub async fn find_children_paginated<C: ConnectionTrait>(
    db: &C,
    user_id: i64,
    parent_id: Option<i64>,
    limit: u64,
    offset: u64,
    sort_by: crate::api::pagination::SortBy,
    sort_order: crate::api::pagination::SortOrder,
) -> Result<(Vec<folder::Model>, u64)> {
    let mut cond = Condition::all()
        .add(folder::Column::UserId.eq(user_id))
        .add(folder::Column::DeletedAt.is_null());
    cond = match parent_id {
        Some(pid) => cond.add(folder::Column::ParentId.eq(pid)),
        None => cond.add(folder::Column::ParentId.is_null()),
    };

    let base = Folder::find().filter(cond);

    let total = base.clone().count(db).await.map_err(AsterError::from)?;
    if total == 0 || limit == 0 {
        return Ok((vec![], total));
    }

    use crate::api::pagination::{SortBy, SortOrder};
    let is_asc = sort_order == SortOrder::Asc;
    let items = match sort_by {
        SortBy::CreatedAt => {
            if is_asc {
                base.order_by_asc(folder::Column::CreatedAt)
                    .order_by_asc(folder::Column::Id)
            } else {
                base.order_by_desc(folder::Column::CreatedAt)
                    .order_by_desc(folder::Column::Id)
            }
        }
        SortBy::UpdatedAt => {
            if is_asc {
                base.order_by_asc(folder::Column::UpdatedAt)
                    .order_by_asc(folder::Column::Id)
            } else {
                base.order_by_desc(folder::Column::UpdatedAt)
                    .order_by_desc(folder::Column::Id)
            }
        }
        // name, size, type — all fall back to name for folders
        _ => {
            if is_asc {
                base.order_by_asc(folder::Column::Name)
                    .order_by_asc(folder::Column::Id)
            } else {
                base.order_by_desc(folder::Column::Name)
                    .order_by_desc(folder::Column::Id)
            }
        }
    }
    .offset(offset)
    .limit(limit)
    .all(db)
    .await
    .map_err(AsterError::from)?;

    Ok((items, total))
}

/// 查询顶层已删除文件夹（分页），用 SQL 过滤而非内存过滤
pub async fn find_top_level_deleted_paginated<C: ConnectionTrait>(
    db: &C,
    user_id: i64,
    limit: u64,
    offset: u64,
) -> Result<(Vec<folder::Model>, u64)> {
    // 顶层 = deleted_at IS NOT NULL 且 parent 要么是 NULL，要么 parent 未被删除
    use sea_orm::sea_query::{Alias, Expr, Query};

    let parent_deleted_subquery = Query::select()
        .expr(Expr::val(1i32))
        .from_as(Alias::new("folders"), Alias::new("p"))
        .and_where(
            Expr::col((Alias::new("p"), Alias::new("id")))
                .equals((Alias::new("folders"), folder::Column::ParentId)),
        )
        .and_where(Expr::col((Alias::new("p"), Alias::new("deleted_at"))).is_not_null())
        .to_owned();

    let cond = Condition::all()
        .add(folder::Column::UserId.eq(user_id))
        .add(folder::Column::DeletedAt.is_not_null())
        .add(
            Condition::any()
                .add(folder::Column::ParentId.is_null())
                .add(Expr::exists(parent_deleted_subquery).not()),
        );

    let base = Folder::find().filter(cond);

    let total = base.clone().count(db).await.map_err(AsterError::from)?;
    if total == 0 || limit == 0 {
        return Ok((vec![], total));
    }

    let items = base
        .order_by_desc(folder::Column::DeletedAt)
        .offset(offset)
        .limit(limit)
        .all(db)
        .await
        .map_err(AsterError::from)?;

    Ok((items, total))
}

/// 按名称查文件夹（排除已删除）
pub async fn find_by_name_in_parent<C: ConnectionTrait>(
    db: &C,
    user_id: i64,
    parent_id: Option<i64>,
    name: &str,
) -> Result<Option<folder::Model>> {
    let mut q = Folder::find()
        .filter(folder::Column::UserId.eq(user_id))
        .filter(folder::Column::Name.eq(name))
        .filter(folder::Column::DeletedAt.is_null());
    q = match parent_id {
        Some(pid) => q.filter(folder::Column::ParentId.eq(pid)),
        None => q.filter(folder::Column::ParentId.is_null()),
    };
    q.one(db).await.map_err(AsterError::from)
}

pub async fn create<C: ConnectionTrait>(
    db: &C,
    model: folder::ActiveModel,
) -> Result<folder::Model> {
    model.insert(db).await.map_err(AsterError::from)
}

/// 硬删除文件夹记录（回收站清理用）
pub async fn delete<C: ConnectionTrait>(db: &C, id: i64) -> Result<()> {
    Folder::delete_by_id(id)
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(())
}

/// 批量硬删除文件夹记录
pub async fn delete_many<C: ConnectionTrait>(db: &C, ids: &[i64]) -> Result<()> {
    if ids.is_empty() {
        return Ok(());
    }
    Folder::delete_many()
        .filter(folder::Column::Id.is_in(ids.to_vec()))
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(())
}

/// 查找某文件夹下的所有子文件夹（含已删除，递归收集用）
pub async fn find_all_children<C: ConnectionTrait>(
    db: &C,
    parent_id: i64,
) -> Result<Vec<folder::Model>> {
    Folder::find()
        .filter(folder::Column::ParentId.eq(parent_id))
        .all(db)
        .await
        .map_err(AsterError::from)
}

/// 查找某文件夹下的所有文件（含已删除，递归收集用）
pub async fn find_all_files_in_folder<C: ConnectionTrait>(
    db: &C,
    folder_id: i64,
) -> Result<Vec<crate::entities::file::Model>> {
    use crate::entities::file::{self, Entity as File};
    File::find()
        .filter(file::Column::FolderId.eq(folder_id))
        .all(db)
        .await
        .map_err(AsterError::from)
}

/// 查找文件夹的祖先链（从根下第一层到当前文件夹），校验归属与未删除
pub async fn find_ancestors<C: ConnectionTrait>(
    db: &C,
    user_id: i64,
    folder_id: i64,
) -> Result<Vec<(i64, String)>> {
    let mut path = Vec::new();
    let mut current_id = folder_id;

    loop {
        let folder = find_by_id(db, current_id).await?;
        crate::utils::verify_owner(folder.user_id, user_id, "folder")?;
        if folder.deleted_at.is_some() {
            return Err(AsterError::file_not_found(format!(
                "folder #{current_id} is in trash"
            )));
        }
        path.push((folder.id, folder.name));
        match folder.parent_id {
            Some(pid) => current_id = pid,
            None => break,
        }
    }

    path.reverse();
    Ok(path)
}

// ── 软删除 / 回收站 ─────────────────────────────────────────────────

/// 软删除：标记 deleted_at
pub async fn soft_delete<C: ConnectionTrait>(db: &C, id: i64) -> Result<()> {
    let f = find_by_id(db, id).await?;
    let mut active: folder::ActiveModel = f.into();
    active.deleted_at = Set(Some(Utc::now()));
    active.update(db).await.map_err(AsterError::from)?;
    Ok(())
}

/// 批量软删除：一次 UPDATE 标记多个文件夹的 deleted_at
pub async fn soft_delete_many<C: ConnectionTrait>(
    db: &C,
    ids: &[i64],
    now: chrono::DateTime<Utc>,
) -> Result<()> {
    if ids.is_empty() {
        return Ok(());
    }
    Folder::update_many()
        .col_expr(
            folder::Column::DeletedAt,
            sea_orm::sea_query::Expr::value(Some(now)),
        )
        .filter(folder::Column::Id.is_in(ids.to_vec()))
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(())
}

/// 恢复：清除 deleted_at
pub async fn restore<C: ConnectionTrait>(db: &C, id: i64) -> Result<()> {
    let f = find_by_id(db, id).await?;
    let mut active: folder::ActiveModel = f.into();
    active.deleted_at = Set(None);
    active.update(db).await.map_err(AsterError::from)?;
    Ok(())
}

/// 批量恢复：一次 UPDATE 清除多个文件夹的 deleted_at
pub async fn restore_many<C: ConnectionTrait>(db: &C, ids: &[i64]) -> Result<()> {
    if ids.is_empty() {
        return Ok(());
    }
    Folder::update_many()
        .col_expr(
            folder::Column::DeletedAt,
            sea_orm::sea_query::Expr::value(Option::<chrono::DateTime<Utc>>::None),
        )
        .filter(folder::Column::Id.is_in(ids.to_vec()))
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(())
}

/// 查询用户回收站中的文件夹（只查顶层被删除的，不含子目录）
pub async fn find_deleted_by_user<C: ConnectionTrait>(
    db: &C,
    user_id: i64,
) -> Result<Vec<folder::Model>> {
    Folder::find()
        .filter(folder::Column::UserId.eq(user_id))
        .filter(folder::Column::DeletedAt.is_not_null())
        .order_by_desc(folder::Column::DeletedAt)
        .all(db)
        .await
        .map_err(AsterError::from)
}

/// 查询某文件夹下的已删除子文件夹（递归恢复/清理用，避免 N+1）
pub async fn find_deleted_children<C: ConnectionTrait>(
    db: &C,
    parent_id: i64,
) -> Result<Vec<folder::Model>> {
    Folder::find()
        .filter(folder::Column::ParentId.eq(parent_id))
        .filter(folder::Column::DeletedAt.is_not_null())
        .all(db)
        .await
        .map_err(AsterError::from)
}

/// 清除引用某存储策略的所有 folder.policy_id（策略删除时调用）
pub async fn clear_policy_references<C: ConnectionTrait>(db: &C, policy_id: i64) -> Result<u64> {
    let result = Folder::update_many()
        .col_expr(
            folder::Column::PolicyId,
            sea_orm::sea_query::Expr::value(Option::<i64>::None),
        )
        .filter(folder::Column::PolicyId.eq(policy_id))
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(result.rows_affected)
}

/// 查询过期的已删除文件夹（自动清理用）
pub async fn find_expired_deleted<C: ConnectionTrait>(
    db: &C,
    before: chrono::DateTime<Utc>,
) -> Result<Vec<folder::Model>> {
    Folder::find()
        .filter(folder::Column::DeletedAt.is_not_null())
        .filter(folder::Column::DeletedAt.lt(before))
        .all(db)
        .await
        .map_err(AsterError::from)
}

/// 查询用户的所有文件夹（含已删除，force_delete 用）
pub async fn find_all_by_user<C: ConnectionTrait>(
    db: &C,
    user_id: i64,
) -> Result<Vec<folder::Model>> {
    Folder::find()
        .filter(folder::Column::UserId.eq(user_id))
        .all(db)
        .await
        .map_err(AsterError::from)
}
