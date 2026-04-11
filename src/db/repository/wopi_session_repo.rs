use crate::entities::wopi_session::{self, Entity as WopiSession};
use crate::errors::{AsterError, Result};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};

pub async fn create<C: ConnectionTrait>(
    db: &C,
    model: wopi_session::ActiveModel,
) -> Result<wopi_session::Model> {
    model.insert(db).await.map_err(AsterError::from)
}

pub async fn find_by_token_hash<C: ConnectionTrait>(
    db: &C,
    token_hash: &str,
) -> Result<Option<wopi_session::Model>> {
    WopiSession::find()
        .filter(wopi_session::Column::TokenHash.eq(token_hash))
        .one(db)
        .await
        .map_err(AsterError::from)
}

pub async fn delete_by_id<C: ConnectionTrait>(db: &C, id: i64) -> Result<()> {
    WopiSession::delete_by_id(id)
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(())
}

pub async fn delete_expired<C: ConnectionTrait>(db: &C) -> Result<u64> {
    let result = WopiSession::delete_many()
        .filter(wopi_session::Column::ExpiresAt.lt(Utc::now()))
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(result.rows_affected)
}
