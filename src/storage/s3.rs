use super::traits::{StorageBackend, StorageError, StorageResult};
use async_trait::async_trait;
use aws_sdk_s3::primitives::ByteStream;
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
        
        let mut sdk_config = config_loader.load().await;
        
        if let Some(endpoint) = endpoint {
            sdk_config = aws_config::SdkConfig::builder()
                .endpoint_url(endpoint)
                .region(sdk_config.region().cloned())
                .credentials_provider(sdk_config.credentials_provider().unwrap().clone())
                .build();
        }
        
        let client = Client::new(&sdk_config);
        
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
        _size: u64,
    ) -> StorageResult<String> {
        // Read stream into memory
        let mut buffer = Vec::new();
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
                if e.to_string().contains("NoSuchKey") {
                    StorageError::NotFound(path.to_string())
                } else {
                    StorageError::Backend(e.to_string())
                }
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
                if e.to_string().contains("NotFound") {
                    Ok(false)
                } else {
                    Err(StorageError::Backend(e.to_string()))
                }
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
                if e.to_string().contains("NotFound") {
                    StorageError::NotFound(path.to_string())
                } else {
                    StorageError::Backend(e.to_string())
                }
            })?;
        
        Ok(response.content_length().unwrap_or(0) as u64)
    }
}
