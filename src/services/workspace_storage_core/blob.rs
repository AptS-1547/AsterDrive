use chrono::Utc;
use sea_orm::{ConnectionTrait, Set};

use crate::db::repository::file_repo;
use crate::entities::file_blob;
use crate::errors::Result;

pub(crate) async fn create_nondedup_blob_with_key<C: ConnectionTrait>(
    db: &C,
    size: i64,
    policy_id: i64,
    blob_key: &str,
    storage_path: &str,
) -> Result<file_blob::Model> {
    let now = Utc::now();

    file_repo::create_blob(
        db,
        file_blob::ActiveModel {
            hash: Set(blob_key.to_string()),
            size: Set(size),
            policy_id: Set(policy_id),
            storage_path: Set(storage_path.to_string()),
            ref_count: Set(1),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        },
    )
    .await
}

pub(crate) async fn create_nondedup_blob<C: ConnectionTrait>(
    db: &C,
    size: i64,
    policy_id: i64,
) -> Result<file_blob::Model> {
    let blob_key = crate::utils::id::new_short_token();
    let storage_path = crate::utils::storage_path_from_blob_key(&blob_key);

    create_nondedup_blob_with_key(db, size, policy_id, &blob_key, &storage_path).await
}

pub(crate) async fn create_s3_nondedup_blob<C: ConnectionTrait>(
    db: &C,
    size: i64,
    policy_id: i64,
    upload_id: &str,
) -> Result<file_blob::Model> {
    create_opaque_nondedup_blob(db, size, policy_id, "s3", upload_id).await
}

pub(crate) async fn create_remote_nondedup_blob<C: ConnectionTrait>(
    db: &C,
    size: i64,
    policy_id: i64,
    upload_id: &str,
) -> Result<file_blob::Model> {
    create_opaque_nondedup_blob(db, size, policy_id, "remote", upload_id).await
}

async fn create_opaque_nondedup_blob<C: ConnectionTrait>(
    db: &C,
    size: i64,
    policy_id: i64,
    hash_prefix: &str,
    object_id: &str,
) -> Result<file_blob::Model> {
    let now = Utc::now();
    let file_hash = format!("{hash_prefix}-{object_id}");
    let storage_path = format!("files/{object_id}");

    file_repo::create_blob(
        db,
        file_blob::ActiveModel {
            hash: Set(file_hash),
            size: Set(size),
            policy_id: Set(policy_id),
            storage_path: Set(storage_path),
            ref_count: Set(1),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        },
    )
    .await
}
