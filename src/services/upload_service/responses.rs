//! 上传服务子模块：`responses`。

use serde::Serialize;
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::ToSchema;

use crate::types::{UploadMode, UploadSessionStatus};

#[derive(Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct InitUploadResponse {
    pub mode: UploadMode,
    pub upload_id: Option<String>,
    pub chunk_size: Option<i64>,
    pub total_chunks: Option<i32>,
    /// S3 presigned PUT URL（仅 presigned 模式）
    pub presigned_url: Option<String>,
}

#[derive(Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct ChunkUploadResponse {
    pub received_count: i32,
    pub total_chunks: i32,
}

#[derive(Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct UploadProgressResponse {
    pub upload_id: String,
    pub status: UploadSessionStatus,
    pub received_count: i32,
    pub chunks_on_disk: Vec<i32>,
    pub chunk_size: i64,
    pub total_chunks: i32,
    pub filename: String,
}
