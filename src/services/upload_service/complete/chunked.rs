use chrono::Utc;

use crate::db::repository::file_repo;
use crate::entities::{file, storage_policy, upload_session};
use crate::errors::{AsterError, MapAsterErr, Result};
use crate::runtime::PrimaryAppState;
use crate::services::upload_service::shared::{
    mark_session_failed, transition_upload_session_to_assembling,
};
use crate::services::workspace_storage_service::{self, PreparedNonDedupBlobUpload};
use crate::storage::driver::StorageDriver;
use crate::types::{DriverType, UploadSessionStatus};
use crate::utils::numbers::usize_to_i64;
use crate::utils::paths;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;

struct AssembledTempFile {
    path: String,
    size: i64,
    file_hash: Option<String>,
}

enum AssembledBlobPlan {
    Dedup {
        file_hash: String,
        storage_path: String,
    },
    Preuploaded(PreparedNonDedupBlobUpload),
}

pub(super) async fn complete_chunked_upload(
    state: &PrimaryAppState,
    session: upload_session::Model,
) -> Result<file::Model> {
    let db = &state.db;
    let upload_id = session.id.clone();

    transition_upload_session_to_assembling(
        db,
        &upload_id,
        session.status,
        UploadSessionStatus::Uploading,
    )
    .await?;

    let policy = state.policy_snapshot.get_policy_or_err(session.policy_id)?;
    let driver = state.driver_registry.get_driver(&policy)?;
    let result = finalize_chunked_upload_session(state, &session, &policy, driver.as_ref()).await;

    match result {
        Ok(created) => {
            let temp_dir = paths::upload_temp_dir(&state.config.server.upload_temp_dir, &upload_id);
            crate::utils::cleanup_temp_dir(&temp_dir).await;
            tracing::debug!(
                upload_id = %upload_id,
                file_id = created.id,
                blob_id = created.blob_id,
                size = created.size,
                "completed upload session"
            );
            Ok(created)
        }
        Err(error) => {
            // session 一旦进入 failed，就不允许客户端继续 retry 当前 upload_id，
            // 必须重新 init 一个新的会话，避免半成品状态被反复叠加。
            mark_session_failed(db, &upload_id).await;
            Err(error)
        }
    }
}

async fn finalize_chunked_upload_session(
    state: &PrimaryAppState,
    session: &upload_session::Model,
    policy: &storage_policy::Model,
    driver: &dyn StorageDriver,
) -> Result<file::Model> {
    if policy.driver_type == DriverType::Remote {
        return finalize_remote_chunked_upload_session(state, session, policy, driver).await;
    }

    let assembled = assemble_local_chunks_to_temp_file(
        state,
        session,
        workspace_storage_service::local_content_dedup_enabled(policy),
    )
    .await?;
    let blob_plan = stage_assembled_blob_upload(driver, policy, &assembled).await?;
    persist_assembled_upload(
        state,
        session,
        driver,
        policy.id,
        assembled.size,
        &blob_plan,
    )
    .await
}

async fn finalize_remote_chunked_upload_session(
    state: &PrimaryAppState,
    session: &upload_session::Model,
    policy: &storage_policy::Model,
    driver: &dyn StorageDriver,
) -> Result<file::Model> {
    const CHUNK_RELAY_BUFFER_SIZE: usize = 64 * 1024;

    let prepared =
        workspace_storage_service::prepare_non_dedup_blob_upload(policy, session.total_size);
    let (writer, reader) = tokio::io::duplex(CHUNK_RELAY_BUFFER_SIZE);
    let relay_task = tokio::spawn(stream_local_chunks_into_writer(
        state.config.server.upload_temp_dir.clone(),
        session.id.clone(),
        session.total_chunks,
        writer,
    ));

    let upload_result = workspace_storage_service::upload_reader_to_prepared_blob(
        driver,
        &prepared,
        Box::new(reader),
        session.total_size,
    )
    .await;

    let relay_result = relay_task.await.map_err(|error| {
        AsterError::upload_assembly_failed(format!("remote chunk relay task failed: {error}"))
    })?;

    upload_result?;
    if let Err(error) = relay_result {
        workspace_storage_service::cleanup_preuploaded_blob_upload(
            driver,
            &prepared,
            "chunked upload relay error",
        )
        .await;
        return Err(error);
    }

    persist_preuploaded_chunked_upload(state, session, driver, &prepared).await
}

async fn stream_local_chunks_into_writer(
    upload_temp_dir: String,
    upload_id: String,
    total_chunks: i32,
    mut writer: tokio::io::DuplexStream,
) -> Result<()> {
    const STREAM_BUFFER_SIZE: usize = 64 * 1024;

    let mut buffer = vec![0u8; STREAM_BUFFER_SIZE];
    for chunk_number in 0..total_chunks {
        let chunk_path = paths::upload_chunk_path(&upload_temp_dir, &upload_id, chunk_number);
        let mut chunk_file = tokio::fs::File::open(&chunk_path).await.map_aster_err_ctx(
            &format!("open chunk {chunk_number}"),
            AsterError::upload_assembly_failed,
        )?;

        loop {
            let read = chunk_file.read(&mut buffer).await.map_aster_err_ctx(
                &format!("read chunk {chunk_number}"),
                AsterError::upload_assembly_failed,
            )?;
            if read == 0 {
                break;
            }
            writer
                .write_all(&buffer[..read])
                .await
                .map_aster_err_ctx("relay remote chunk", AsterError::upload_assembly_failed)?;
        }
    }

    writer.shutdown().await.map_aster_err_ctx(
        "shutdown remote chunk relay",
        AsterError::upload_assembly_failed,
    )?;
    Ok(())
}

async fn assemble_local_chunks_to_temp_file(
    state: &PrimaryAppState,
    session: &upload_session::Model,
    should_dedup: bool,
) -> Result<AssembledTempFile> {
    use sha2::{Digest, Sha256};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    const ASSEMBLY_BUFFER_SIZE: usize = 64 * 1024;

    let upload_id = session.id.as_str();
    let assembled_path =
        paths::upload_assembled_path(&state.config.server.upload_temp_dir, upload_id);
    let mut out_file = tokio::fs::File::create(&assembled_path)
        .await
        .map_aster_err_ctx("create assembled file", AsterError::upload_assembly_failed)?;
    let mut hasher = should_dedup.then(Sha256::new);
    let mut size: i64 = 0;
    let mut buffer = vec![0u8; ASSEMBLY_BUFFER_SIZE];

    // 本地 chunk 模式：先按顺序把所有 chunk 拼成 assembled 文件。
    // 如果 local 策略启用了 dedup，会在拼装过程中顺便流式计算 hash，
    // 避免第二遍再把 assembled 文件完整读一遍。
    for i in 0..session.total_chunks {
        let chunk_path =
            paths::upload_chunk_path(&state.config.server.upload_temp_dir, upload_id, i);
        let mut chunk_file = tokio::fs::File::open(&chunk_path).await.map_aster_err_ctx(
            &format!("open chunk {i}"),
            AsterError::upload_assembly_failed,
        )?;

        loop {
            let n = chunk_file.read(&mut buffer).await.map_aster_err_ctx(
                &format!("read chunk {i}"),
                AsterError::upload_assembly_failed,
            )?;
            if n == 0 {
                break;
            }

            let data = &buffer[..n];
            if let Some(hasher) = hasher.as_mut() {
                hasher.update(data);
            }
            let chunk_len = usize_to_i64(n, "assembled chunk length")?;
            size = size.checked_add(chunk_len).ok_or_else(|| {
                AsterError::upload_assembly_failed("assembled upload size exceeds i64 range")
            })?;
            out_file
                .write_all(data)
                .await
                .map_aster_err_ctx("write assembled", AsterError::upload_assembly_failed)?;
        }
    }
    out_file
        .flush()
        .await
        .map_aster_err_ctx("flush assembled", AsterError::upload_assembly_failed)?;
    drop(out_file);

    Ok(AssembledTempFile {
        path: assembled_path,
        size,
        file_hash: hasher
            .map(|hasher| crate::utils::hash::sha256_digest_to_hex(&hasher.finalize())),
    })
}

async fn stage_assembled_blob_upload(
    driver: &dyn StorageDriver,
    policy: &storage_policy::Model,
    assembled: &AssembledTempFile,
) -> Result<AssembledBlobPlan> {
    if let Some(file_hash) = assembled.file_hash.as_ref() {
        let storage_path = crate::utils::storage_path_from_hash(file_hash);

        // exists() 作为冗余 PUT 的软短路：失败/返回 false 都会退化为一次 PUT，
        // 语义等同；真正的并发安全由内容寻址 + find_or_create_blob 保证。
        let already_stored = driver.exists(&storage_path).await.unwrap_or(false);
        if already_stored {
            crate::utils::cleanup_temp_file(&assembled.path).await;
        } else {
            let stream_driver = driver
                .as_stream_upload()
                .ok_or_else(|| AsterError::storage_driver_error("stream upload not supported"))?;
            stream_driver
                .put_file(&storage_path, &assembled.path)
                .await?;
        }

        return Ok(AssembledBlobPlan::Dedup {
            file_hash: file_hash.clone(),
            storage_path,
        });
    }

    // 不做 dedup 的情况下，先为 blob 预分配最终 key，再把 assembled 文件传上去。
    // 失败只会留下孤儿 storage 对象，由 blob GC 自然回收。
    let preuploaded =
        workspace_storage_service::prepare_non_dedup_blob_upload(policy, assembled.size);
    workspace_storage_service::upload_temp_file_to_prepared_blob(
        driver,
        &preuploaded,
        &assembled.path,
    )
    .await?;
    Ok(AssembledBlobPlan::Preuploaded(preuploaded))
}

async fn persist_assembled_upload(
    state: &PrimaryAppState,
    session: &upload_session::Model,
    driver: &dyn StorageDriver,
    policy_id: i64,
    size: i64,
    blob_plan: &AssembledBlobPlan,
) -> Result<file::Model> {
    let now = Utc::now();
    let create_result = async {
        let txn = crate::db::transaction::begin(&state.db).await?;

        let blob = match blob_plan {
            AssembledBlobPlan::Dedup {
                file_hash,
                storage_path,
            } => {
                let blob =
                    file_repo::find_or_create_blob(&txn, file_hash, size, policy_id, storage_path)
                        .await?;
                blob.model
            }
            AssembledBlobPlan::Preuploaded(preuploaded) => {
                workspace_storage_service::persist_preuploaded_blob(&txn, preuploaded).await?
            }
        };

        let created =
            workspace_storage_service::finalize_upload_session_blob(&txn, session, &blob, now)
                .await?;

        crate::db::transaction::commit(txn).await?;
        Ok::<file::Model, AsterError>(created)
    }
    .await;

    match create_result {
        Ok(created) => Ok(created),
        Err(error) => {
            if let AssembledBlobPlan::Preuploaded(preuploaded) = blob_plan {
                workspace_storage_service::cleanup_preuploaded_blob_upload(
                    driver,
                    preuploaded,
                    "chunked upload DB error after storing assembled blob",
                )
                .await;
            }
            // dedup 失败不主动删 storage 对象：另一路并发上传可能正在引用同内容的 blob，
            // 删除会造成 ref=1 的活 blob 丢数据；留给 orphan-blob GC 处理。
            Err(error)
        }
    }
}

async fn persist_preuploaded_chunked_upload(
    state: &PrimaryAppState,
    session: &upload_session::Model,
    driver: &dyn StorageDriver,
    prepared: &PreparedNonDedupBlobUpload,
) -> Result<file::Model> {
    let now = Utc::now();
    let create_result = async {
        let txn = crate::db::transaction::begin(&state.db).await?;
        let blob = workspace_storage_service::persist_preuploaded_blob(&txn, prepared).await?;
        let created =
            workspace_storage_service::finalize_upload_session_blob(&txn, session, &blob, now)
                .await?;
        crate::db::transaction::commit(txn).await?;
        Ok::<file::Model, AsterError>(created)
    }
    .await;

    match create_result {
        Ok(created) => Ok(created),
        Err(error) => {
            workspace_storage_service::cleanup_preuploaded_blob_upload(
                driver,
                prepared,
                "chunked upload DB error after streaming preuploaded blob",
            )
            .await;
            Err(error)
        }
    }
}
