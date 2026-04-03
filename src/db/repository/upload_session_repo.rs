use crate::entities::upload_session::{self, Entity as UploadSession};
use crate::errors::{AsterError, Result};
use crate::types::UploadSessionStatus;
use sea_orm::{ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};

pub async fn find_by_id<C: ConnectionTrait>(db: &C, id: &str) -> Result<upload_session::Model> {
    UploadSession::find_by_id(id.to_string())
        .one(db)
        .await
        .map_err(AsterError::from)?
        .ok_or_else(|| AsterError::upload_session_not_found(format!("session {id}")))
}

pub async fn create<C: ConnectionTrait>(
    db: &C,
    model: upload_session::ActiveModel,
) -> Result<upload_session::Model> {
    model.insert(db).await.map_err(AsterError::from)
}

pub async fn update<C: ConnectionTrait>(
    db: &C,
    model: upload_session::ActiveModel,
) -> Result<upload_session::Model> {
    model.update(db).await.map_err(AsterError::from)
}

pub async fn delete<C: ConnectionTrait>(db: &C, id: &str) -> Result<()> {
    UploadSession::delete_by_id(id.to_string())
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(())
}

/// 原子状态转换：只有当前状态匹配 expected 时才更新为 new_status。
/// 返回转换是否成功（false = 状态已被其他请求抢占）。
pub async fn try_transition_status<C: ConnectionTrait>(
    db: &C,
    id: &str,
    expected: UploadSessionStatus,
    new_status: UploadSessionStatus,
) -> Result<bool> {
    use sea_orm::ActiveEnum;
    let result = UploadSession::update_many()
        .col_expr(
            upload_session::Column::Status,
            sea_orm::sea_query::Expr::value(new_status.to_value()),
        )
        .col_expr(
            upload_session::Column::UpdatedAt,
            sea_orm::sea_query::Expr::value(chrono::Utc::now()),
        )
        .filter(upload_session::Column::Id.eq(id))
        .filter(upload_session::Column::Status.eq(expected))
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(result.rows_affected > 0)
}

/// 查找所有过期且未完成的 session
pub async fn find_expired<C: ConnectionTrait>(db: &C) -> Result<Vec<upload_session::Model>> {
    let now = chrono::Utc::now();
    UploadSession::find()
        .filter(upload_session::Column::ExpiresAt.lt(now))
        .filter(upload_session::Column::Status.ne("completed"))
        .all(db)
        .await
        .map_err(AsterError::from)
}

pub async fn find_by_team<C: ConnectionTrait>(
    db: &C,
    team_id: i64,
) -> Result<Vec<upload_session::Model>> {
    UploadSession::find()
        .filter(upload_session::Column::TeamId.eq(team_id))
        .all(db)
        .await
        .map_err(AsterError::from)
}

/// 批量删除用户的所有上传会话
pub async fn delete_all_by_user<C: ConnectionTrait>(db: &C, user_id: i64) -> Result<u64> {
    let res = UploadSession::delete_many()
        .filter(upload_session::Column::UserId.eq(user_id))
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(res.rows_affected)
}

pub async fn delete_all_by_team<C: ConnectionTrait>(db: &C, team_id: i64) -> Result<u64> {
    let res = UploadSession::delete_many()
        .filter(upload_session::Column::TeamId.eq(team_id))
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(res.rows_affected)
}
