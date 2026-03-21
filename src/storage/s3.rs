use super::driver::{BlobMetadata, StorageDriver};
use crate::entities::storage_policy;
use crate::errors::{AsterError, Result};
use async_trait::async_trait;
use aws_credential_types::Credentials;
use aws_sdk_s3::Client;
use aws_sdk_s3::config::{BehaviorVersion, Region};
use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::primitives::ByteStream;
use std::time::Duration;
use tokio::io::AsyncRead;

pub struct S3Driver {
    client: Client,
    bucket: String,
    base_path: String,
}

impl S3Driver {
    pub fn new(policy: &storage_policy::Model) -> Result<Self> {
        let credentials = Credentials::new(
            &policy.access_key,
            &policy.secret_key,
            None,
            None,
            "aster-s3-driver",
        );

        let mut config_builder = aws_sdk_s3::Config::builder()
            .behavior_version(BehaviorVersion::latest())
            .region(Region::new("auto"))
            .credentials_provider(credentials)
            .force_path_style(true); // MinIO / R2 等需要

        // 自定义 endpoint（MinIO、R2、OSS 等）
        if !policy.endpoint.is_empty() {
            config_builder = config_builder.endpoint_url(&policy.endpoint);
        }

        let config = config_builder.build();
        let client = Client::from_conf(config);

        let bucket = if policy.bucket.is_empty() {
            return Err(AsterError::storage_driver_error(
                "S3 bucket name is required",
            ));
        } else {
            policy.bucket.clone()
        };

        Ok(Self {
            client,
            bucket,
            base_path: policy.base_path.clone(),
        })
    }

    fn full_key(&self, path: &str) -> String {
        if self.base_path.is_empty() {
            path.trim_start_matches('/').to_string()
        } else {
            format!(
                "{}/{}",
                self.base_path.trim_end_matches('/'),
                path.trim_start_matches('/')
            )
        }
    }
}

#[async_trait]
impl StorageDriver for S3Driver {
    async fn put(&self, path: &str, data: &[u8]) -> Result<String> {
        let key = self.full_key(path);
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(&key)
            .body(ByteStream::from(data.to_vec()))
            .send()
            .await
            .map_err(|e| AsterError::storage_driver_error(format!("S3 put failed: {e}")))?;
        Ok(path.to_string())
    }

    async fn get(&self, path: &str) -> Result<Vec<u8>> {
        let key = self.full_key(path);
        let resp = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await
            .map_err(|e| AsterError::storage_driver_error(format!("S3 get failed: {e}")))?;

        let bytes = resp
            .body
            .collect()
            .await
            .map_err(|e| AsterError::storage_driver_error(format!("S3 read body failed: {e}")))?
            .into_bytes();

        Ok(bytes.to_vec())
    }

    async fn get_stream(&self, path: &str) -> Result<Box<dyn AsyncRead + Unpin + Send>> {
        let key = self.full_key(path);
        let resp = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await
            .map_err(|e| AsterError::storage_driver_error(format!("S3 get_stream failed: {e}")))?;

        Ok(Box::new(resp.body.into_async_read()))
    }

    async fn delete(&self, path: &str) -> Result<()> {
        let key = self.full_key(path);
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await
            .map_err(|e| AsterError::storage_driver_error(format!("S3 delete failed: {e}")))?;
        Ok(())
    }

    async fn exists(&self, path: &str) -> Result<bool> {
        let key = self.full_key(path);
        match self
            .client
            .head_object()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await
        {
            Ok(_) => Ok(true),
            Err(e) => {
                let svc_err = e.into_service_error();
                if svc_err.is_not_found() {
                    Ok(false)
                } else {
                    Err(AsterError::storage_driver_error(format!(
                        "S3 exists check failed: {svc_err}"
                    )))
                }
            }
        }
    }

    async fn metadata(&self, path: &str) -> Result<BlobMetadata> {
        let key = self.full_key(path);
        let resp = self
            .client
            .head_object()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await
            .map_err(|e| AsterError::storage_driver_error(format!("S3 head failed: {e}")))?;

        Ok(BlobMetadata {
            size: resp.content_length.unwrap_or(0) as u64,
            content_type: resp.content_type,
        })
    }

    async fn put_file(&self, storage_path: &str, local_path: &str) -> Result<String> {
        let key = self.full_key(storage_path);
        let body = ByteStream::from_path(local_path)
            .await
            .map_err(|e| AsterError::storage_driver_error(format!("S3 read file: {e}")))?;
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(&key)
            .body(body)
            .send()
            .await
            .map_err(|e| AsterError::storage_driver_error(format!("S3 put_file failed: {e}")))?;
        Ok(storage_path.to_string())
    }

    async fn presigned_url(&self, path: &str, expires: Duration) -> Result<Option<String>> {
        let key = self.full_key(path);
        let presign_config = PresigningConfig::builder()
            .expires_in(expires)
            .build()
            .map_err(|e| AsterError::storage_driver_error(format!("presign config: {e}")))?;

        let url = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(&key)
            .presigned(presign_config)
            .await
            .map_err(|e| {
                AsterError::storage_driver_error(format!("S3 presigned URL failed: {e}"))
            })?;

        Ok(Some(url.uri().to_string()))
    }

    async fn presigned_put_url(&self, path: &str, expires: Duration) -> Result<Option<String>> {
        let key = self.full_key(path);
        let presign_config = PresigningConfig::builder()
            .expires_in(expires)
            .build()
            .map_err(|e| AsterError::storage_driver_error(format!("presign config: {e}")))?;

        let url = self
            .client
            .put_object()
            .bucket(&self.bucket)
            .key(&key)
            .presigned(presign_config)
            .await
            .map_err(|e| {
                AsterError::storage_driver_error(format!("S3 presigned PUT failed: {e}"))
            })?;

        Ok(Some(url.uri().to_string()))
    }

    async fn copy_object(&self, src_path: &str, dest_path: &str) -> Result<String> {
        let src_key = self.full_key(src_path);
        let dest_key = self.full_key(dest_path);
        let copy_source = format!("{}/{}", self.bucket, src_key);

        self.client
            .copy_object()
            .bucket(&self.bucket)
            .copy_source(&copy_source)
            .key(&dest_key)
            .send()
            .await
            .map_err(|e| AsterError::storage_driver_error(format!("S3 copy_object failed: {e}")))?;

        Ok(dest_path.to_string())
    }
}
