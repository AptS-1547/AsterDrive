use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, ExprTrait, QueryFilter,
    QueryOrder, sea_query::Expr,
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

pub async fn find_all<C: ConnectionTrait>(db: &C) -> Result<Vec<team::Model>> {
    Team::find()
        .order_by_asc(team::Column::Id)
        .all(db)
        .await
        .map_err(AsterError::from)
}

pub async fn check_quota<C: ConnectionTrait>(db: &C, team_id: i64, needed_size: i64) -> Result<()> {
    let team = find_active_by_id(db, team_id).await?;
    if team.storage_quota > 0 && team.storage_used + needed_size > team.storage_quota {
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

    let result = Team::update_many()
        .col_expr(team::Column::StorageUsed, expr)
        .filter(team::Column::Id.eq(id))
        .exec(db)
        .await
        .map_err(AsterError::from)?;

    if result.rows_affected == 0 {
        return Err(AsterError::record_not_found(format!("team #{id}")));
    }

    Ok(())
}
