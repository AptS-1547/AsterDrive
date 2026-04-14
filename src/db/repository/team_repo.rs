use crate::db::repository::pagination_repo::fetch_offset_page;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, ConnectionTrait, DbBackend, EntityTrait, ExprTrait,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, sea_query::Expr,
};

use crate::entities::team::{self, Entity as Team};
use crate::errors::{AsterError, Result};

pub async fn create<C: ConnectionTrait>(db: &C, model: team::ActiveModel) -> Result<team::Model> {
    model.insert(db).await.map_err(AsterError::from)
}

pub async fn update<C: ConnectionTrait>(db: &C, model: team::ActiveModel) -> Result<team::Model> {
    model.update(db).await.map_err(AsterError::from)
}

pub async fn find_by_id<C: ConnectionTrait>(db: &C, id: i64) -> Result<team::Model> {
    Team::find_by_id(id)
        .one(db)
        .await
        .map_err(AsterError::from)?
        .ok_or_else(|| AsterError::record_not_found(format!("team #{id}")))
}

pub async fn find_active_by_id<C: ConnectionTrait>(db: &C, id: i64) -> Result<team::Model> {
    Team::find()
        .filter(team::Column::Id.eq(id))
        .filter(team::Column::ArchivedAt.is_null())
        .one(db)
        .await
        .map_err(AsterError::from)?
        .ok_or_else(|| AsterError::record_not_found(format!("team #{id}")))
}

pub async fn find_archived_by_id<C: ConnectionTrait>(db: &C, id: i64) -> Result<team::Model> {
    Team::find()
        .filter(team::Column::Id.eq(id))
        .filter(team::Column::ArchivedAt.is_not_null())
        .one(db)
        .await
        .map_err(AsterError::from)?
        .ok_or_else(|| AsterError::record_not_found(format!("team #{id}")))
}

pub async fn lock_active_by_id<C: ConnectionTrait>(db: &C, id: i64) -> Result<team::Model> {
    match db.get_database_backend() {
        DbBackend::Postgres | DbBackend::MySql => Team::find()
            .filter(team::Column::Id.eq(id))
            .filter(team::Column::ArchivedAt.is_null())
            .lock_exclusive()
            .one(db)
            .await
            .map_err(AsterError::from)?
            .ok_or_else(|| AsterError::record_not_found(format!("team #{id}"))),
        DbBackend::Sqlite => {
            Team::update_many()
                .col_expr(team::Column::UpdatedAt, Expr::col(team::Column::UpdatedAt))
                .filter(team::Column::Id.eq(id))
                .filter(team::Column::ArchivedAt.is_null())
                .exec(db)
                .await
                .map_err(AsterError::from)?;
            find_active_by_id(db, id).await
        }
        _ => find_active_by_id(db, id).await,
    }
}

pub async fn lock_archived_by_id<C: ConnectionTrait>(db: &C, id: i64) -> Result<team::Model> {
    match db.get_database_backend() {
        DbBackend::Postgres | DbBackend::MySql => Team::find()
            .filter(team::Column::Id.eq(id))
            .filter(team::Column::ArchivedAt.is_not_null())
            .lock_exclusive()
            .one(db)
            .await
            .map_err(AsterError::from)?
            .ok_or_else(|| AsterError::record_not_found(format!("team #{id}"))),
        DbBackend::Sqlite => {
            Team::update_many()
                .col_expr(team::Column::UpdatedAt, Expr::col(team::Column::UpdatedAt))
                .filter(team::Column::Id.eq(id))
                .filter(team::Column::ArchivedAt.is_not_null())
                .exec(db)
                .await
                .map_err(AsterError::from)?;
            find_archived_by_id(db, id).await
        }
        _ => find_archived_by_id(db, id).await,
    }
}

pub async fn find_all<C: ConnectionTrait>(db: &C) -> Result<Vec<team::Model>> {
    Team::find()
        .order_by_asc(team::Column::Id)
        .all(db)
        .await
        .map_err(AsterError::from)
}

pub async fn find_active_paginated<C: ConnectionTrait>(
    db: &C,
    limit: u64,
    offset: u64,
    keyword: Option<&str>,
) -> Result<(Vec<team::Model>, u64)> {
    find_paginated_by_archived_state(db, limit, offset, keyword, false).await
}

pub async fn find_archived_paginated<C: ConnectionTrait>(
    db: &C,
    limit: u64,
    offset: u64,
    keyword: Option<&str>,
) -> Result<(Vec<team::Model>, u64)> {
    find_paginated_by_archived_state(db, limit, offset, keyword, true).await
}

async fn find_paginated_by_archived_state<C: ConnectionTrait>(
    db: &C,
    limit: u64,
    offset: u64,
    keyword: Option<&str>,
    archived: bool,
) -> Result<(Vec<team::Model>, u64)> {
    let mut q = Team::find().order_by_asc(team::Column::Id);

    q = if archived {
        q.filter(team::Column::ArchivedAt.is_not_null())
    } else {
        q.filter(team::Column::ArchivedAt.is_null())
    };

    if let Some(keyword) = keyword.filter(|s| !s.trim().is_empty()) {
        let pattern = format!("%{}%", keyword.trim());
        q = q.filter(
            Condition::any()
                .add(team::Column::Name.like(&pattern))
                .add(team::Column::Description.like(&pattern)),
        );
    }

    fetch_offset_page(db, q, limit, offset).await
}

pub async fn find_archived_before<C: ConnectionTrait>(
    db: &C,
    before: chrono::DateTime<chrono::Utc>,
) -> Result<Vec<team::Model>> {
    Team::find()
        .filter(team::Column::ArchivedAt.is_not_null())
        .filter(team::Column::ArchivedAt.lt(before))
        .order_by_asc(team::Column::ArchivedAt)
        .order_by_asc(team::Column::Id)
        .all(db)
        .await
        .map_err(AsterError::from)
}

pub async fn delete<C: ConnectionTrait>(db: &C, id: i64) -> Result<()> {
    let result = Team::delete_by_id(id)
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    if result.rows_affected == 0 {
        return Err(AsterError::record_not_found(format!("team #{id}")));
    }
    Ok(())
}

pub async fn count_active_by_policy_group<C: ConnectionTrait>(
    db: &C,
    policy_group_id: i64,
) -> Result<u64> {
    Team::find()
        .filter(team::Column::ArchivedAt.is_null())
        .filter(team::Column::PolicyGroupId.eq(policy_group_id))
        .count(db)
        .await
        .map_err(AsterError::from)
}

pub async fn check_quota<C: ConnectionTrait>(db: &C, team_id: i64, needed_size: i64) -> Result<()> {
    let team = find_active_by_id(db, team_id).await?;
    let projected_storage_used = team.storage_used.checked_add(needed_size).ok_or_else(|| {
        AsterError::internal_error(format!(
            "team storage usage overflow: used {}, delta {}",
            team.storage_used, needed_size
        ))
    })?;
    if team.storage_quota > 0 && projected_storage_used > team.storage_quota {
        return Err(AsterError::storage_quota_exceeded(format!(
            "team quota {}, used {}, need {}",
            team.storage_quota, team.storage_used, needed_size
        )));
    }
    Ok(())
}

pub async fn update_storage_used<C: ConnectionTrait>(db: &C, id: i64, delta: i64) -> Result<()> {
    let expr = if delta >= 0 {
        Expr::col(team::Column::StorageUsed).add(delta)
    } else {
        let decrement_by = -delta;
        Expr::case(Expr::col(team::Column::StorageUsed).lt(decrement_by), 0)
            .finally(Expr::col(team::Column::StorageUsed).sub(decrement_by))
            .into()
    };

    let mut query = Team::update_many()
        .col_expr(team::Column::StorageUsed, expr)
        .filter(team::Column::Id.eq(id));

    if delta >= 0 {
        query = query.filter(
            Condition::any().add(team::Column::StorageQuota.eq(0)).add(
                Expr::col(team::Column::StorageUsed)
                    .add(delta)
                    .lte(Expr::col(team::Column::StorageQuota)),
            ),
        );
    }

    let result = query.exec(db).await.map_err(AsterError::from)?;

    if result.rows_affected == 0 {
        if delta >= 0 {
            let team = find_by_id(db, id).await?;
            let projected_storage_used = team.storage_used.checked_add(delta).ok_or_else(|| {
                AsterError::internal_error(format!(
                    "team storage usage overflow: used {}, delta {}",
                    team.storage_used, delta
                ))
            })?;
            if team.storage_quota > 0 && projected_storage_used > team.storage_quota {
                return Err(AsterError::storage_quota_exceeded(format!(
                    "team quota {}, used {}, need {}",
                    team.storage_quota, team.storage_used, delta
                )));
            }
        }
        return Err(AsterError::record_not_found(format!("team #{id}")));
    }

    Ok(())
}

pub async fn set_storage_used<C: ConnectionTrait>(db: &C, id: i64, value: i64) -> Result<()> {
    let result = Team::update_many()
        .col_expr(team::Column::StorageUsed, Expr::value(value))
        .filter(team::Column::Id.eq(id))
        .exec(db)
        .await
        .map_err(AsterError::from)?;

    if result.rows_affected == 0 {
        return Err(AsterError::record_not_found(format!("team #{id}")));
    }

    Ok(())
}
