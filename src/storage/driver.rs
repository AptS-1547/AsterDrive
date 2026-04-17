use crate::errors::Result;
use async_trait::async_trait;
use std::time::Duration;
use tokio::io::AsyncRead;

#[derive(Debug, Clone)]
pub struct BlobMetadata {
    pub size: u64,
    pub content_type: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct PresignedDownloadOptions {
    pub response_cache_control: Option<String>,
    pub response_content_disposition: Option<String>,
    pub response_content_type: Option<String>,
}

pub trait StoragePathVisitor: Send {
    fn visit_path(&mut self, path: String) -> Result<()>;
}

#[async_trait]
pub trait StorageDriver: Send + Sync {
    /// 写入文件，返回最终存储路径
    async fn put(&self, path: &str, data: &[u8]) -> Result<String>;

    /// 读取文件全部内容
    async fn get(&self, path: &str) -> Result<Vec<u8>>;

    /// 获取文件流（大文件下载）
    async fn get_stream(&self, path: &str) -> Result<Box<dyn AsyncRead + Unpin + Send>>;

    /// 删除文件
    async fn delete(&self, path: &str) -> Result<()>;

    /// 文件是否存在
    async fn exists(&self, path: &str) -> Result<bool>;

    /// 获取文件元信息
    async fn metadata(&self, path: &str) -> Result<BlobMetadata>;

    /// 列出当前策略下的对象路径（相对路径）
    async fn list_paths(&self, prefix: Option<&str>) -> Result<Vec<String>> {
        let _ = prefix;
        Err(crate::errors::AsterError::storage_driver_error(
            "list_paths not supported by this driver",
        ))
    }

    /// 逐条扫描当前策略下的对象路径（相对路径），避免一次性拉取整个列表
    async fn scan_paths(
        &self,
        prefix: Option<&str>,
        visitor: &mut dyn StoragePathVisitor,
    ) -> Result<()> {
        for path in self.list_paths(prefix).await? {
            visitor.visit_path(path)?;
        }
        Ok(())
    }

    /// 从本地文件路径写入存储（分片上传组装后用，避免全量读入内存）。
    ///
    /// 这里故意不提供默认实现，防止新 driver 误用 “读完整文件到内存再 put”
    /// 的退化路径。
    async fn put_file(&self, storage_path: &str, local_path: &str) -> Result<String>;

    /// 从 reader 流式写入存储，适用于不应先落本地临时文件的上传路径
    async fn put_reader(
        &self,
        storage_path: &str,
        reader: Box<dyn AsyncRead + Unpin + Send + Sync>,
        size: i64,
    ) -> Result<String> {
        let _ = (storage_path, reader, size);
        Err(crate::errors::AsterError::storage_driver_error(
            "stream upload not supported by this driver",
        ))
    }

    /// 生成临时访问 URL（本地存储返回 None）
    async fn presigned_url(
        &self,
        path: &str,
        expires: Duration,
        options: PresignedDownloadOptions,
    ) -> Result<Option<String>>;

    /// 生成 presigned PUT URL 供客户端直传（S3 only，本地返回 None）
    async fn presigned_put_url(&self, path: &str, expires: Duration) -> Result<Option<String>> {
        let _ = (path, expires);
        Ok(None)
    }

    /// 同 bucket 内复制对象（S3 server-side copy）
    async fn copy_object(&self, src_path: &str, dest_path: &str) -> Result<String> {
        let _ = (src_path, dest_path);
        Err(crate::errors::AsterError::storage_driver_error(
            "copy_object not supported by this driver",
        ))
    }

    /// 将 self 向下转换为 `MultipartStorageDriver`（仅 S3 类驱动）。
    ///
    /// LocalDriver 返回 `None`；S3Driver 返回 `Some(self)`。
    fn as_multipart(&self) -> Option<&dyn super::multipart::MultipartStorageDriver> {
        None
    }
}
