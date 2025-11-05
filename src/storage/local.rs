use super::traits::{StorageBackend, StorageError, StorageResult};
use async_trait::async_trait;
use bytes::Bytes;
use std::path::PathBuf;
use tokio::fs;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};

pub struct LocalStorage {
    base_path: PathBuf,
}

impl LocalStorage {
    pub async fn new(base_path: PathBuf) -> StorageResult<Self> {
        fs::create_dir_all(&base_path).await?;
        Ok(Self { base_path })
    }
    
    fn full_path(&self, path: &str) -> PathBuf {
        self.base_path.join(path)
    }
}

#[async_trait]
impl StorageBackend for LocalStorage {
    async fn store(&self, path: &str, data: Bytes) -> StorageResult<String> {
        let full_path = self.full_path(path);
        
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).await?;
        }
        
        fs::write(&full_path, data).await?;
        Ok(path.to_string())
    }
    
    async fn store_stream(
        &self,
        path: &str,
        mut stream: Box<dyn AsyncRead + Send + Unpin>,
        _size: u64,
    ) -> StorageResult<String> {
        let full_path = self.full_path(path);
        
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).await?;
        }
        
        let mut file = fs::File::create(&full_path).await?;
        let mut buffer = vec![0u8; 8192];
        
        loop {
            let n = stream.read(&mut buffer).await?;
            if n == 0 {
                break;
            }
            file.write_all(&buffer[..n]).await?;
        }
        
        file.sync_all().await?;
        Ok(path.to_string())
    }
    
    async fn retrieve(&self, path: &str) -> StorageResult<Bytes> {
        let full_path = self.full_path(path);
        let data = fs::read(&full_path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                StorageError::NotFound(path.to_string())
            } else {
                StorageError::Io(e)
            }
        })?;
        Ok(Bytes::from(data))
    }
    
    async fn delete(&self, path: &str) -> StorageResult<()> {
        let full_path = self.full_path(path);
        fs::remove_file(&full_path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                StorageError::NotFound(path.to_string())
            } else {
                StorageError::Io(e)
            }
        })
    }
    
    async fn exists(&self, path: &str) -> StorageResult<bool> {
        let full_path = self.full_path(path);
        Ok(full_path.exists())
    }
    
    async fn size(&self, path: &str) -> StorageResult<u64> {
        let full_path = self.full_path(path);
        let metadata = fs::metadata(&full_path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                StorageError::NotFound(path.to_string())
            } else {
                StorageError::Io(e)
            }
        })?;
        Ok(metadata.len())
    }
}
