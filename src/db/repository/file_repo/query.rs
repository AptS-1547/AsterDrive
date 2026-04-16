use sea_orm::{
    ColumnTrait, Condition, ConnectionTrait, DbBackend, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect,
};

use crate::api::pagination::{SortBy, SortOrder};
use crate::entities::file::{self, Entity as File};
use crate::errors::{AsterError, MapAsterErr, Result};

use super::common::{FileScope, active_scope_condition, apply_folder_condition, scope_condition};

async fn find_by_folders_in_scope<C: ConnectionTrait>(
    db: &C,
    scope: FileScope,
    folder_ids: &[i64],
) -> Result<Vec<file::Model>> {
    if folder_ids.is_empty() {
        return Ok(vec![]);
    }
    File::find()
        .filter(active_scope_condition(scope))
        .filter(file::Column::FolderId.is_in(folder_ids.iter().copied()))
        .all(db)
        .await
        .map_err(AsterError::from)
}

async fn find_by_folder_in_scope<C: ConnectionTrait>(
    db: &C,
    scope: FileScope,
    folder_id: Option<i64>,
) -> Result<Vec<file::Model>> {
    File::find()
        .filter(apply_folder_condition(
            active_scope_condition(scope),
            folder_id,
        ))
        .order_by_asc(file::Column::Name)
        .all(db)
        .await
        .map_err(AsterError::from)
}

pub async fn find_by_id<C: ConnectionTrait>(db: &C, id: i64) -> Result<file::Model> {
    File::find_by_id(id)
        .one(db)
        .await
        .map_err(AsterError::from)?
        .ok_or_else(|| AsterError::file_not_found(format!("file #{id}")))
}

pub async fn lock_by_id<C: ConnectionTrait>(db: &C, id: i64) -> Result<file::Model> {
    match db.get_database_backend() {
        DbBackend::Postgres | DbBackend::MySql => File::find_by_id(id)
            .lock_exclusive()
            .one(db)
            .await
            .map_err(AsterError::from)?
            .ok_or_else(|| AsterError::file_not_found(format!("file #{id}"))),
        DbBackend::Sqlite => find_by_id(db, id).await,
        _ => find_by_id(db, id).await,
    }
}

pub async fn find_by_ids<C: ConnectionTrait>(db: &C, ids: &[i64]) -> Result<Vec<file::Model>> {
    if ids.is_empty() {
        return Ok(vec![]);
    }
    File::find()
        .filter(file::Column::Id.is_in(ids.iter().copied()))
        .all(db)
        .await
        .map_err(AsterError::from)
}

async fn find_by_ids_in_scope<C: ConnectionTrait>(
    db: &C,
    scope: FileScope,
    ids: &[i64],
) -> Result<Vec<file::Model>> {
    if ids.is_empty() {
        return Ok(vec![]);
    }
    File::find()
        .filter(scope_condition(scope))
        .filter(file::Column::Id.is_in(ids.iter().copied()))
        .all(db)
        .await
        .map_err(AsterError::from)
}

pub async fn find_by_ids_in_personal_scope<C: ConnectionTrait>(
    db: &C,
    user_id: i64,
    ids: &[i64],
) -> Result<Vec<file::Model>> {
    find_by_ids_in_scope(db, FileScope::Personal { user_id }, ids).await
}

pub async fn find_by_ids_in_team_scope<C: ConnectionTrait>(
    db: &C,
    team_id: i64,
    ids: &[i64],
) -> Result<Vec<file::Model>> {
    find_by_ids_in_scope(db, FileScope::Team { team_id }, ids).await
}

/// 批量查询多个文件夹下的未删除文件
pub async fn find_by_folders<C: ConnectionTrait>(
    db: &C,
    user_id: i64,
    folder_ids: &[i64],
) -> Result<Vec<file::Model>> {
    find_by_folders_in_scope(db, FileScope::Personal { user_id }, folder_ids).await
}

pub async fn find_by_team_folders<C: ConnectionTrait>(
    db: &C,
    team_id: i64,
    folder_ids: &[i64],
) -> Result<Vec<file::Model>> {
    find_by_folders_in_scope(db, FileScope::Team { team_id }, folder_ids).await
}

/// 批量查询多个文件夹下的文件（含已删除）
pub async fn find_all_in_folders<C: ConnectionTrait>(
    db: &C,
    folder_ids: &[i64],
) -> Result<Vec<file::Model>> {
    if folder_ids.is_empty() {
        return Ok(vec![]);
    }
    File::find()
        .filter(file::Column::FolderId.is_in(folder_ids.to_vec()))
        .all(db)
        .await
        .map_err(AsterError::from)
}

/// 查询文件夹下的文件（排除已删除）
pub async fn find_by_folder<C: ConnectionTrait>(
    db: &C,
    user_id: i64,
    folder_id: Option<i64>,
) -> Result<Vec<file::Model>> {
    find_by_folder_in_scope(db, FileScope::Personal { user_id }, folder_id).await
}

pub async fn find_by_team_folder<C: ConnectionTrait>(
    db: &C,
    team_id: i64,
    folder_id: Option<i64>,
) -> Result<Vec<file::Model>> {
    find_by_folder_in_scope(db, FileScope::Team { team_id }, folder_id).await
}

/// 查询文件夹下的文件（排除已删除，cursor 分页，支持多字段排序）
async fn find_by_folder_cursor_in_scope<C: ConnectionTrait>(
    db: &C,
    scope: FileScope,
    folder_id: Option<i64>,
    limit: u64,
    after: Option<(String, i64)>,
    sort_by: SortBy,
    sort_order: SortOrder,
) -> Result<(Vec<file::Model>, u64)> {
    let base = File::find().filter(apply_folder_condition(
        active_scope_condition(scope),
        folder_id,
    ));
    let total = base.clone().count(db).await.map_err(AsterError::from)?;

    if total == 0 || limit == 0 {
        return Ok((vec![], total));
    }

    let is_asc = matches!(sort_order, SortOrder::Asc);

    let mut q = base;
    if let Some((after_value, after_id)) = after {
        let cursor_cond = build_cursor_condition(sort_by, is_asc, &after_value, after_id)?;
        q = q.filter(cursor_cond);
    }

    let primary_col = match sort_by {
        SortBy::Name => file::Column::Name,
        SortBy::Size => file::Column::Size,
        SortBy::CreatedAt => file::Column::CreatedAt,
        SortBy::UpdatedAt => file::Column::UpdatedAt,
        SortBy::Type => file::Column::MimeType,
    };

    q = if is_asc {
        q.order_by_asc(primary_col).order_by_asc(file::Column::Id)
    } else {
        q.order_by_desc(primary_col).order_by_desc(file::Column::Id)
    };

    let items = q.limit(limit).all(db).await.map_err(AsterError::from)?;
    Ok((items, total))
}

pub async fn find_by_folder_cursor<C: ConnectionTrait>(
    db: &C,
    user_id: i64,
    folder_id: Option<i64>,
    limit: u64,
    after: Option<(String, i64)>,
    sort_by: SortBy,
    sort_order: SortOrder,
) -> Result<(Vec<file::Model>, u64)> {
    find_by_folder_cursor_in_scope(
        db,
        FileScope::Personal { user_id },
        folder_id,
        limit,
        after,
        sort_by,
        sort_order,
    )
    .await
}

pub async fn find_by_team_folder_cursor<C: ConnectionTrait>(
    db: &C,
    team_id: i64,
    folder_id: Option<i64>,
    limit: u64,
    after: Option<(String, i64)>,
    sort_by: SortBy,
    sort_order: SortOrder,
) -> Result<(Vec<file::Model>, u64)> {
    find_by_folder_cursor_in_scope(
        db,
        FileScope::Team { team_id },
        folder_id,
        limit,
        after,
        sort_by,
        sort_order,
    )
    .await
}

/// 构建 cursor WHERE 条件
/// ASC:  (col > val) OR (col = val AND id > after_id)
/// DESC: (col < val) OR (col = val AND id < after_id)
fn build_cursor_condition(
    sort_by: SortBy,
    is_asc: bool,
    after_value: &str,
    after_id: i64,
) -> Result<Condition> {
    let id_cond = if is_asc {
        file::Column::Id.gt(after_id)
    } else {
        file::Column::Id.lt(after_id)
    };

    match sort_by {
        SortBy::Name => {
            let val = after_value.to_string();
            let (gt, eq) = if is_asc {
                (
                    file::Column::Name.gt(val.clone()),
                    file::Column::Name.eq(val),
                )
            } else {
                (
                    file::Column::Name.lt(val.clone()),
                    file::Column::Name.eq(val),
                )
            };
            Ok(Condition::any()
                .add(gt)
                .add(Condition::all().add(eq).add(id_cond)))
        }
        SortBy::Size => {
            let val: i64 = after_value.parse().map_aster_err_with(|| {
                AsterError::validation_error("invalid cursor value for size sort")
            })?;
            let (gt, eq) = if is_asc {
                (file::Column::Size.gt(val), file::Column::Size.eq(val))
            } else {
                (file::Column::Size.lt(val), file::Column::Size.eq(val))
            };
            Ok(Condition::any()
                .add(gt)
                .add(Condition::all().add(eq).add(id_cond)))
        }
        SortBy::CreatedAt => {
            let val: chrono::DateTime<chrono::Utc> =
                after_value.parse().map_aster_err_with(|| {
                    AsterError::validation_error("invalid cursor value for created_at sort")
                })?;
            let (gt, eq) = if is_asc {
                (
                    file::Column::CreatedAt.gt(val),
                    file::Column::CreatedAt.eq(val),
                )
            } else {
                (
                    file::Column::CreatedAt.lt(val),
                    file::Column::CreatedAt.eq(val),
                )
            };
            Ok(Condition::any()
                .add(gt)
                .add(Condition::all().add(eq).add(id_cond)))
        }
        SortBy::UpdatedAt => {
            let val: chrono::DateTime<chrono::Utc> =
                after_value.parse().map_aster_err_with(|| {
                    AsterError::validation_error("invalid cursor value for updated_at sort")
                })?;
            let (gt, eq) = if is_asc {
                (
                    file::Column::UpdatedAt.gt(val),
                    file::Column::UpdatedAt.eq(val),
                )
            } else {
                (
                    file::Column::UpdatedAt.lt(val),
                    file::Column::UpdatedAt.eq(val),
                )
            };
            Ok(Condition::any()
                .add(gt)
                .add(Condition::all().add(eq).add(id_cond)))
        }
        SortBy::Type => {
            let val = after_value.to_string();
            let (gt, eq) = if is_asc {
                (
                    file::Column::MimeType.gt(val.clone()),
                    file::Column::MimeType.eq(val),
                )
            } else {
                (
                    file::Column::MimeType.lt(val.clone()),
                    file::Column::MimeType.eq(val),
                )
            };
            Ok(Condition::any()
                .add(gt)
                .add(Condition::all().add(eq).add(id_cond)))
        }
    }
}

/// 按名称查文件（排除已删除）
async fn find_by_name_in_folder_in_scope<C: ConnectionTrait>(
    db: &C,
    scope: FileScope,
    folder_id: Option<i64>,
    name: &str,
) -> Result<Option<file::Model>> {
    File::find()
        .filter(apply_folder_condition(
            active_scope_condition(scope),
            folder_id,
        ))
        .filter(file::Column::Name.eq(name))
        .one(db)
        .await
        .map_err(AsterError::from)
}

async fn find_by_names_in_folder_in_scope<C: ConnectionTrait>(
    db: &C,
    scope: FileScope,
    folder_id: Option<i64>,
    names: &[String],
) -> Result<Vec<file::Model>> {
    if names.is_empty() {
        return Ok(vec![]);
    }

    File::find()
        .filter(apply_folder_condition(
            active_scope_condition(scope),
            folder_id,
        ))
        .filter(file::Column::Name.is_in(names.iter().cloned()))
        .all(db)
        .await
        .map_err(AsterError::from)
}

pub async fn find_by_name_in_folder<C: ConnectionTrait>(
    db: &C,
    user_id: i64,
    folder_id: Option<i64>,
    name: &str,
) -> Result<Option<file::Model>> {
    find_by_name_in_folder_in_scope(db, FileScope::Personal { user_id }, folder_id, name).await
}

pub async fn find_by_name_in_team_folder<C: ConnectionTrait>(
    db: &C,
    team_id: i64,
    folder_id: Option<i64>,
    name: &str,
) -> Result<Option<file::Model>> {
    find_by_name_in_folder_in_scope(db, FileScope::Team { team_id }, folder_id, name).await
}

pub async fn find_by_names_in_folder<C: ConnectionTrait>(
    db: &C,
    user_id: i64,
    folder_id: Option<i64>,
    names: &[String],
) -> Result<Vec<file::Model>> {
    find_by_names_in_folder_in_scope(db, FileScope::Personal { user_id }, folder_id, names).await
}

pub async fn find_by_names_in_team_folder<C: ConnectionTrait>(
    db: &C,
    team_id: i64,
    folder_id: Option<i64>,
    names: &[String],
) -> Result<Vec<file::Model>> {
    find_by_names_in_folder_in_scope(db, FileScope::Team { team_id }, folder_id, names).await
}

/// 查找不冲突的文件名：如果 name 已存在则递增 " (1)", " (2)" ...
async fn resolve_unique_filename_in_scope<C: ConnectionTrait>(
    db: &C,
    scope: FileScope,
    folder_id: Option<i64>,
    name: &str,
) -> Result<String> {
    let mut final_name = name.to_string();
    while find_by_name_in_folder_in_scope(db, scope, folder_id, &final_name)
        .await?
        .is_some()
    {
        final_name = crate::utils::next_copy_name(&final_name);
    }
    Ok(final_name)
}

pub async fn resolve_unique_filename<C: ConnectionTrait>(
    db: &C,
    user_id: i64,
    folder_id: Option<i64>,
    name: &str,
) -> Result<String> {
    resolve_unique_filename_in_scope(db, FileScope::Personal { user_id }, folder_id, name).await
}

pub async fn resolve_unique_team_filename<C: ConnectionTrait>(
    db: &C,
    team_id: i64,
    folder_id: Option<i64>,
    name: &str,
) -> Result<String> {
    resolve_unique_filename_in_scope(db, FileScope::Team { team_id }, folder_id, name).await
}
