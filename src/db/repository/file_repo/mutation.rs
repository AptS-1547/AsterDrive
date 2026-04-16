use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, sea_query::Expr,
};

use crate::entities::file::{self, Entity as File};
use crate::errors::{AsterError, Result};

use super::common::map_bulk_name_db_err;

pub async fn create<C: ConnectionTrait>(db: &C, model: file::ActiveModel) -> Result<file::Model> {
    model.insert(db).await.map_err(AsterError::from)
}

/// 批量插入文件记录（不返回创建的 Model，批量复制用）
pub async fn create_many<C: ConnectionTrait>(db: &C, models: Vec<file::ActiveModel>) -> Result<()> {
    if models.is_empty() {
        return Ok(());
    }
    File::insert_many(models).exec(db).await.map_err(|err| {
        map_bulk_name_db_err(err, "one or more files already exist in this folder")
    })?;
    Ok(())
}

/// 批量移动文件到同一文件夹
pub async fn move_many_to_folder<C: ConnectionTrait>(
    db: &C,
    ids: &[i64],
    folder_id: Option<i64>,
    now: chrono::DateTime<Utc>,
) -> Result<()> {
    if ids.is_empty() {
        return Ok(());
    }
    File::update_many()
        .col_expr(file::Column::FolderId, Expr::value(folder_id))
        .col_expr(file::Column::UpdatedAt, Expr::value(now))
        .filter(file::Column::Id.is_in(ids.to_vec()))
        .exec(db)
        .await
        .map_err(|err| {
            map_bulk_name_db_err(err, "one or more files already exist in target folder")
        })?;
    Ok(())
}
