use crate::errors::Result;
use async_trait::async_trait;
use std::time::Duration;
use tokio::io::AsyncRead;

#[derive(Debug, Clone)]
pub struct BlobMetadata {
    pub size: u64,
    pub content_type: Option<String>,
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

    /// 从本地文件路径写入存储（分片上传组装后用，避免全量读入内存）
    /// 默认实现：读取文件 → put，子类可覆盖为 rename/stream
    async fn put_file(&self, storage_path: &str, local_path: &str) -> Result<String> {
        let data = tokio::fs::read(local_path).await.map_err(|e| {
            crate::errors::AsterError::storage_driver_error(format!("read file: {e}"))
        })?;
        self.put(storage_path, &data).await
    }

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
    async fn presigned_url(&self, path: &str, expires: Duration) -> Result<Option<String>>;

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

    // ── S3 Multipart Upload ──────────────────────────────────────────

    /// 创建 S3 multipart upload，返回 AWS upload_id
    async fn create_multipart_upload(&self, path: &str) -> Result<String> {
        let _ = path;
        Err(crate::errors::AsterError::storage_driver_error(
            "multipart upload not supported by this driver",
        ))
    }

    /// 为指定 part 生成 presigned PUT URL
    async fn presigned_upload_part_url(
        &self,
        path: &str,
        upload_id: &str,
        part_number: i32,
        expires: Duration,
    ) -> Result<String> {
        let _ = (path, upload_id, part_number, expires);
        Err(crate::errors::AsterError::storage_driver_error(
            "multipart upload not supported by this driver",
        ))
    }

    /// 完成 S3 multipart upload（parts: Vec<(part_number, etag)>）
    async fn complete_multipart_upload(
        &self,
        path: &str,
        upload_id: &str,
        parts: Vec<(i32, String)>,
    ) -> Result<()> {
        let _ = (path, upload_id, parts);
        Err(crate::errors::AsterError::storage_driver_error(
            "multipart upload not supported by this driver",
        ))
    }

    /// 服务端直接上传一个 multipart part，返回该 part 的 ETag
    async fn upload_multipart_part(
        &self,
        path: &str,
        upload_id: &str,
        part_number: i32,
        data: &[u8],
    ) -> Result<String> {
        let _ = (path, upload_id, part_number, data);
        Err(crate::errors::AsterError::storage_driver_error(
            "multipart upload not supported by this driver",
        ))
    }

    /// 取消 S3 multipart upload（清理已上传的 parts）
    async fn abort_multipart_upload(&self, path: &str, upload_id: &str) -> Result<()> {
        let _ = (path, upload_id);
        Err(crate::errors::AsterError::storage_driver_error(
            "multipart upload not supported by this driver",
        ))
    }

    /// 列出已上传的 parts（返回 part numbers，用于断点续传进度查询）
    async fn list_uploaded_parts(&self, path: &str, upload_id: &str) -> Result<Vec<i32>> {
        let _ = (path, upload_id);
        Err(crate::errors::AsterError::storage_driver_error(
            "multipart upload not supported by this driver",
        ))
    }
}
