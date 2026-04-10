use std::collections::HashSet;
use std::io::Cursor;
use std::sync::Arc;

use image::ImageFormat;
use image::imageops::FilterType;
use image::{ImageReader, Limits};
use tokio::sync::{Mutex, Semaphore, mpsc};

use crate::db::repository::file_repo;
use crate::entities::file_blob;
use crate::errors::{AsterError, MapAsterErr, Result};
use crate::runtime::AppState;
use crate::storage::{DriverRegistry, PolicySnapshot};

const THUMB_MAX_DIM: u32 = 200;
const THUMB_PREFIX: &str = "_thumb";
const THUMB_VERSION: &str = "v2";
/// 单次解码最大内存分配（防止恶意/超大图 OOM）
const MAX_DECODE_ALLOC: u64 = 128 * 1024 * 1024;
/// 在解码前限制源文件大小，避免原始字节和像素 buffer 双重叠峰。
const MAX_THUMB_SOURCE_BYTES: i64 = 64 * 1024 * 1024;
const MAX_THUMB_WORKERS: usize = 2;

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
fn thumb_path(blob_hash: &str) -> String {
    format!(
        "{}/{}/{}/{}/{}.webp",
        THUMB_PREFIX,
        THUMB_VERSION,
        &blob_hash[..2],
        &blob_hash[2..4],
        blob_hash
    )
}

fn legacy_thumb_path(blob_hash: &str) -> String {
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

/// 尝试获取已有缩略图，如果不存在则入队后台生成并返回 None
pub async fn get_or_enqueue(state: &AppState, blob: &file_blob::Model) -> Result<Option<Vec<u8>>> {
    ensure_source_size_supported(blob)?;
    let policy = state.policy_snapshot.get_policy_or_err(blob.policy_id)?;
    let driver = state.driver_registry.get_driver(&policy)?;
    let path = thumb_path(&blob.hash);

    // 已存在 → 直接返回
    if let Ok(data) = driver.get(&path).await {
        return Ok(Some(data));
    }

    // 入队后台生成（非阻塞，队列满时 drop）
    if let Err(e) = state.thumbnail_tx.try_send(blob.id) {
        tracing::warn!(
            blob_id = blob.id,
            "thumbnail queue full, dropping request: {e}"
        );
    }

    Ok(None)
}

/// 获取或同步生成缩略图（仅用于公开分享等无法等待的场景）
pub async fn get_or_generate(state: &AppState, blob: &file_blob::Model) -> Result<Vec<u8>> {
    ensure_source_size_supported(blob)?;
    let policy = state.policy_snapshot.get_policy_or_err(blob.policy_id)?;
    let driver = state.driver_registry.get_driver(&policy)?;
    let path = thumb_path(&blob.hash);

    // 已存在 → 直接返回
    if let Ok(data) = driver.get(&path).await {
        return Ok(data);
    }

    // 同步生成（CPU 密集部分走 blocking 线程池）
    let original = driver.get(&blob.storage_path).await?;
    let webp_bytes = tokio::task::spawn_blocking(move || generate_thumbnail(original))
        .await
        .map_aster_err_ctx(
            "thumbnail task panicked",
            AsterError::thumbnail_generation_failed,
        )??;

    if let Err(e) = driver.put(&path, &webp_bytes).await {
        tracing::warn!("failed to store thumbnail {path}: {e}");
    }

    Ok(webp_bytes)
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

/// 并发上限：避免大量图片同时解码导致内存峰值
fn max_concurrent_thumbnails() -> usize {
    num_cpus::get().min(MAX_THUMB_WORKERS)
}

/// 启动后台缩略图 worker（并发处理，Semaphore 限流，panic-safe）
pub fn spawn_worker(
    db: actix_web::web::Data<sea_orm::DatabaseConnection>,
    driver_registry: Arc<DriverRegistry>,
    policy_snapshot: Arc<PolicySnapshot>,
    mut rx: mpsc::Receiver<i64>,
) {
    let pending = Arc::new(Mutex::new(HashSet::<i64>::new()));
    let semaphore = Arc::new(Semaphore::new(max_concurrent_thumbnails()));

    tokio::spawn(async move {
        tracing::info!(
            "thumbnail worker started (concurrency={})",
            max_concurrent_thumbnails()
        );

        while let Some(blob_id) = rx.recv().await {
            // 去重检查
            {
                let mut set = pending.lock().await;
                if set.contains(&blob_id) {
                    continue;
                }
                set.insert(blob_id);
            }

            let db = db.clone();
            let registry = driver_registry.clone();
            let policy_snapshot = policy_snapshot.clone();
            let pending_inner = pending.clone();
            let sem = semaphore.clone();

            // 并发派发，由 Semaphore 控制同时处理数量
            tokio::spawn(async move {
                // 获取许可（背压：队列消费速度受限于并发上限）
                let _permit = sem.acquire().await;

                if let Err(e) =
                    process_one_thumbnail(&db, &registry, &policy_snapshot, blob_id).await
                {
                    tracing::warn!("thumbnail generation failed for blob #{blob_id}: {e}");
                }

                pending_inner.lock().await.remove(&blob_id);
            });
        }

        tracing::info!("thumbnail worker stopped");
    });
}

/// 处理单个 blob 的缩略图生成
async fn process_one_thumbnail(
    db: &sea_orm::DatabaseConnection,
    driver_registry: &DriverRegistry,
    policy_snapshot: &PolicySnapshot,
    blob_id: i64,
) -> Result<()> {
    let blob = file_repo::find_blob_by_id(db, blob_id).await?;
    ensure_source_size_supported(&blob)?;
    let policy = policy_snapshot.get_policy_or_err(blob.policy_id)?;
    let driver = driver_registry.get_driver(&policy)?;
    let path = thumb_path(&blob.hash);

    // 再次检查（可能已由其他路径生成）
    if driver.exists(&path).await.unwrap_or(false) {
        return Ok(());
    }

    // 读取原文件 + 生成缩略图（CPU 密集部分走 blocking 线程池）
    let original = driver.get(&blob.storage_path).await?;
    let webp_bytes = tokio::task::spawn_blocking(move || generate_thumbnail(original))
        .await
        .map_aster_err_ctx(
            "thumbnail task panicked",
            AsterError::thumbnail_generation_failed,
        )??;

    driver.put(&path, &webp_bytes).await?;

    tracing::debug!("thumbnail generated for blob #{blob_id}");
    Ok(())
}

fn ensure_source_size_supported(blob: &file_blob::Model) -> Result<()> {
    if blob.size > MAX_THUMB_SOURCE_BYTES {
        return Err(AsterError::validation_error(format!(
            "thumbnail source exceeds {} MiB limit",
            MAX_THUMB_SOURCE_BYTES / 1024 / 1024
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_THUMB_SOURCE_BYTES, MAX_THUMB_WORKERS, ensure_source_size_supported,
        max_concurrent_thumbnails, thumb_path, thumbnail_etag_value,
    };
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
        assert!(ensure_source_size_supported(&blob_with_size(MAX_THUMB_SOURCE_BYTES)).is_ok());
    }

    #[test]
    fn rejects_thumbnail_source_above_size_limit() {
        assert!(ensure_source_size_supported(&blob_with_size(MAX_THUMB_SOURCE_BYTES + 1)).is_err());
    }

    #[test]
    fn thumbnail_worker_concurrency_is_memory_bounded() {
        assert!(max_concurrent_thumbnails() <= MAX_THUMB_WORKERS);
        assert!(max_concurrent_thumbnails() >= 1);
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
