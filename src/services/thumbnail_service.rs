use std::io::Cursor;

use image::ImageFormat;
use image::imageops::FilterType;
use image::{ImageReader, Limits};

use crate::config::operations;
use crate::entities::file_blob;
use crate::errors::{AsterError, MapAsterErr, Result};
use crate::runtime::AppState;
use crate::storage::StorageDriver;

const THUMB_MAX_DIM: u32 = 200;
const THUMB_PREFIX: &str = "_thumb";
const THUMB_VERSION: &str = "v2";
/// 单次解码最大内存分配（防止恶意/超大图 OOM）
const MAX_DECODE_ALLOC: u64 = 128 * 1024 * 1024;

/// 判断 MIME 类型是否支持生成缩略图
pub fn is_supported_mime(mime: &str) -> bool {
    matches!(
        mime,
        "image/jpeg" | "image/png" | "image/gif" | "image/webp" | "image/bmp" | "image/tiff"
    )
}

pub fn ensure_supported_mime(mime: &str) -> Result<()> {
    if is_supported_mime(mime) {
        return Ok(());
    }

    Err(AsterError::validation_error(format!(
        "thumbnails are not supported for MIME type '{mime}'"
    )))
}

/// 计算缩略图在存储驱动中的路径
pub(crate) fn thumb_path(blob_hash: &str) -> String {
    format!(
        "{}/{}/{}/{}/{}.webp",
        THUMB_PREFIX,
        THUMB_VERSION,
        &blob_hash[..2],
        &blob_hash[2..4],
        blob_hash
    )
}

pub(crate) fn legacy_thumb_path(blob_hash: &str) -> String {
    format!(
        "{}/{}/{}/{}.webp",
        THUMB_PREFIX,
        &blob_hash[..2],
        &blob_hash[2..4],
        blob_hash
    )
}

pub(crate) fn thumbnail_etag_value(blob_hash: &str) -> String {
    format!("thumb-{THUMB_VERSION}-{blob_hash}")
}

pub(crate) fn is_thumbnail_path(path: &str) -> bool {
    path.trim_start_matches('/')
        .starts_with(&format!("{THUMB_PREFIX}/"))
}

/// 尝试获取已有缩略图，如果不存在则返回 None。
pub async fn load_thumbnail_if_exists(
    state: &AppState,
    blob: &file_blob::Model,
) -> Result<Option<Vec<u8>>> {
    ensure_source_size_supported(
        blob,
        operations::thumbnail_max_source_bytes(&state.runtime_config),
    )?;
    let driver = thumbnail_driver(state, blob)?;
    let path = thumb_path(&blob.hash);

    match driver.get(&path).await {
        Ok(data) => Ok(Some(data)),
        Err(_) => Ok(None),
    }
}

/// 获取或同步生成缩略图（仅用于公开分享等无法等待的场景）
pub async fn get_or_generate(state: &AppState, blob: &file_blob::Model) -> Result<Vec<u8>> {
    if let Some(data) = load_thumbnail_if_exists(state, blob).await? {
        return Ok(data);
    }

    let driver = thumbnail_driver(state, blob)?;
    let path = thumb_path(&blob.hash);
    let webp_bytes = render_thumbnail_bytes(driver.as_ref(), blob).await?;

    if let Err(e) = driver.put(&path, &webp_bytes).await {
        tracing::warn!("failed to store thumbnail {path}: {e}");
    }

    Ok(webp_bytes)
}

/// 严格生成并写回缩略图。
///
/// 如果缩略图已存在，会直接复用并返回 `(path, true)`。
/// 如果本次成功生成并持久化，会返回 `(path, false)`。
pub async fn generate_and_store(
    state: &AppState,
    blob: &file_blob::Model,
) -> Result<(String, bool)> {
    ensure_source_size_supported(
        blob,
        operations::thumbnail_max_source_bytes(&state.runtime_config),
    )?;
    let driver = thumbnail_driver(state, blob)?;
    let path = thumb_path(&blob.hash);

    if driver.exists(&path).await.unwrap_or(false) {
        return Ok((path, true));
    }

    let webp_bytes = render_thumbnail_bytes(driver.as_ref(), blob).await?;
    driver.put(&path, &webp_bytes).await?;
    Ok((path, false))
}

/// 删除缩略图（blob 物理删除时调用）
pub async fn delete_thumbnail(state: &AppState, blob: &file_blob::Model) -> Result<()> {
    let policy = state.policy_snapshot.get_policy_or_err(blob.policy_id)?;
    let driver = state.driver_registry.get_driver(&policy)?;

    for path in [thumb_path(&blob.hash), legacy_thumb_path(&blob.hash)] {
        if driver.exists(&path).await.unwrap_or(false) {
            driver.delete(&path).await?;
        }
    }
    Ok(())
}

/// 解码图片 → 缩放 → 编码为 WebP（CPU 密集，应在 spawn_blocking 中调用）
///
/// 接管 Vec 所有权：decode 后原始字节立即释放，减少峰值内存
fn generate_thumbnail(data: Vec<u8>) -> Result<Vec<u8>> {
    // ImageReader: 支持格式检测 + 内存限制
    let mut reader = ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .map_aster_err_ctx("guess format", AsterError::thumbnail_generation_failed)?;

    // 限制解码内存，防止恶意超大图 OOM
    let mut limits = Limits::default();
    limits.max_alloc = Some(MAX_DECODE_ALLOC);
    reader.limits(limits);

    // decode() 消费 reader → 内部 Cursor 持有的 Vec<u8> 原始字节在此释放
    let img = reader
        .decode()
        .map_aster_err_ctx("decode", AsterError::thumbnail_generation_failed)?;

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
        .map_aster_err_ctx("encode webp", AsterError::thumbnail_generation_failed)?;
    Ok(buf.into_inner())
}

fn thumbnail_driver(
    state: &AppState,
    blob: &file_blob::Model,
) -> Result<std::sync::Arc<dyn StorageDriver>> {
    let policy = state.policy_snapshot.get_policy_or_err(blob.policy_id)?;
    state.driver_registry.get_driver(&policy)
}

async fn render_thumbnail_bytes(
    driver: &dyn StorageDriver,
    blob: &file_blob::Model,
) -> Result<Vec<u8>> {
    let original = driver.get(&blob.storage_path).await?;
    tokio::task::spawn_blocking(move || generate_thumbnail(original))
        .await
        .map_aster_err_ctx(
            "thumbnail task panicked",
            AsterError::thumbnail_generation_failed,
        )?
}

fn ensure_source_size_supported(blob: &file_blob::Model, max_source_bytes: i64) -> Result<()> {
    if blob.size > max_source_bytes {
        return Err(AsterError::validation_error(format!(
            "thumbnail source exceeds {} MiB limit",
            max_source_bytes / 1024 / 1024
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{ensure_source_size_supported, thumb_path, thumbnail_etag_value};
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
            ref_count: 1,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn accepts_thumbnail_source_within_size_limit() {
        assert!(
            ensure_source_size_supported(
                &blob_with_size(DEFAULT_THUMBNAIL_MAX_SOURCE_BYTES as i64),
                DEFAULT_THUMBNAIL_MAX_SOURCE_BYTES as i64,
            )
            .is_ok()
        );
    }

    #[test]
    fn rejects_thumbnail_source_above_size_limit() {
        assert!(
            ensure_source_size_supported(
                &blob_with_size(DEFAULT_THUMBNAIL_MAX_SOURCE_BYTES as i64 + 1),
                DEFAULT_THUMBNAIL_MAX_SOURCE_BYTES as i64,
            )
            .is_err()
        );
    }

    #[test]
    fn thumbnail_paths_are_versioned() {
        let hash = "abc".repeat(21) + "a";
        assert_eq!(thumb_path(&hash), format!("_thumb/v2/ab/ca/{hash}.webp"));
    }

    #[test]
    fn thumbnail_etag_uses_thumbnail_version_namespace() {
        let hash = "abc".repeat(21) + "a";
        assert_eq!(thumbnail_etag_value(&hash), format!("thumb-v2-{hash}"));
    }
}
