//! 文件下载主链路。
//!
//! 下载有两种真正的出站方式：
//! - 服务端自己流式读取并回给客户端
//! - 对满足条件的 S3 附件下载返回 presigned redirect
//!
//! route / scope 层只决定“是否允许下载”，真正的传输策略在这里统一收口。

use std::time::Duration;

use actix_web::{HttpResponse, http::header};

use crate::db::repository::file_repo;
use crate::entities::{file, file_blob};
use crate::errors::{AsterError, Result};
use crate::runtime::AppState;
use crate::services::workspace_storage_service::WorkspaceStorageScope;
use crate::storage::driver::PresignedDownloadOptions;
use crate::types::{DriverType, S3DownloadStrategy, parse_storage_policy_options};

use super::{
    DownloadDisposition, ensure_personal_file_scope, get_info_in_scope, if_none_match_matches,
    inline_sandbox_csp, requires_inline_sandbox,
};

const PRESIGNED_DOWNLOAD_TTL_SECS: u64 = 5 * 60;

pub(crate) async fn download_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    id: i64,
    if_none_match: Option<&str>,
) -> Result<HttpResponse> {
    tracing::debug!(
        scope = ?scope,
        file_id = id,
        has_if_none_match = if_none_match.is_some(),
        "starting file download"
    );
    let file = get_info_in_scope(state, scope, id).await?;
    let blob = file_repo::find_blob_by_id(&state.db, file.blob_id).await?;
    build_download_response(state, &file, &blob, if_none_match).await
}

/// 下载文件（流式，不全量缓冲）
pub async fn download(
    state: &AppState,
    id: i64,
    user_id: i64,
    if_none_match: Option<&str>,
) -> Result<HttpResponse> {
    download_in_scope(
        state,
        WorkspaceStorageScope::Personal { user_id },
        id,
        if_none_match,
    )
    .await
}

/// 下载文件（无用户校验，用于分享链接，流式）
pub async fn download_raw(
    state: &AppState,
    id: i64,
    if_none_match: Option<&str>,
) -> Result<HttpResponse> {
    let db = &state.db;
    let f = file_repo::find_by_id(db, id).await?;
    ensure_personal_file_scope(&f)?;
    download_raw_unchecked_with_file(state, f, if_none_match).await
}

async fn download_raw_unchecked_with_file(
    state: &AppState,
    f: file::Model,
    if_none_match: Option<&str>,
) -> Result<HttpResponse> {
    let blob = file_repo::find_blob_by_id(&state.db, f.blob_id).await?;
    build_stream_response(state, &f, &blob, if_none_match).await
}

/// 构建流式下载响应
pub(crate) async fn build_stream_response(
    state: &AppState,
    f: &file::Model,
    blob: &file_blob::Model,
    if_none_match: Option<&str>,
) -> Result<HttpResponse> {
    build_stream_response_with_disposition(
        state,
        f,
        blob,
        DownloadDisposition::Attachment,
        if_none_match,
    )
    .await
}

pub(crate) async fn build_download_response(
    state: &AppState,
    f: &file::Model,
    blob: &file_blob::Model,
    if_none_match: Option<&str>,
) -> Result<HttpResponse> {
    build_download_response_with_disposition(
        state,
        f,
        blob,
        DownloadDisposition::Attachment,
        if_none_match,
    )
    .await
}

pub(crate) async fn build_download_response_with_disposition(
    state: &AppState,
    f: &file::Model,
    blob: &file_blob::Model,
    disposition: DownloadDisposition,
    if_none_match: Option<&str>,
) -> Result<HttpResponse> {
    if let Some(if_none_match) = if_none_match
        && if_none_match_matches(if_none_match, &blob.hash)
    {
        // 命中 If-None-Match 时仍走统一 response builder，
        // 这样 304 和 200 会共享相同的缓存头 / sandbox 头策略。
        return build_stream_response_with_disposition(
            state,
            f,
            blob,
            disposition,
            Some(if_none_match),
        )
        .await;
    }

    let policy = state.policy_snapshot.get_policy_or_err(blob.policy_id)?;
    let options = parse_storage_policy_options(policy.options.as_ref());
    let should_presign = policy.driver_type == DriverType::S3
        && disposition == DownloadDisposition::Attachment
        && options.effective_s3_download_strategy() == S3DownloadStrategy::Presigned;

    if should_presign {
        // 只有“附件下载 + S3 + 策略允许”才走 presigned redirect。
        // inline 预览仍由服务端统一加 CSP 和缓存头，避免把浏览器安全策略交给外部存储。
        return build_presigned_redirect_response(state, &policy, f, blob).await;
    }

    build_stream_response_with_disposition(state, f, blob, disposition, None).await
}

async fn build_presigned_redirect_response(
    state: &AppState,
    policy: &crate::entities::storage_policy::Model,
    f: &file::Model,
    blob: &file_blob::Model,
) -> Result<HttpResponse> {
    let driver = state.driver_registry.get_driver(policy)?;
    let url = driver
        .presigned_url(
            &blob.storage_path,
            Duration::from_secs(PRESIGNED_DOWNLOAD_TTL_SECS),
            PresignedDownloadOptions {
                response_cache_control: Some("private, max-age=0, must-revalidate".to_string()),
                response_content_disposition: Some(
                    DownloadDisposition::Attachment.header_value(&f.name),
                ),
                response_content_type: Some(f.mime_type.clone()),
            },
        )
        .await?
        .ok_or_else(|| {
            AsterError::storage_driver_error("presigned download not supported by driver")
        })?;

    tracing::debug!(
        file_id = f.id,
        blob_id = blob.id,
        policy_id = blob.policy_id,
        ttl_secs = PRESIGNED_DOWNLOAD_TTL_SECS,
        "redirecting file download to presigned S3 URL"
    );

    Ok(HttpResponse::Found()
        .insert_header((header::LOCATION, url))
        .insert_header((header::CACHE_CONTROL, "no-store"))
        .finish())
}

pub(crate) async fn build_stream_response_with_disposition(
    state: &AppState,
    f: &file::Model,
    blob: &file_blob::Model,
    disposition: DownloadDisposition,
    if_none_match: Option<&str>,
) -> Result<HttpResponse> {
    let requires_sandbox =
        disposition == DownloadDisposition::Inline && requires_inline_sandbox(&f.mime_type);

    if requires_sandbox {
        tracing::debug!(
            file_id = f.id,
            blob_id = blob.id,
            mime_type = %f.mime_type,
            "adding CSP sandbox for inline script-capable file"
        );
    }

    let etag = format!("\"{}\"", blob.hash);
    if let Some(if_none_match) = if_none_match
        && if_none_match_matches(if_none_match, &blob.hash)
    {
        tracing::debug!(
            file_id = f.id,
            blob_id = blob.id,
            disposition = ?disposition,
            "serving cached file response with 304"
        );
        let mut response = HttpResponse::NotModified();
        response.insert_header(("ETag", etag));
        response.insert_header(("Cache-Control", "private, max-age=0, must-revalidate"));
        if requires_sandbox {
            response.insert_header(("Content-Security-Policy", inline_sandbox_csp()));
            response.insert_header(("X-Content-Type-Options", "nosniff"));
        }
        return Ok(response.finish());
    }

    let policy = state.policy_snapshot.get_policy_or_err(blob.policy_id)?;
    let driver = state.driver_registry.get_driver(&policy)?;
    // 主下载链路必须保持流式读取；不要改回 driver.get() 的全量缓冲实现。
    let stream = driver.get_stream(&blob.storage_path).await?;

    // 64KB buffer — 比默认 4KB 减少系统调用和分配开销
    let reader_stream = tokio_util::io::ReaderStream::with_capacity(stream, 64 * 1024);

    tracing::debug!(
        file_id = f.id,
        blob_id = blob.id,
        policy_id = blob.policy_id,
        size = blob.size,
        disposition = ?disposition,
        "building streaming file response"
    );

    let mut response = HttpResponse::Ok();
    response.content_type(f.mime_type.clone());
    response.insert_header(("Content-Length", blob.size.to_string()));
    response.insert_header(("Content-Disposition", disposition.header_value(&f.name)));
    response.insert_header(("ETag", etag));
    response.insert_header(("Cache-Control", "private, max-age=0, must-revalidate"));
    if requires_sandbox {
        response.insert_header(("Content-Security-Policy", inline_sandbox_csp()));
        response.insert_header(("X-Content-Type-Options", "nosniff"));
    }
    // 跳过全局 Compress 中间件，避免压缩编码器为了攒出更大的压缩块而额外缓存，
    // 让大文件下载从“稳定流式”退化成高内存占用。
    response.insert_header(("Content-Encoding", "identity"));
    Ok(response.streaming(reader_stream))
}

#[cfg(test)]
mod tests {
    use super::{DownloadDisposition, build_stream_response_with_disposition};
    use crate::cache;
    use crate::config::{CacheConfig, Config, DatabaseConfig, RuntimeConfig};
    use crate::db::repository::file_repo;
    use crate::entities::{file, file_blob, storage_policy, user};
    use crate::runtime::AppState;
    use crate::services::{mail_service, policy_service};
    use crate::storage::driver::BlobMetadata;
    use crate::storage::{DriverRegistry, PolicySnapshot, StorageDriver};
    use crate::types::{DriverType, StoredStoragePolicyAllowedTypes, UserRole, UserStatus};
    use actix_web::body;
    use async_trait::async_trait;
    use chrono::Utc;
    use migration::{Migrator, MigratorTrait};
    use sea_orm::{ActiveModelTrait, Set};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use std::time::Duration;
    use tokio::io::{AsyncRead, AsyncWriteExt};

    #[derive(Clone)]
    struct CountingStreamDriver {
        bytes: Arc<Vec<u8>>,
        get_calls: Arc<AtomicUsize>,
        get_stream_calls: Arc<AtomicUsize>,
    }

    impl CountingStreamDriver {
        fn new(bytes: Vec<u8>) -> Self {
            Self {
                bytes: Arc::new(bytes),
                get_calls: Arc::new(AtomicUsize::new(0)),
                get_stream_calls: Arc::new(AtomicUsize::new(0)),
            }
        }
    }

    #[async_trait]
    impl StorageDriver for CountingStreamDriver {
        async fn put(&self, path: &str, _data: &[u8]) -> crate::errors::Result<String> {
            Ok(path.to_string())
        }

        async fn get(&self, _path: &str) -> crate::errors::Result<Vec<u8>> {
            self.get_calls.fetch_add(1, Ordering::SeqCst);
            Err(crate::errors::AsterError::storage_driver_error(
                "download stream regression: get() should not be used here",
            ))
        }

        async fn get_stream(
            &self,
            _path: &str,
        ) -> crate::errors::Result<Box<dyn AsyncRead + Unpin + Send>> {
            self.get_stream_calls.fetch_add(1, Ordering::SeqCst);
            let (mut writer, reader) = tokio::io::duplex(self.bytes.len().max(1));
            let payload = self.bytes.as_ref().clone();
            tokio::spawn(async move {
                let _ = writer.write_all(&payload).await;
                let _ = writer.shutdown().await;
            });
            Ok(Box::new(reader))
        }

        async fn delete(&self, _path: &str) -> crate::errors::Result<()> {
            Ok(())
        }

        async fn exists(&self, _path: &str) -> crate::errors::Result<bool> {
            Ok(true)
        }

        async fn metadata(&self, _path: &str) -> crate::errors::Result<BlobMetadata> {
            Ok(BlobMetadata {
                size: self.bytes.len() as u64,
                content_type: Some("text/plain".to_string()),
            })
        }

        async fn put_file(
            &self,
            storage_path: &str,
            _local_path: &str,
        ) -> crate::errors::Result<String> {
            Ok(storage_path.to_string())
        }

        async fn presigned_url(
            &self,
            _path: &str,
            _expires: Duration,
            _options: crate::storage::driver::PresignedDownloadOptions,
        ) -> crate::errors::Result<Option<String>> {
            Ok(None)
        }
    }

    async fn build_download_test_state(
        driver: CountingStreamDriver,
        payload_size: i64,
    ) -> (
        AppState,
        file::Model,
        file_blob::Model,
        CountingStreamDriver,
    ) {
        let temp_root = std::env::temp_dir().join(format!(
            "asterdrive-download-stream-test-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&temp_root).expect("download test temp root should exist");

        let db = crate::db::connect(&DatabaseConfig {
            url: "sqlite::memory:".to_string(),
            pool_size: 1,
            retry_count: 0,
        })
        .await
        .expect("download test database should connect");
        Migrator::up(&db, None)
            .await
            .expect("download test migrations should succeed");

        let now = Utc::now();
        let policy = storage_policy::ActiveModel {
            name: Set("Download Stream Policy".to_string()),
            driver_type: Set(DriverType::Local),
            endpoint: Set(String::new()),
            bucket: Set(String::new()),
            access_key: Set(String::new()),
            secret_key: Set(String::new()),
            base_path: Set(temp_root.to_string_lossy().into_owned()),
            max_file_size: Set(0),
            allowed_types: Set(StoredStoragePolicyAllowedTypes::empty()),
            options: Set(crate::types::StoredStoragePolicyOptions::empty()),
            is_default: Set(true),
            chunk_size: Set(5_242_880),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        }
        .insert(&db)
        .await
        .expect("download test policy should be inserted");

        let user = user::ActiveModel {
            username: Set("dldstream".to_string()),
            email: Set("dldstream@example.com".to_string()),
            password_hash: Set("unused".to_string()),
            role: Set(UserRole::User),
            status: Set(UserStatus::Active),
            session_version: Set(0),
            email_verified_at: Set(Some(now)),
            pending_email: Set(None),
            storage_used: Set(0),
            storage_quota: Set(0),
            policy_group_id: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
            config: Set(None),
            ..Default::default()
        }
        .insert(&db)
        .await
        .expect("download test user should be inserted");

        policy_service::ensure_policy_groups_seeded(&db)
            .await
            .expect("download test policy groups should be seeded");

        let policy_snapshot = Arc::new(PolicySnapshot::new());
        policy_snapshot
            .reload(&db)
            .await
            .expect("download test policy snapshot should reload");

        let driver_registry = Arc::new(DriverRegistry::new());
        driver_registry.insert_for_test(policy.id, Arc::new(driver.clone()));

        let runtime_config = Arc::new(RuntimeConfig::new());
        let cache = cache::create_cache(&CacheConfig {
            enabled: false,
            ..Default::default()
        })
        .await;

        let mut config = Config::default();
        config.server.temp_dir = temp_root.join(".tmp").to_string_lossy().into_owned();
        config.server.upload_temp_dir = temp_root.join(".uploads").to_string_lossy().into_owned();

        let (storage_change_tx, _) = tokio::sync::broadcast::channel(
            crate::services::storage_change_service::STORAGE_CHANGE_CHANNEL_CAPACITY,
        );

        let state = AppState {
            db: db.clone(),
            driver_registry,
            runtime_config: runtime_config.clone(),
            policy_snapshot,
            config: Arc::new(config),
            cache,
            mail_sender: mail_service::runtime_sender(runtime_config),
            storage_change_tx,
        };

        let blob = file_repo::create_blob(
            &db,
            file_blob::ActiveModel {
                hash: Set(format!("download-stream-{}", uuid::Uuid::new_v4())),
                size: Set(payload_size),
                policy_id: Set(policy.id),
                storage_path: Set(format!("files/{}", uuid::Uuid::new_v4())),
                ref_count: Set(1),
                created_at: Set(now),
                updated_at: Set(now),
                ..Default::default()
            },
        )
        .await
        .expect("download test blob should be inserted");

        let file = file_repo::create(
            &db,
            file::ActiveModel {
                name: Set("download.txt".to_string()),
                folder_id: Set(None),
                team_id: Set(None),
                blob_id: Set(blob.id),
                size: Set(payload_size),
                user_id: Set(user.id),
                mime_type: Set("text/plain".to_string()),
                created_at: Set(now),
                updated_at: Set(now),
                deleted_at: Set(None),
                is_locked: Set(false),
                ..Default::default()
            },
        )
        .await
        .expect("download test file should be inserted");

        (state, file, blob, driver)
    }

    #[actix_web::test]
    async fn build_stream_response_uses_get_stream_instead_of_get() {
        let payload = b"streamed download payload".to_vec();
        let driver = CountingStreamDriver::new(payload.clone());
        let get_calls = driver.get_calls.clone();
        let get_stream_calls = driver.get_stream_calls.clone();
        let (state, file, blob, _) = build_download_test_state(driver, payload.len() as i64).await;

        let response = build_stream_response_with_disposition(
            &state,
            &file,
            &blob,
            DownloadDisposition::Attachment,
            None,
        )
        .await
        .expect("stream download response should build");

        let body = body::to_bytes(response.into_body())
            .await
            .expect("stream response body should read");
        assert_eq!(body.as_ref(), payload.as_slice());
        assert_eq!(
            get_calls.load(Ordering::SeqCst),
            0,
            "download response must not fall back to StorageDriver::get()"
        );
        assert_eq!(
            get_stream_calls.load(Ordering::SeqCst),
            1,
            "download response should open exactly one streaming reader"
        );
    }
}
