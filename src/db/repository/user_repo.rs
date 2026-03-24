use crate::entities::user::{self, Entity as User};
use crate::errors::{AsterError, Result};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, PaginatorTrait, QueryFilter,
    sea_query::Expr,
};

pub async fn find_by_id<C: ConnectionTrait>(db: &C, id: i64) -> Result<user::Model> {
    User::find_by_id(id)
        .one(db)
        .await
        .map_err(AsterError::from)?
        .ok_or_else(|| AsterError::record_not_found(format!("user #{id}")))
}

pub async fn find_by_username<C: ConnectionTrait>(
    db: &C,
    username: &str,
) -> Result<Option<user::Model>> {
    User::find()
        .filter(user::Column::Username.eq(username))
        .one(db)
        .await
        .map_err(AsterError::from)
}

pub async fn find_by_email<C: ConnectionTrait>(db: &C, email: &str) -> Result<Option<user::Model>> {
    User::find()
        .filter(user::Column::Email.eq(email))
        .one(db)
        .await
        .map_err(AsterError::from)
}

pub async fn find_all<C: ConnectionTrait>(db: &C) -> Result<Vec<user::Model>> {
    User::find().all(db).await.map_err(AsterError::from)
}

pub async fn count_all<C: ConnectionTrait>(db: &C) -> Result<u64> {
    User::find().count(db).await.map_err(AsterError::from)
}

pub async fn create<C: ConnectionTrait>(db: &C, model: user::ActiveModel) -> Result<user::Model> {
    model.insert(db).await.map_err(AsterError::from)
}

/// 检查用户配额是否足够。quota=0 表示不限。
pub async fn check_quota<C: ConnectionTrait>(db: &C, user_id: i64, needed_size: i64) -> Result<()> {
    let user = find_by_id(db, user_id).await?;
    if user.storage_quota > 0 && user.storage_used + needed_size > user.storage_quota {
        return Err(AsterError::storage_quota_exceeded(format!(
            "quota {}, used {}, need {}",
            user.storage_quota, user.storage_used, needed_size
        )));
    }
    Ok(())
}

pub async fn update_storage_used<C: ConnectionTrait>(db: &C, id: i64, delta: i64) -> Result<()> {
    let expr = if delta >= 0 {
        Expr::cust_with_values("storage_used + ?", [delta])
    } else {
        Expr::cust_with_values(
            "CASE WHEN storage_used < ? THEN 0 ELSE storage_used - ? END",
            [-delta, -delta],
        )
    };

    let result = User::update_many()
        .col_expr(user::Column::StorageUsed, expr)
        .filter(user::Column::Id.eq(id))
        .exec(db)
        .await
        .map_err(AsterError::from)?;

    if result.rows_affected == 0 {
        return Err(AsterError::record_not_found(format!("user #{id}")));
    }

    Ok(())
}
