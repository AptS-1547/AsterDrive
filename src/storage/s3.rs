use super::traits::{StorageBackend, StorageError, StorageResult};
use async_trait::async_trait;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::error::SdkError;
use aws_sdk_s3::operation::get_object::GetObjectError;
use aws_sdk_s3::operation::head_object::HeadObjectError;
use aws_sdk_s3::Client;
use bytes::Bytes;
use tokio::io::AsyncRead;

pub struct S3Storage {
    client: Client,
    bucket: String,
}

impl S3Storage {
    pub async fn new(bucket: String, region: Option<String>, endpoint: Option<String>) -> StorageResult<Self> {
        let mut config_loader = aws_config::defaults(aws_config::BehaviorVersion::latest());
        
        if let Some(region) = region {
            config_loader = config_loader.region(aws_config::Region::new(region));
        }
        
        let sdk_config = config_loader.load().await;
        
        let client = if let Some(endpoint) = endpoint {
            // Custom endpoint configuration (e.g., MinIO)
            let credentials = sdk_config.credentials_provider()
                .ok_or_else(|| StorageError::Backend("No credentials provider configured".to_string()))?;
            
            let custom_config = aws_config::SdkConfig::builder()
                .endpoint_url(endpoint)
                .region(sdk_config.region().cloned())
                .credentials_provider(credentials.clone())
                .build();
            
            Client::new(&custom_config)
        } else {
            Client::new(&sdk_config)
        };
        
        Ok(Self { client, bucket })
    }
}

#[async_trait]
impl StorageBackend for S3Storage {
    async fn store(&self, path: &str, data: Bytes) -> StorageResult<String> {
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(path)
            .body(ByteStream::from(data))
            .send()
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))?;
        
        Ok(path.to_string())
    }
    
    async fn store_stream(
        &self,
        path: &str,
        mut stream: Box<dyn AsyncRead + Send + Unpin>,
        size: u64,
    ) -> StorageResult<String> {
        // For small files, read into memory. For large files, consider implementing
        // multipart upload for better performance and reliability.
        const MAX_MEMORY_SIZE: u64 = 100 * 1024 * 1024; // 100MB
        
        if size > MAX_MEMORY_SIZE {
            tracing::warn!(
                "Uploading large file ({} bytes) by reading into memory. Consider implementing multipart upload.",
                size
            );
        }
        
        let mut buffer = Vec::with_capacity(size as usize);
        tokio::io::AsyncReadExt::read_to_end(&mut stream, &mut buffer).await?;
        
        let byte_stream = ByteStream::from(buffer);
        
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(path)
            .body(byte_stream)
            .send()
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))?;
        
        Ok(path.to_string())
    }
    
    async fn retrieve(&self, path: &str) -> StorageResult<Bytes> {
        let response = self.client
            .get_object()
            .bucket(&self.bucket)
            .key(path)
            .send()
            .await
            .map_err(|e| {
                // Check if it's a NoSuchKey error using proper error types
                if let SdkError::ServiceError(ref service_err) = e {
                    if matches!(service_err.err(), GetObjectError::NoSuchKey(_)) {
                        return StorageError::NotFound(path.to_string());
                    }
                }
                StorageError::Backend(e.to_string())
            })?;
        
        let data = response.body.collect().await
            .map_err(|e| StorageError::Backend(e.to_string()))?;
        
        Ok(data.into_bytes())
    }
    
    async fn delete(&self, path: &str) -> StorageResult<()> {
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(path)
            .send()
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))?;
        
        Ok(())
    }
    
    async fn exists(&self, path: &str) -> StorageResult<bool> {
        match self.client
            .head_object()
            .bucket(&self.bucket)
            .key(path)
            .send()
            .await
        {
            Ok(_) => Ok(true),
            Err(e) => {
                // Check if it's a NotFound error using proper error types
                if let SdkError::ServiceError(ref service_err) = e {
                    if matches!(service_err.err(), HeadObjectError::NotFound(_)) {
                        return Ok(false);
                    }
                }
                Err(StorageError::Backend(e.to_string()))
            }
        }
    }
    
    async fn size(&self, path: &str) -> StorageResult<u64> {
        let response = self.client
            .head_object()
            .bucket(&self.bucket)
            .key(path)
            .send()
            .await
            .map_err(|e| {
                // Check if it's a NotFound error using proper error types
                if let SdkError::ServiceError(ref service_err) = e {
                    if matches!(service_err.err(), HeadObjectError::NotFound(_)) {
                        return StorageError::NotFound(path.to_string());
                    }
                }
                StorageError::Backend(e.to_string())
            })?;
        
        Ok(response.content_length().unwrap_or(0) as u64)
    }
}
