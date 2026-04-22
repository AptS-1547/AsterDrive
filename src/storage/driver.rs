//! 存储子模块：`driver`。

use crate::errors::{AsterError, MapAsterErr, Result};
use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncReadExt};

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

/// 存储驱动核心 trait。
///
/// 设计原则：
/// - 最小接口：仅包含所有存储类型必须实现的基础操作
/// - 默认实现：copy_object 提供基于 get+put 的通用实现，驱动可覆盖优化
/// - 扩展能力：通过 as_xxx() 方法暴露可选 trait，避免强制实现
#[async_trait]
pub trait StorageDriver: Send + Sync {
    /// 写入文件，返回最终存储路径
    async fn put(&self, path: &str, data: &[u8]) -> Result<String>;

    /// 读取文件全部内容
    async fn get(&self, path: &str) -> Result<Vec<u8>>;

    /// 获取文件流（大文件下载）
    async fn get_stream(&self, path: &str) -> Result<Box<dyn AsyncRead + Unpin + Send>>;

    /// 获取文件的指定字节区间（HTTP Range / 视频 seek / 断点续传下载）
    ///
    /// - `offset`: 从文件起始的字节偏移；0 表示从头读
    /// - `length`: `None` 表示读到文件末尾，`Some(n)` 表示最多读 `n` 字节
    ///
    /// 默认实现基于 `get_stream` + 字节丢弃，性能不如原生 Range；
    /// 支持原生 Range 请求的驱动（S3/R2/OSS 等）以及可 seek 的驱动（本地文件）
    /// 应覆盖此方法以避免读完整文件。
    async fn get_range(
        &self,
        path: &str,
        offset: u64,
        length: Option<u64>,
    ) -> Result<Box<dyn AsyncRead + Unpin + Send>> {
        let mut stream = self.get_stream(path).await?;
        if offset > 0 {
            let mut skip = (&mut stream).take(offset);
            tokio::io::copy(&mut skip, &mut tokio::io::sink())
                .await
                .map_aster_err_ctx("skip bytes for range", AsterError::storage_driver_error)?;
        }
        Ok(match length {
            Some(len) => Box::new(stream.take(len)),
            None => stream,
        })
    }

    /// 删除文件
    async fn delete(&self, path: &str) -> Result<()>;

    /// 文件是否存在
    async fn exists(&self, path: &str) -> Result<bool>;

    /// 获取文件元信息
    async fn metadata(&self, path: &str) -> Result<BlobMetadata>;

    /// 同 bucket/存储内复制对象
    ///
    /// 默认实现基于 get + put，性能较慢但通用。
    /// 支持 server-side copy 的驱动（如 S3）应覆盖此方法。
    async fn copy_object(&self, src_path: &str, dest_path: &str) -> Result<String> {
        let data = self.get(src_path).await?;
        self.put(dest_path, &data).await
    }

    // =========================================================================
    // 扩展能力查询（返回 Option<&dyn Trait>，不支持的驱动返回 None）
    // =========================================================================

    /// 获取 presigned URL 支持
    ///
    /// S3/R2/OSS 等对象存储返回 Some，本地存储返回 None。
    fn as_presigned(&self) -> Option<&dyn super::extensions::PresignedStorageDriver> {
        None
    }

    /// 获取路径列举支持
    ///
    /// 用于后台维护任务（孤儿 blob 清理等）。
    fn as_list(&self) -> Option<&dyn super::extensions::ListStorageDriver> {
        None
    }

    /// 获取流式直传支持
    ///
    /// S3 支持原生流式上传；本地存储基于临时文件提供通用实现。
    fn as_stream_upload(&self) -> Option<&dyn super::extensions::StreamUploadDriver> {
        None
    }

    /// 获取存储侧原生缩略图支持
    ///
    /// OneDrive / 数据万象 / 对象存储图片处理等驱动返回 Some。
    fn as_native_thumbnail(&self) -> Option<&dyn super::extensions::NativeThumbnailStorageDriver> {
        None
    }

    /// 获取 multipart upload 支持（S3 特有）
    ///
    /// 通过 downcast 获取 MultipartStorageDriver，用于分片上传。
    fn as_multipart(&self) -> Option<&dyn super::multipart::MultipartStorageDriver> {
        None
    }
}
