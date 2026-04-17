//! S3 multipart upload 抽象层。
//!
//! Multipart upload 是 S3/MinIO/R2 等对象存储特有的语义，本地存储不支持。
//! 将其隔离在 `MultipartStorageDriver` 子 trait 中，避免 `StorageDriver` trait
//! 被 S3 特有概念（upload_id / part_number / ETag）污染。

use crate::errors::Result;
use async_trait::async_trait;
use std::time::Duration;

/// Multipart upload 支持（仅 S3 类驱动）。
///
/// 调用方通过 `driver.as_multipart()` 获取引用。
/// **调用方必须确保 session 携带了 `s3_multipart_id`**，否则不应该调用此方法。
#[async_trait]
pub trait MultipartStorageDriver: Send + Sync {
    /// 创建 multipart upload，返回 provider 端的 upload_id
    async fn create_multipart_upload(&self, path: &str) -> Result<String>;

    /// 为指定 part 生成 presigned PUT URL
    async fn presigned_upload_part_url(
        &self,
        path: &str,
        upload_id: &str,
        part_number: i32,
        expires: Duration,
    ) -> Result<String>;

    /// 完成 multipart upload（parts: Vec<(part_number, etag)>）
    async fn complete_multipart_upload(
        &self,
        path: &str,
        upload_id: &str,
        parts: Vec<(i32, String)>,
    ) -> Result<()>;

    /// 服务端直接上传一个 multipart part，返回该 part 的 ETag
    async fn upload_multipart_part(
        &self,
        path: &str,
        upload_id: &str,
        part_number: i32,
        data: &[u8],
    ) -> Result<String>;

    /// 取消 multipart upload（清理已上传的 parts）
    async fn abort_multipart_upload(&self, path: &str, upload_id: &str) -> Result<()>;

    /// 列出已上传的 parts（返回 part numbers，用于断点续传进度查询）
    async fn list_uploaded_parts(&self, path: &str, upload_id: &str) -> Result<Vec<i32>>;
}
