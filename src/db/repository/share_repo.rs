use crate::entities::share::{self, Entity as Share};
use crate::errors::{AsterError, Result};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, sea_query::Expr,
};

pub async fn find_by_id<C: ConnectionTrait>(db: &C, id: i64) -> Result<share::Model> {
    Share::find_by_id(id)
        .one(db)
        .await
        .map_err(AsterError::from)?
        .ok_or_else(|| AsterError::share_not_found(format!("share #{id}")))
}

pub async fn find_by_token<C: ConnectionTrait>(
    db: &C,
    token: &str,
) -> Result<Option<share::Model>> {
    Share::find()
        .filter(share::Column::Token.eq(token))
        .one(db)
        .await
        .map_err(AsterError::from)
}

pub async fn find_by_user<C: ConnectionTrait>(db: &C, user_id: i64) -> Result<Vec<share::Model>> {
    Share::find()
        .filter(share::Column::UserId.eq(user_id))
        .all(db)
        .await
        .map_err(AsterError::from)
}

pub async fn find_all<C: ConnectionTrait>(db: &C) -> Result<Vec<share::Model>> {
    Share::find().all(db).await.map_err(AsterError::from)
}

/// 查找用户对同一资源是否已有活跃分享
pub async fn find_active_by_resource<C: ConnectionTrait>(
    db: &C,
    user_id: i64,
    file_id: Option<i64>,
    folder_id: Option<i64>,
) -> Result<Option<share::Model>> {
    let mut q = Share::find().filter(share::Column::UserId.eq(user_id));
    if let Some(fid) = file_id {
        q = q.filter(share::Column::FileId.eq(fid));
    }
    if let Some(fid) = folder_id {
        q = q.filter(share::Column::FolderId.eq(fid));
    }
    q.one(db).await.map_err(AsterError::from)
}

pub async fn create<C: ConnectionTrait>(db: &C, model: share::ActiveModel) -> Result<share::Model> {
    model.insert(db).await.map_err(AsterError::from)
}

pub async fn delete<C: ConnectionTrait>(db: &C, id: i64) -> Result<()> {
    Share::delete_by_id(id)
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(())
}

/// 原子递增 view_count
pub async fn increment_view_count<C: ConnectionTrait>(db: &C, id: i64) -> Result<()> {
    Share::update_many()
        .col_expr(
            share::Column::ViewCount,
            Expr::cust_with_values("view_count + ?", [1i64]),
        )
        .filter(share::Column::Id.eq(id))
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(())
}

/// 原子递增 download_count，同时校验下载限制。
/// 返回 false 表示已达上限未递增。
pub async fn increment_download_count<C: ConnectionTrait>(db: &C, id: i64) -> Result<bool> {
    let result = Share::update_many()
        .col_expr(
            share::Column::DownloadCount,
            Expr::cust_with_values("download_count + ?", [1i64]),
        )
        .filter(share::Column::Id.eq(id))
        // 只在未达上限时递增（max_downloads=0 表示不限）
        .filter(Expr::cust(
            "max_downloads = 0 OR download_count < max_downloads",
        ))
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(result.rows_affected > 0)
}

/// 批量删除用户的所有分享链接
pub async fn delete_all_by_user<C: ConnectionTrait>(db: &C, user_id: i64) -> Result<u64> {
    let res = Share::delete_many()
        .filter(share::Column::UserId.eq(user_id))
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(res.rows_affected)
}
