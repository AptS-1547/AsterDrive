use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, Set,
};

use crate::entities::upload_session_part::{self, Entity as UploadSessionPart};
use crate::errors::{AsterError, Result};

pub async fn upsert_part<C: ConnectionTrait>(
    db: &C,
    upload_id: &str,
    part_number: i32,
    etag: &str,
    size: i64,
) -> Result<upload_session_part::Model> {
    if let Some(existing) = UploadSessionPart::find()
        .filter(upload_session_part::Column::UploadId.eq(upload_id))
        .filter(upload_session_part::Column::PartNumber.eq(part_number))
        .one(db)
        .await
        .map_err(AsterError::from)?
    {
        let mut active: upload_session_part::ActiveModel = existing.into();
        active.etag = Set(etag.to_string());
        active.size = Set(size);
        active.updated_at = Set(Utc::now());
        return active.update(db).await.map_err(AsterError::from);
    }

    upload_session_part::ActiveModel {
        upload_id: Set(upload_id.to_string()),
        part_number: Set(part_number),
        etag: Set(etag.to_string()),
        size: Set(size),
        created_at: Set(Utc::now()),
        updated_at: Set(Utc::now()),
        ..Default::default()
    }
    .insert(db)
    .await
    .map_err(AsterError::from)
}

pub async fn find_by_upload_and_part<C: ConnectionTrait>(
    db: &C,
    upload_id: &str,
    part_number: i32,
) -> Result<Option<upload_session_part::Model>> {
    UploadSessionPart::find()
        .filter(upload_session_part::Column::UploadId.eq(upload_id))
        .filter(upload_session_part::Column::PartNumber.eq(part_number))
        .one(db)
        .await
        .map_err(AsterError::from)
}

pub async fn list_by_upload<C: ConnectionTrait>(
    db: &C,
    upload_id: &str,
) -> Result<Vec<upload_session_part::Model>> {
    UploadSessionPart::find()
        .filter(upload_session_part::Column::UploadId.eq(upload_id))
        .order_by_asc(upload_session_part::Column::PartNumber)
        .all(db)
        .await
        .map_err(AsterError::from)
}

pub async fn list_part_numbers<C: ConnectionTrait>(db: &C, upload_id: &str) -> Result<Vec<i32>> {
    Ok(list_by_upload(db, upload_id)
        .await?
        .into_iter()
        .map(|part| part.part_number)
        .collect())
}

pub async fn delete_by_upload<C: ConnectionTrait>(db: &C, upload_id: &str) -> Result<u64> {
    let res = UploadSessionPart::delete_many()
        .filter(upload_session_part::Column::UploadId.eq(upload_id))
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(res.rows_affected)
}
