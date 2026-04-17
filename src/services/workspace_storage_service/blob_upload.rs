use sea_orm::ConnectionTrait;
use std::path::{Component, Path, PathBuf};

use crate::entities::file_blob;
use crate::errors::Result;
use crate::types::DriverType;

use super::{create_nondedup_blob_with_key, create_s3_nondedup_blob};

#[derive(Debug, Clone)]
pub(crate) enum PreparedNonDedupBlobUpload {
    Local {
        base_path: PathBuf,
        blob_key: String,
        storage_path: String,
        size: i64,
        policy_id: i64,
    },
    S3 {
        upload_id: String,
        storage_path: String,
        size: i64,
        policy_id: i64,
    },
}

impl PreparedNonDedupBlobUpload {
    pub(crate) fn storage_path(&self) -> &str {
        match self {
            Self::Local { storage_path, .. } | Self::S3 { storage_path, .. } => storage_path,
        }
    }
}

pub(crate) fn prepare_non_dedup_blob_upload(
    policy: &crate::entities::storage_policy::Model,
    size: i64,
) -> PreparedNonDedupBlobUpload {
    if policy.driver_type == DriverType::S3 {
        let upload_id = crate::utils::id::new_uuid();
        PreparedNonDedupBlobUpload::S3 {
            storage_path: format!("files/{upload_id}"),
            upload_id,
            size,
            policy_id: policy.id,
        }
    } else {
        let blob_key = crate::utils::id::new_short_token();
        PreparedNonDedupBlobUpload::Local {
            base_path: crate::storage::drivers::local::effective_base_path(policy),
            storage_path: crate::utils::storage_path_from_blob_key(&blob_key),
            blob_key,
            size,
            policy_id: policy.id,
        }
    }
}

fn normalize_absolute_cleanup_path(path: &Path) -> Option<PathBuf> {
    if !path.is_absolute() {
        return None;
    }

    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    return None;
                }
            }
            Component::Normal(part) => normalized.push(part),
        }
    }

    Some(normalized)
}

fn normalize_cleanup_root(path: &Path) -> Option<PathBuf> {
    if path.is_absolute() {
        return normalize_absolute_cleanup_path(path);
    }

    let current_dir = std::env::current_dir().ok()?;
    normalize_absolute_cleanup_path(&current_dir.join(path))
}

async fn cleanup_empty_local_blob_dirs(prefix_dir: &Path, root_dir: &Path) {
    let Some(mut current) = normalize_cleanup_root(prefix_dir) else {
        tracing::warn!(
            "skip blob dir cleanup for unresolved prefix {}",
            prefix_dir.display()
        );
        return;
    };
    let Some(root_dir) = normalize_cleanup_root(root_dir) else {
        tracing::warn!(
            "skip blob dir cleanup for unresolved root {}",
            root_dir.display()
        );
        return;
    };

    if current == root_dir || !current.starts_with(&root_dir) {
        tracing::warn!(
            "skip blob dir cleanup outside storage root: prefix={}, root={}",
            current.display(),
            root_dir.display()
        );
        return;
    }

    while current != root_dir {
        match tokio::fs::remove_dir(&current).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) if error.kind() == std::io::ErrorKind::DirectoryNotEmpty => break,
            Err(error) => {
                tracing::warn!("failed to cleanup blob dir {}: {error}", current.display());
                break;
            }
        }

        let Some(parent) = current.parent() else {
            break;
        };
        current = parent.to_path_buf();
    }
}

pub(crate) async fn cleanup_preuploaded_blob_upload(
    driver: &dyn crate::storage::driver::StorageDriver,
    prepared: &PreparedNonDedupBlobUpload,
    reason: &str,
) {
    match prepared {
        PreparedNonDedupBlobUpload::Local {
            base_path,
            storage_path,
            ..
        } => {
            let full_path = base_path.join(storage_path.trim_start_matches('/'));
            match tokio::fs::remove_file(&full_path).await {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    tracing::warn!(
                        storage_path = %storage_path,
                        full_path = %full_path.display(),
                        "failed to cleanup preuploaded local blob after {reason}: {error}"
                    );
                    return;
                }
            }

            if let Some(parent) = full_path.parent() {
                cleanup_empty_local_blob_dirs(parent, base_path).await;
            }
        }
        PreparedNonDedupBlobUpload::S3 { .. } => {
            if let Err(cleanup_err) = driver.delete(prepared.storage_path()).await {
                tracing::warn!(
                    storage_path = %prepared.storage_path(),
                    "failed to cleanup preuploaded blob after {reason}: {cleanup_err}"
                );
            }
        }
    }
}

pub(crate) async fn upload_temp_file_to_prepared_blob(
    driver: &dyn crate::storage::driver::StorageDriver,
    prepared: &PreparedNonDedupBlobUpload,
    temp_path: &str,
) -> Result<()> {
    if let Err(error) = driver.put_file(prepared.storage_path(), temp_path).await {
        cleanup_preuploaded_blob_upload(driver, prepared, "upload error").await;
        return Err(error);
    }

    Ok(())
}

pub(crate) async fn persist_preuploaded_blob<C: ConnectionTrait>(
    db: &C,
    prepared: &PreparedNonDedupBlobUpload,
) -> Result<file_blob::Model> {
    match prepared {
        PreparedNonDedupBlobUpload::Local {
            blob_key,
            storage_path,
            size,
            policy_id,
            ..
        } => create_nondedup_blob_with_key(db, *size, *policy_id, blob_key, storage_path).await,
        PreparedNonDedupBlobUpload::S3 {
            upload_id,
            size,
            policy_id,
            ..
        } => create_s3_nondedup_blob(db, *size, *policy_id, upload_id).await,
    }
}
