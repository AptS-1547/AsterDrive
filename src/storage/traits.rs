use async_trait::async_trait;
use bytes::Bytes;
use thiserror::Error;
use tokio::io::AsyncRead;

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Storage backend error: {0}")]
    Backend(String),
    
    #[error("File not found: {0}")]
    NotFound(String),
}

pub type StorageResult<T> = Result<T, StorageError>;

#[async_trait]
pub trait StorageBackend: Send + Sync {
    /// Store a file and return its storage path
    async fn store(&self, path: &str, data: Bytes) -> StorageResult<String>;
    
    /// Store a file from a stream
    async fn store_stream(
        &self,
        path: &str,
        stream: Box<dyn AsyncRead + Send + Unpin>,
        size: u64,
    ) -> StorageResult<String>;
    
    /// Retrieve a file
    async fn retrieve(&self, path: &str) -> StorageResult<Bytes>;
    
    /// Delete a file
    async fn delete(&self, path: &str) -> StorageResult<()>;
    
    /// Check if a file exists
    async fn exists(&self, path: &str) -> StorageResult<bool>;
    
    /// Get file size
    async fn size(&self, path: &str) -> StorageResult<u64>;
}
