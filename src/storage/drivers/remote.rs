//! 存储驱动实现：`remote`。

use crate::entities::{managed_follower, storage_policy};
use crate::errors::{AsterError, Result};
use crate::storage::driver::{BlobMetadata, PresignedDownloadOptions, StorageDriver};
use crate::storage::error::{StorageErrorKind, storage_driver_error};
use crate::storage::extensions::{ListStorageDriver, PresignedStorageDriver, StreamUploadDriver};
use crate::storage::multipart::MultipartStorageDriver;
use crate::storage::remote_protocol::RemoteStorageClient;
use async_trait::async_trait;
use std::path::Path;
use std::time::Duration;
use tokio::io::AsyncRead;

pub struct RemoteDriver {
    client: RemoteStorageClient,
    base_path: String,
}

impl RemoteDriver {
    const MULTIPART_UPLOADS_PREFIX: &str = "uploads";

    pub fn new(policy: &storage_policy::Model, follower: &managed_follower::Model) -> Result<Self> {
        Ok(Self {
            client: RemoteStorageClient::new(
                &follower.base_url,
                &follower.access_key,
                &follower.secret_key,
            )?,
            base_path: policy.base_path.trim_matches('/').to_string(),
        })
    }

    fn object_key(&self, path: &str) -> String {
        let path = path.trim_start_matches('/');
        if self.base_path.is_empty() {
            path.to_string()
        } else if path.is_empty() {
            self.base_path.clone()
        } else {
            format!("{}/{}", self.base_path, path)
        }
    }

    fn strip_base_path<'a>(&self, object_key: &'a str) -> Option<&'a str> {
        if self.base_path.is_empty() {
            return Some(object_key.trim_start_matches('/'));
        }

        object_key
            .strip_prefix(&self.base_path)
            .map(|suffix| suffix.trim_start_matches('/'))
            .or_else(|| (object_key == self.base_path).then_some(""))
    }

    fn multipart_parts_prefix(upload_id: &str) -> String {
        format!("{}/{upload_id}/parts", Self::MULTIPART_UPLOADS_PREFIX)
    }

    fn multipart_part_key(upload_id: &str, part_number: i32) -> Result<String> {
        if part_number <= 0 {
            return Err(AsterError::validation_error(format!(
                "multipart part_number must be positive, got {part_number}"
            )));
        }
        Ok(format!(
            "{}/{}",
            Self::multipart_parts_prefix(upload_id),
            part_number
        ))
    }
}

#[async_trait]
impl StorageDriver for RemoteDriver {
    async fn put(&self, path: &str, data: &[u8]) -> Result<String> {
        self.client.put_bytes(&self.object_key(path), data).await?;
        Ok(path.to_string())
    }

    async fn get(&self, path: &str) -> Result<Vec<u8>> {
        self.client.get_bytes(&self.object_key(path)).await
    }

    async fn get_stream(&self, path: &str) -> Result<Box<dyn AsyncRead + Unpin + Send>> {
        self.client
            .get_stream(&self.object_key(path), None, None)
            .await
    }

    async fn get_range(
        &self,
        path: &str,
        offset: u64,
        length: Option<u64>,
    ) -> Result<Box<dyn AsyncRead + Unpin + Send>> {
        self.client
            .get_stream(&self.object_key(path), Some(offset), length)
            .await
    }

    async fn delete(&self, path: &str) -> Result<()> {
        self.client.delete(&self.object_key(path)).await
    }

    async fn exists(&self, path: &str) -> Result<bool> {
        self.client.exists(&self.object_key(path)).await
    }

    async fn metadata(&self, path: &str) -> Result<BlobMetadata> {
        self.client.metadata(&self.object_key(path)).await
    }

    fn as_list(&self) -> Option<&dyn ListStorageDriver> {
        Some(self)
    }

    fn as_stream_upload(&self) -> Option<&dyn StreamUploadDriver> {
        Some(self)
    }

    fn as_presigned(&self) -> Option<&dyn PresignedStorageDriver> {
        Some(self)
    }

    fn as_multipart(&self) -> Option<&dyn MultipartStorageDriver> {
        Some(self)
    }
}

#[async_trait]
impl ListStorageDriver for RemoteDriver {
    async fn list_paths(&self, prefix: Option<&str>) -> Result<Vec<String>> {
        let full_prefix = prefix.map(|value| self.object_key(value));
        let paths = self.client.list_paths(full_prefix.as_deref()).await?;
        Ok(paths
            .into_iter()
            .filter_map(|path| self.strip_base_path(&path).map(str::to_string))
            .collect())
    }
}

#[async_trait]
impl StreamUploadDriver for RemoteDriver {
    async fn put_reader(
        &self,
        storage_path: &str,
        reader: Box<dyn AsyncRead + Unpin + Send + Sync>,
        size: i64,
    ) -> Result<String> {
        let size = u64::try_from(size).map_err(|_| {
            storage_driver_error(
                StorageErrorKind::Precondition,
                format!("remote stream upload size must be non-negative, got {size}"),
            )
        })?;
        self.client
            .put_reader(&self.object_key(storage_path), reader, size)
            .await?;
        Ok(storage_path.to_string())
    }

    async fn put_file(&self, storage_path: &str, local_path: &str) -> Result<String> {
        let metadata = tokio::fs::metadata(local_path).await.map_err(|e| {
            AsterError::storage_driver_error(format!("remote put_file metadata: {e}"))
        })?;
        let file = tokio::fs::File::open(Path::new(local_path))
            .await
            .map_err(|e| AsterError::storage_driver_error(format!("remote put_file open: {e}")))?;
        self.put_reader(
            storage_path,
            Box::new(file),
            i64::try_from(metadata.len()).map_err(|_| {
                storage_driver_error(
                    StorageErrorKind::Precondition,
                    "remote put_file size exceeds i64 range",
                )
            })?,
        )
        .await
    }
}

#[async_trait]
impl PresignedStorageDriver for RemoteDriver {
    async fn presigned_url(
        &self,
        path: &str,
        expires: Duration,
        options: PresignedDownloadOptions,
    ) -> Result<Option<String>> {
        self.client
            .presigned_url(&self.object_key(path), expires, options)
            .map(Some)
    }

    async fn presigned_put_url(&self, path: &str, expires: Duration) -> Result<Option<String>> {
        self.client
            .presigned_put_url(&self.object_key(path), expires)
            .map(Some)
    }
}

#[async_trait]
impl MultipartStorageDriver for RemoteDriver {
    async fn create_multipart_upload(&self, _path: &str) -> Result<String> {
        Ok(crate::utils::id::new_uuid())
    }

    async fn presigned_upload_part_url(
        &self,
        _path: &str,
        upload_id: &str,
        part_number: i32,
        expires: Duration,
    ) -> Result<String> {
        let part_key = Self::multipart_part_key(upload_id, part_number)?;
        self.client
            .presigned_put_url(&self.object_key(&part_key), expires)
    }

    async fn complete_multipart_upload(
        &self,
        path: &str,
        upload_id: &str,
        mut parts: Vec<(i32, String)>,
    ) -> Result<()> {
        if parts.is_empty() {
            return Err(AsterError::validation_error(
                "multipart completion requires at least one part",
            ));
        }

        parts.sort_by_key(|(part_number, _)| *part_number);
        let mut expected_size = 0i64;
        let mut part_keys = Vec::with_capacity(parts.len());
        for (part_number, _) in parts {
            let part_key = Self::multipart_part_key(upload_id, part_number)?;
            let remote_key = self.object_key(&part_key);
            let metadata = self.client.metadata(&remote_key).await?;
            let part_size = i64::try_from(metadata.size).map_err(|_| {
                storage_driver_error(
                    StorageErrorKind::Precondition,
                    "remote multipart part size exceeds i64 range",
                )
            })?;
            expected_size = expected_size.checked_add(part_size).ok_or_else(|| {
                storage_driver_error(
                    StorageErrorKind::Precondition,
                    "remote multipart expected size overflow",
                )
            })?;
            part_keys.push(remote_key);
        }

        self.client
            .compose_objects(&self.object_key(path), part_keys, expected_size)
            .await?;
        Ok(())
    }

    async fn upload_multipart_part(
        &self,
        _path: &str,
        upload_id: &str,
        part_number: i32,
        data: &[u8],
    ) -> Result<String> {
        let part_key = Self::multipart_part_key(upload_id, part_number)?;
        self.client
            .put_bytes(&self.object_key(&part_key), data)
            .await?;

        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(data);
        Ok(format!("\"{}\"", hex::encode(hasher.finalize())))
    }

    async fn abort_multipart_upload(&self, _path: &str, upload_id: &str) -> Result<()> {
        let prefix = Self::multipart_parts_prefix(upload_id);
        let parts = self.list_paths(Some(&prefix)).await?;
        for part_path in parts {
            self.client.delete(&self.object_key(&part_path)).await?;
        }
        Ok(())
    }

    async fn list_uploaded_parts(&self, _path: &str, upload_id: &str) -> Result<Vec<i32>> {
        let prefix = Self::multipart_parts_prefix(upload_id);
        let mut parts = self
            .list_paths(Some(&prefix))
            .await?
            .into_iter()
            .filter_map(|path| {
                path.rsplit('/')
                    .next()
                    .and_then(|segment| segment.parse::<i32>().ok())
            })
            .collect::<Vec<_>>();
        parts.sort_unstable();
        parts.dedup();
        Ok(parts)
    }
}
