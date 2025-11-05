pub mod traits;
pub mod local;
pub mod s3;

pub use traits::{StorageBackend, StorageError, StorageResult};
pub use local::LocalStorage;
pub use s3::S3Storage;

use crate::config::StorageConfig;
use std::sync::Arc;

pub async fn create_storage_backend(config: &StorageConfig) -> StorageResult<Arc<dyn StorageBackend>> {
    match config.backend.as_str() {
        "local" => {
            let path = config.local_path.clone()
                .unwrap_or_else(|| std::path::PathBuf::from("./data/uploads"));
            let storage = LocalStorage::new(path).await?;
            Ok(Arc::new(storage))
        }
        "s3" => {
            let bucket = config.s3_bucket.clone()
                .ok_or_else(|| StorageError::Backend("S3 bucket not configured".to_string()))?;
            let storage = S3Storage::new(
                bucket,
                config.s3_region.clone(),
                config.s3_endpoint.clone(),
            ).await?;
            Ok(Arc::new(storage))
        }
        _ => Err(StorageError::Backend(format!("Unknown storage backend: {}", config.backend))),
    }
}
