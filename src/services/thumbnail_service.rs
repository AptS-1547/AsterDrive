//! 服务模块：`thumbnail_service`。

use std::io::Cursor;

use image::ImageFormat;
use image::imageops::FilterType;
use image::{ImageReader, Limits};

use crate::entities::file_blob;
use crate::errors::{
    AsterError, MapAsterErr, Result, thumbnail_generation_error_with_subcode,
    validation_error_with_subcode,
};
use crate::storage::StorageDriver;

const THUMB_MAX_DIM: u32 = 200;
const THUMB_PREFIX: &str = "_thumb";
pub(crate) const CURRENT_THUMBNAIL_VERSION: &str = "1";
pub(crate) const IMAGES_THUMBNAIL_PROCESSOR_NAMESPACE: &str = "images";
/// 单次解码最大内存分配（防止恶意/超大图 OOM）
const MAX_DECODE_ALLOC: u64 = 128 * 1024 * 1024;

fn thumbnail_format_guess_failed(message: String) -> AsterError {
    thumbnail_generation_error_with_subcode("thumbnail.format_guess_failed", message)
}

fn thumbnail_decode_failed(message: String) -> AsterError {
    thumbnail_generation_error_with_subcode("thumbnail.decode_failed", message)
}

fn thumbnail_encode_failed(message: String) -> AsterError {
    thumbnail_generation_error_with_subcode("thumbnail.encode_failed", message)
}

fn thumbnail_task_panicked(message: String) -> AsterError {
    thumbnail_generation_error_with_subcode("thumbnail.task_panicked", message)
}

/// 计算缩略图在存储驱动中的路径
pub(crate) fn thumb_path(blob_hash: &str) -> String {
    thumb_path_for(
        blob_hash,
        IMAGES_THUMBNAIL_PROCESSOR_NAMESPACE,
        CURRENT_THUMBNAIL_VERSION,
    )
}

pub(crate) fn thumb_path_for(
    blob_hash: &str,
    thumbnail_processor: &str,
    thumbnail_version: &str,
) -> String {
    format!(
        "{}/{}/{}/{}/{}/{}.webp",
        THUMB_PREFIX,
        thumbnail_processor,
        thumbnail_version,
        &blob_hash[..2],
        &blob_hash[2..4],
        blob_hash
    )
}

pub(crate) fn thumbnail_etag_value_for(
    blob_hash: &str,
    thumbnail_processor: Option<&str>,
    thumbnail_version: Option<&str>,
) -> String {
    format!(
        "thumb-{}-{}-{blob_hash}",
        thumbnail_processor.unwrap_or(IMAGES_THUMBNAIL_PROCESSOR_NAMESPACE),
        thumbnail_version.unwrap_or(CURRENT_THUMBNAIL_VERSION)
    )
}

pub(crate) fn current_thumbnail_max_dim() -> u32 {
    THUMB_MAX_DIM
}

pub fn is_thumbnail_path(path: &str) -> bool {
    path.trim_start_matches('/')
        .starts_with(&format!("{THUMB_PREFIX}/"))
}

/// 解码图片 → 缩放 → 编码为 WebP（CPU 密集，应在 spawn_blocking 中调用）
///
/// 接管 Vec 所有权：decode 后原始字节立即释放，减少峰值内存
fn generate_thumbnail(data: Vec<u8>) -> Result<Vec<u8>> {
    // ImageReader: 支持格式检测 + 内存限制
    let mut reader = ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .map_aster_err_ctx("guess format", thumbnail_format_guess_failed)?;

    // 限制解码内存，防止恶意超大图 OOM
    let mut limits = Limits::default();
    limits.max_alloc = Some(MAX_DECODE_ALLOC);
    reader.limits(limits);

    // decode() 消费 reader → 内部 Cursor 持有的 Vec<u8> 原始字节在此释放
    let img = reader
        .decode()
        .map_aster_err_ctx("decode", thumbnail_decode_failed)?;

    // 已经小于目标尺寸 → 直接编码，跳过 resize
    if img.width() <= THUMB_MAX_DIM && img.height() <= THUMB_MAX_DIM {
        return encode_webp(&img);
    }

    // Triangle（双线性）滤镜：比 Lanczos3 快 2-3 倍，200px 缩略图肉眼无差
    let thumb = img.resize(THUMB_MAX_DIM, THUMB_MAX_DIM, FilterType::Triangle);
    drop(img); // 释放全尺寸像素 buffer，再编码

    encode_webp(&thumb)
}

fn encode_webp(img: &image::DynamicImage) -> Result<Vec<u8>> {
    let mut buf = Cursor::new(Vec::new());
    img.write_to(&mut buf, ImageFormat::WebP)
        .map_aster_err_ctx("encode webp", thumbnail_encode_failed)?;
    Ok(buf.into_inner())
}

pub(crate) async fn render_thumbnail_bytes(
    driver: &dyn StorageDriver,
    blob: &file_blob::Model,
) -> Result<Vec<u8>> {
    let original = driver.get(&blob.storage_path).await?;
    tokio::task::spawn_blocking(move || generate_thumbnail(original))
        .await
        .map_aster_err_ctx("thumbnail task panicked", thumbnail_task_panicked)?
}

pub(crate) fn ensure_source_size_supported(
    blob: &file_blob::Model,
    max_source_bytes: i64,
) -> Result<()> {
    if blob.size > max_source_bytes {
        return Err(validation_error_with_subcode(
            "thumbnail.source_too_large",
            format!(
                "thumbnail source exceeds {} MiB limit",
                max_source_bytes / 1024 / 1024
            ),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{ensure_source_size_supported, thumb_path, thumbnail_etag_value_for};
    use crate::config::operations::DEFAULT_THUMBNAIL_MAX_SOURCE_BYTES;
    use crate::entities::file_blob;
    use chrono::Utc;

    fn blob_with_size(size: i64) -> file_blob::Model {
        file_blob::Model {
            id: 1,
            hash: "abc".repeat(21) + "a",
            size,
            policy_id: 1,
            storage_path: "files/test".to_string(),
            thumbnail_path: None,
            thumbnail_processor: None,
            thumbnail_version: None,
            ref_count: 1,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn accepts_thumbnail_source_within_size_limit() {
        let max_source_bytes = crate::utils::numbers::u64_to_i64(
            DEFAULT_THUMBNAIL_MAX_SOURCE_BYTES,
            "thumbnail max source bytes",
        )
        .unwrap();
        assert!(
            ensure_source_size_supported(&blob_with_size(max_source_bytes), max_source_bytes,)
                .is_ok()
        );
    }

    #[test]
    fn rejects_thumbnail_source_above_size_limit() {
        let max_source_bytes = crate::utils::numbers::u64_to_i64(
            DEFAULT_THUMBNAIL_MAX_SOURCE_BYTES,
            "thumbnail max source bytes",
        )
        .unwrap();
        let err =
            ensure_source_size_supported(&blob_with_size(max_source_bytes + 1), max_source_bytes)
                .unwrap_err();
        assert_eq!(err.api_error_subcode(), Some("thumbnail.source_too_large"));
    }

    #[test]
    fn thumbnail_paths_are_versioned() {
        let hash = "abc".repeat(21) + "a";
        assert_eq!(
            thumb_path(&hash),
            format!("_thumb/images/1/ab/ca/{hash}.webp")
        );
    }

    #[test]
    fn thumbnail_etag_uses_thumbnail_version_namespace() {
        let hash = "abc".repeat(21) + "a";
        assert_eq!(
            thumbnail_etag_value_for(&hash, None, None),
            format!("thumb-images-1-{hash}")
        );
    }

    #[test]
    fn thumbnail_etag_can_use_persisted_processor_and_version() {
        let hash = "abc".repeat(21) + "a";
        assert_eq!(
            thumbnail_etag_value_for(&hash, Some("vips-cli"), Some("7")),
            format!("thumb-vips-cli-7-{hash}")
        );
    }
}
