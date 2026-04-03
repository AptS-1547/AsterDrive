use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, ConnectionTrait, DbBackend, EntityTrait, ExprTrait,
    FromQueryResult, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Set,
    entity::prelude::DeriveIden,
    sea_query::{Asterisk, CommonTableExpression, Expr, Order, Query, UnionType, WithClause},
};

use crate::entities::folder::{self, Entity as Folder};
use crate::errors::{AsterError, Result};

#[derive(Clone, Copy)]
enum FolderScope {
    Personal { user_id: i64 },
    Team { team_id: i64 },
}

fn scope_condition(scope: FolderScope) -> Condition {
    match scope {
        FolderScope::Personal { user_id } => Condition::all()
            .add(folder::Column::UserId.eq(user_id))
            .add(folder::Column::TeamId.is_null()),
        FolderScope::Team { team_id } => Condition::all().add(folder::Column::TeamId.eq(team_id)),
    }
}

fn active_scope_condition(scope: FolderScope) -> Condition {
    scope_condition(scope).add(folder::Column::DeletedAt.is_null())
}

fn apply_parent_condition(cond: Condition, parent_id: Option<i64>) -> Condition {
    match parent_id {
        Some(parent_id) => cond.add(folder::Column::ParentId.eq(parent_id)),
        None => cond.add(folder::Column::ParentId.is_null()),
    }
}

pub async fn lock_by_id<C: ConnectionTrait>(db: &C, id: i64) -> Result<folder::Model> {
    match db.get_database_backend() {
        DbBackend::Postgres | DbBackend::MySql => Folder::find_by_id(id)
            .lock_exclusive()
            .one(db)
            .await
            .map_err(AsterError::from)?
            .ok_or_else(|| AsterError::folder_not_found(format!("folder #{id}"))),
        DbBackend::Sqlite => {
            // TODO: SQLite has no row-level `SELECT ... FOR UPDATE`. This
            // `Folder::update_many()` + `find_by_id()` sequence does not provide
            // a real row lock; callers that need stronger isolation should use
            // `BEGIN IMMEDIATE`/`BEGIN EXCLUSIVE` around this code path.
            Folder::update_many()
                .col_expr(
                    folder::Column::UpdatedAt,
                    Expr::col(folder::Column::UpdatedAt),
                )
                .filter(folder::Column::Id.eq(id))
                .exec(db)
                .await
                .map_err(AsterError::from)?;
            find_by_id(db, id).await
        }
        _ => find_by_id(db, id).await,
    }
}

#[derive(Debug, Clone, FromQueryResult)]
struct ResolvedPathFolderRow {
    segment_index: i64,
    id: i64,
    name: String,
    parent_id: Option<i64>,
    team_id: Option<i64>,
    user_id: i64,
    policy_id: Option<i64>,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
    deleted_at: Option<chrono::DateTime<chrono::Utc>>,
    is_locked: bool,
}

impl From<ResolvedPathFolderRow> for folder::Model {
    fn from(row: ResolvedPathFolderRow) -> Self {
        let _ = row.segment_index;
        Self {
            id: row.id,
            name: row.name,
            parent_id: row.parent_id,
            team_id: row.team_id,
            user_id: row.user_id,
            policy_id: row.policy_id,
            created_at: row.created_at,
            updated_at: row.updated_at,
            deleted_at: row.deleted_at,
            is_locked: row.is_locked,
        }
    }
}

#[derive(DeriveIden)]
enum RequestedSegments {
    Table,
    Column1,
    Column2,
}

#[derive(DeriveIden)]
enum RequestedValues {
    Table,
}

#[derive(DeriveIden)]
enum FolderChain {
    Table,
    SegmentIndex,
    Id,
    Name,
    ParentId,
    TeamId,
    UserId,
    PolicyId,
    CreatedAt,
    UpdatedAt,
    DeletedAt,
    IsLocked,
}

fn requested_segments_subquery(segments: &[String]) -> sea_orm::sea_query::SelectStatement {
    Query::select()
        .column(Asterisk)
        .from_values(
            segments
                .iter()
                .enumerate()
                .map(|(idx, segment)| ((idx + 1) as i64, segment.clone())),
            RequestedValues::Table,
        )
        .to_owned()
}

fn build_resolve_path_chain_query(
    user_id: i64,
    root_parent_id: Option<i64>,
    segments: &[String],
) -> sea_orm::sea_query::WithQuery {
    let base_requested = requested_segments_subquery(segments);
    let recursive_requested = requested_segments_subquery(segments);

    let mut base_select = Query::select();
    base_select
        .column((RequestedSegments::Table, RequestedSegments::Column1))
        .column((folder::Entity, folder::Column::Id))
        .column((folder::Entity, folder::Column::Name))
        .column((folder::Entity, folder::Column::ParentId))
        .column((folder::Entity, folder::Column::TeamId))
        .column((folder::Entity, folder::Column::UserId))
        .column((folder::Entity, folder::Column::PolicyId))
        .column((folder::Entity, folder::Column::CreatedAt))
        .column((folder::Entity, folder::Column::UpdatedAt))
        .column((folder::Entity, folder::Column::DeletedAt))
        .column((folder::Entity, folder::Column::IsLocked))
        .from(folder::Entity)
        .join_subquery(
            sea_orm::JoinType::InnerJoin,
            base_requested,
            RequestedSegments::Table,
            Condition::all()
                .add(Expr::col((RequestedSegments::Table, RequestedSegments::Column1)).eq(1))
                .add(
                    Expr::col((folder::Entity, folder::Column::Name))
                        .equals((RequestedSegments::Table, RequestedSegments::Column2)),
                ),
        )
        .and_where(Expr::col((folder::Entity, folder::Column::UserId)).eq(user_id))
        .and_where(Expr::col((folder::Entity, folder::Column::TeamId)).is_null())
        .and_where(Expr::col((folder::Entity, folder::Column::DeletedAt)).is_null());

    base_select = match root_parent_id {
        Some(root_parent_id) => base_select
            .and_where(Expr::col((folder::Entity, folder::Column::ParentId)).eq(root_parent_id))
            .to_owned(),
        None => base_select
            .and_where(Expr::col((folder::Entity, folder::Column::ParentId)).is_null())
            .to_owned(),
    };

    let recursive_select = Query::select()
        .column((RequestedSegments::Table, RequestedSegments::Column1))
        .column((folder::Entity, folder::Column::Id))
        .column((folder::Entity, folder::Column::Name))
        .column((folder::Entity, folder::Column::ParentId))
        .column((folder::Entity, folder::Column::TeamId))
        .column((folder::Entity, folder::Column::UserId))
        .column((folder::Entity, folder::Column::PolicyId))
        .column((folder::Entity, folder::Column::CreatedAt))
        .column((folder::Entity, folder::Column::UpdatedAt))
        .column((folder::Entity, folder::Column::DeletedAt))
        .column((folder::Entity, folder::Column::IsLocked))
        .from(folder::Entity)
        .join(
            sea_orm::JoinType::InnerJoin,
            FolderChain::Table,
            Expr::col((folder::Entity, folder::Column::ParentId))
                .equals((FolderChain::Table, FolderChain::Id)),
        )
        .join_subquery(
            sea_orm::JoinType::InnerJoin,
            recursive_requested,
            RequestedSegments::Table,
            Condition::all()
                .add(
                    Expr::col((RequestedSegments::Table, RequestedSegments::Column1))
                        .eq(Expr::col((FolderChain::Table, FolderChain::SegmentIndex)).add(1)),
                )
                .add(
                    Expr::col((folder::Entity, folder::Column::Name))
                        .equals((RequestedSegments::Table, RequestedSegments::Column2)),
                ),
        )
        .and_where(Expr::col((folder::Entity, folder::Column::UserId)).eq(user_id))
        .and_where(Expr::col((folder::Entity, folder::Column::TeamId)).is_null())
        .and_where(Expr::col((folder::Entity, folder::Column::DeletedAt)).is_null())
        .to_owned();

    let folder_chain_cte = CommonTableExpression::new()
        .table_name(FolderChain::Table)
        .columns([
            FolderChain::SegmentIndex,
            FolderChain::Id,
            FolderChain::Name,
            FolderChain::ParentId,
            FolderChain::TeamId,
            FolderChain::UserId,
            FolderChain::PolicyId,
            FolderChain::CreatedAt,
            FolderChain::UpdatedAt,
            FolderChain::DeletedAt,
            FolderChain::IsLocked,
        ])
        .query(
            base_select
                .union(UnionType::All, recursive_select)
                .to_owned(),
        )
        .to_owned();

    let final_select = Query::select()
        .column((FolderChain::Table, FolderChain::SegmentIndex))
        .column((FolderChain::Table, FolderChain::Id))
        .column((FolderChain::Table, FolderChain::Name))
        .column((FolderChain::Table, FolderChain::ParentId))
        .column((FolderChain::Table, FolderChain::TeamId))
        .column((FolderChain::Table, FolderChain::UserId))
        .column((FolderChain::Table, FolderChain::PolicyId))
        .column((FolderChain::Table, FolderChain::CreatedAt))
        .column((FolderChain::Table, FolderChain::UpdatedAt))
        .column((FolderChain::Table, FolderChain::DeletedAt))
        .column((FolderChain::Table, FolderChain::IsLocked))
        .from(FolderChain::Table)
        .order_by((FolderChain::Table, FolderChain::SegmentIndex), Order::Asc)
        .to_owned();

    let with_clause = WithClause::new()
        .recursive(true)
        .cte(folder_chain_cte)
        .to_owned();

    with_clause.query(final_select)
}

pub async fn find_by_id<C: ConnectionTrait>(db: &C, id: i64) -> Result<folder::Model> {
    Folder::find_by_id(id)
        .one(db)
        .await
        .map_err(AsterError::from)?
        .ok_or_else(|| AsterError::record_not_found(format!("folder #{id}")))
}

pub async fn find_by_ids<C: ConnectionTrait>(db: &C, ids: &[i64]) -> Result<Vec<folder::Model>> {
    if ids.is_empty() {
        return Ok(vec![]);
    }
    Folder::find()
        .filter(folder::Column::Id.is_in(ids.iter().copied()))
        .all(db)
        .await
        .map_err(AsterError::from)
}

async fn find_children_in_scope<C: ConnectionTrait>(
    db: &C,
    scope: FolderScope,
    parent_id: Option<i64>,
) -> Result<Vec<folder::Model>> {
    Folder::find()
        .filter(apply_parent_condition(
            active_scope_condition(scope),
            parent_id,
        ))
        .order_by_asc(folder::Column::Name)
        .all(db)
        .await
        .map_err(AsterError::from)
}

/// 查询子文件夹（排除已删除）
pub async fn find_children<C: ConnectionTrait>(
    db: &C,
    user_id: i64,
    parent_id: Option<i64>,
) -> Result<Vec<folder::Model>> {
    find_children_in_scope(db, FolderScope::Personal { user_id }, parent_id).await
}

pub async fn find_team_children<C: ConnectionTrait>(
    db: &C,
    team_id: i64,
    parent_id: Option<i64>,
) -> Result<Vec<folder::Model>> {
    find_children_in_scope(db, FolderScope::Team { team_id }, parent_id).await
}

/// 批量查询多个父文件夹下的未删除子文件夹
async fn find_children_in_parents_in_scope<C: ConnectionTrait>(
    db: &C,
    scope: FolderScope,
    parent_ids: &[i64],
) -> Result<Vec<folder::Model>> {
    if parent_ids.is_empty() {
        return Ok(vec![]);
    }
    Folder::find()
        .filter(active_scope_condition(scope))
        .filter(folder::Column::ParentId.is_in(parent_ids.iter().copied()))
        .all(db)
        .await
        .map_err(AsterError::from)
}

pub async fn find_children_in_parents<C: ConnectionTrait>(
    db: &C,
    user_id: i64,
    parent_ids: &[i64],
) -> Result<Vec<folder::Model>> {
    find_children_in_parents_in_scope(db, FolderScope::Personal { user_id }, parent_ids).await
}

pub async fn find_team_children_in_parents<C: ConnectionTrait>(
    db: &C,
    team_id: i64,
    parent_ids: &[i64],
) -> Result<Vec<folder::Model>> {
    find_children_in_parents_in_scope(db, FolderScope::Team { team_id }, parent_ids).await
}

/// 查询子文件夹（排除已删除，分页）
async fn find_children_paginated_in_scope<C: ConnectionTrait>(
    db: &C,
    scope: FolderScope,
    parent_id: Option<i64>,
    limit: u64,
    offset: u64,
    sort_by: crate::api::pagination::SortBy,
    sort_order: crate::api::pagination::SortOrder,
) -> Result<(Vec<folder::Model>, u64)> {
    let base = Folder::find().filter(apply_parent_condition(
        active_scope_condition(scope),
        parent_id,
    ));

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

pub async fn find_children_paginated<C: ConnectionTrait>(
    db: &C,
    user_id: i64,
    parent_id: Option<i64>,
    limit: u64,
    offset: u64,
    sort_by: crate::api::pagination::SortBy,
    sort_order: crate::api::pagination::SortOrder,
) -> Result<(Vec<folder::Model>, u64)> {
    find_children_paginated_in_scope(
        db,
        FolderScope::Personal { user_id },
        parent_id,
        limit,
        offset,
        sort_by,
        sort_order,
    )
    .await
}

pub async fn find_team_children_paginated<C: ConnectionTrait>(
    db: &C,
    team_id: i64,
    parent_id: Option<i64>,
    limit: u64,
    offset: u64,
    sort_by: crate::api::pagination::SortBy,
    sort_order: crate::api::pagination::SortOrder,
) -> Result<(Vec<folder::Model>, u64)> {
    find_children_paginated_in_scope(
        db,
        FolderScope::Team { team_id },
        parent_id,
        limit,
        offset,
        sort_by,
        sort_order,
    )
    .await
}

/// 查询顶层已删除文件夹（分页），用 SQL 过滤而非内存过滤
fn top_level_deleted_condition(scope: FolderScope) -> Condition {
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

    scope_condition(scope)
        .add(folder::Column::DeletedAt.is_not_null())
        .add(
            Condition::any()
                .add(folder::Column::ParentId.is_null())
                .add(Expr::exists(parent_deleted_subquery).not()),
        )
}

async fn find_top_level_deleted_paginated_in_scope<C: ConnectionTrait>(
    db: &C,
    scope: FolderScope,
    limit: u64,
    offset: u64,
) -> Result<(Vec<folder::Model>, u64)> {
    let base = Folder::find().filter(top_level_deleted_condition(scope));

    let total = base.clone().count(db).await.map_err(AsterError::from)?;
    if total == 0 || limit == 0 {
        return Ok((vec![], total));
    }

    let items = base
        .order_by_desc(folder::Column::DeletedAt)
        .order_by_desc(folder::Column::Id)
        .offset(offset)
        .limit(limit)
        .all(db)
        .await
        .map_err(AsterError::from)?;

    Ok((items, total))
}

pub async fn find_top_level_deleted_paginated<C: ConnectionTrait>(
    db: &C,
    user_id: i64,
    limit: u64,
    offset: u64,
) -> Result<(Vec<folder::Model>, u64)> {
    find_top_level_deleted_paginated_in_scope(db, FolderScope::Personal { user_id }, limit, offset)
        .await
}

pub async fn find_top_level_deleted_by_team_paginated<C: ConnectionTrait>(
    db: &C,
    team_id: i64,
    limit: u64,
    offset: u64,
) -> Result<(Vec<folder::Model>, u64)> {
    find_top_level_deleted_paginated_in_scope(db, FolderScope::Team { team_id }, limit, offset)
        .await
}

/// 按名称查文件夹（排除已删除）
async fn find_by_name_in_parent_in_scope<C: ConnectionTrait>(
    db: &C,
    scope: FolderScope,
    parent_id: Option<i64>,
    name: &str,
) -> Result<Option<folder::Model>> {
    Folder::find()
        .filter(apply_parent_condition(
            active_scope_condition(scope),
            parent_id,
        ))
        .filter(folder::Column::Name.eq(name))
        .one(db)
        .await
        .map_err(AsterError::from)
}

pub async fn find_by_name_in_parent<C: ConnectionTrait>(
    db: &C,
    user_id: i64,
    parent_id: Option<i64>,
    name: &str,
) -> Result<Option<folder::Model>> {
    find_by_name_in_parent_in_scope(db, FolderScope::Personal { user_id }, parent_id, name).await
}

pub async fn find_by_name_in_team_parent<C: ConnectionTrait>(
    db: &C,
    team_id: i64,
    parent_id: Option<i64>,
    name: &str,
) -> Result<Option<folder::Model>> {
    find_by_name_in_parent_in_scope(db, FolderScope::Team { team_id }, parent_id, name).await
}

/// 批量解析路径前缀中的文件夹链，避免逐段 round-trip。
///
/// `resolve_path_chain` only resolves personal user-space folders.
/// `build_resolve_path_chain_query` applies `TeamId IS NULL`, so team-root
/// paths will not match here. Callers resolving team folders must use a
/// different team-aware code path instead of this helper.
///
/// 返回已成功匹配的文件夹链；如果中途断开，只返回前缀中已匹配的部分。
pub async fn resolve_path_chain<C: ConnectionTrait>(
    db: &C,
    user_id: i64,
    root_parent_id: Option<i64>,
    segments: &[String],
) -> Result<Vec<folder::Model>> {
    if segments.is_empty() {
        return Ok(vec![]);
    }

    // The recursive walk keeps hitting idx_folders_user_deleted_parent_name instead of issuing
    // one query per path segment.
    let rows = Folder::find()
        .from_raw_sql(
            db.get_database_backend()
                .build(&build_resolve_path_chain_query(
                    user_id,
                    root_parent_id,
                    segments,
                )),
        )
        .into_model::<ResolvedPathFolderRow>()
        .all(db)
        .await
        .map_err(AsterError::from)?;

    Ok(rows.into_iter().map(Into::into).collect())
}

pub async fn create<C: ConnectionTrait>(
    db: &C,
    model: folder::ActiveModel,
) -> Result<folder::Model> {
    model.insert(db).await.map_err(AsterError::from)
}

/// 批量移动文件夹到同一父文件夹
pub async fn move_many_to_parent<C: ConnectionTrait>(
    db: &C,
    ids: &[i64],
    parent_id: Option<i64>,
    now: chrono::DateTime<Utc>,
) -> Result<()> {
    if ids.is_empty() {
        return Ok(());
    }
    Folder::update_many()
        .col_expr(
            folder::Column::ParentId,
            sea_orm::sea_query::Expr::value(parent_id),
        )
        .col_expr(
            folder::Column::UpdatedAt,
            sea_orm::sea_query::Expr::value(now),
        )
        .filter(folder::Column::Id.is_in(ids.to_vec()))
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(())
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

/// 批量查询多个父文件夹下的子文件夹（含已删除）
pub async fn find_all_children_in_parents<C: ConnectionTrait>(
    db: &C,
    parent_ids: &[i64],
) -> Result<Vec<folder::Model>> {
    if parent_ids.is_empty() {
        return Ok(vec![]);
    }
    Folder::find()
        .filter(folder::Column::ParentId.is_in(parent_ids.to_vec()))
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
        .filter(folder::Column::TeamId.is_null())
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

pub async fn find_all_by_team<C: ConnectionTrait>(
    db: &C,
    team_id: i64,
) -> Result<Vec<folder::Model>> {
    Folder::find()
        .filter(folder::Column::TeamId.eq(team_id))
        .all(db)
        .await
        .map_err(AsterError::from)
}
