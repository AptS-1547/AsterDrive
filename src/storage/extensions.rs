//! StorageDriver 扩展 trait
//!
//! 将可选能力从核心 StorageDriver 分离，避免每个驱动被迫实现不需要的功能。

use crate::errors::Result;
use crate::storage::driver::{PresignedDownloadOptions, StoragePathVisitor};
use async_trait::async_trait;
use std::time::Duration;
use tokio::io::AsyncRead;

/// Presigned URL 支持（S3/R2/OSS 等对象存储）
#[async_trait]
pub trait PresignedStorageDriver: Send + Sync {
    /// 生成临时下载 URL
    async fn presigned_url(
        &self,
        path: &str,
        expires: Duration,
        options: PresignedDownloadOptions,
    ) -> Result<Option<String>>;

    /// 生成 presigned PUT URL 供客户端直传
    async fn presigned_put_url(&self, path: &str, expires: Duration) -> Result<Option<String>>;
}

/// 路径列举支持（用于后台维护任务）
#[async_trait]
pub trait ListStorageDriver: Send + Sync {
    /// 列出当前策略下的对象路径（相对路径）
    async fn list_paths(&self, prefix: Option<&str>) -> Result<Vec<String>>;

    /// 逐条扫描当前策略下的对象路径，避免一次性拉取整个列表
    ///
    /// 默认实现基于 list_paths，驱动可覆盖优化（如流式 API）
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
}

/// 流式直传支持（避免本地临时文件）
#[async_trait]
pub trait StreamUploadDriver: Send + Sync {
    /// 从 reader 流式写入存储
    ///
    /// 适用于不应先落本地临时文件的上传路径（如 WebDAV 直传、S3 流式上传）。
    /// 驱动可实现优化路径；默认实现写临时文件后调用 put_file。
    async fn put_reader(
        &self,
        storage_path: &str,
        reader: Box<dyn AsyncRead + Unpin + Send + Sync>,
        size: i64,
    ) -> Result<String>;

    /// 从本地文件路径写入存储（分片上传组装后使用）
    ///
    /// 这是 put_reader 默认实现的基础；暴露出来供需要显式控制临时文件生命周期的调用方使用。
    async fn put_file(&self, storage_path: &str, local_path: &str) -> Result<String>;
}

/// 为所有 StorageDriver 提供 StreamUploadDriver 的默认实现
///
/// 此模块提供基于临时文件的通用实现，供不支持原生流式上传的驱动使用。
pub mod fallback {
    use super::*;
    use crate::errors::AsterError;
    use crate::storage::MapAsterErr;
    use tokio::io::AsyncWriteExt;

    /// 基于临时文件的 put_reader 通用实现
    pub async fn put_reader_with_temp_file<D>(
        driver: &D,
        storage_path: &str,
        mut reader: Box<dyn AsyncRead + Unpin + Send + Sync>,
        _size: i64,
    ) -> Result<String>
    where
        D: super::super::driver::StorageDriver + ?Sized,
    {
        // 创建临时文件
        let temp_dir = std::env::temp_dir();
        let temp_path = temp_dir.join(format!(
            "aster_put_reader_{}_{}",
            std::process::id(),
            rand::random::<u64>()
        ));

        // 流式写入临时文件
        let mut file = tokio::fs::File::create(&temp_path)
            .await
            .map_aster_err(AsterError::storage_driver_error)?;

        tokio::io::copy(&mut reader, &mut file)
            .await
            .map_aster_err_ctx("write temp file", AsterError::storage_driver_error)?;

        // 确保数据落盘
        file.flush()
            .await
            .map_aster_err(AsterError::storage_driver_error)?;
        drop(file);

        // 使用驱动的 put_file 能力上传（如果驱动实现了 StreamUploadDriver）
        // 否则退化为 put + read file
        let result = if let Some(stream_driver) = driver.as_stream_upload() {
            stream_driver
                .put_file(storage_path, temp_path.to_str().unwrap())
                .await
        } else {
            // 终极 fallback：读文件到内存再 put
            let data = tokio::fs::read(&temp_path)
                .await
                .map_aster_err(AsterError::storage_driver_error)?;
            driver.put(storage_path, &data).await
        };

        // 清理临时文件（忽略错误）
        let _ = tokio::fs::remove_file(&temp_path).await;

        result
    }
}
