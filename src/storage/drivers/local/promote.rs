use std::path::Path;

use crate::errors::{AsterError, MapAsterErr, Result};
use crate::storage::driver::StorageDriver;
use crate::utils::numbers;

const LOCAL_COPY_BUF_SIZE: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromoteLocalFileOutcome {
    Created,
    AlreadyExists,
}

impl PromoteLocalFileOutcome {
    pub fn created(self) -> bool {
        matches!(self, Self::Created)
    }
}

pub async fn promote_local_file_if_absent(
    driver: &dyn StorageDriver,
    storage_path: &str,
    local_path: &str,
    expected_size: i64,
) -> Result<PromoteLocalFileOutcome> {
    promote_local_file_if_absent_inner(
        driver,
        storage_path,
        local_path,
        expected_size,
        false,
        || Ok(()),
    )
    .await
}

pub async fn promote_local_file_if_absent_with_check(
    driver: &dyn StorageDriver,
    storage_path: &str,
    local_path: &str,
    expected_size: i64,
    checkpoint: impl Fn() -> Result<()>,
) -> Result<PromoteLocalFileOutcome> {
    promote_local_file_if_absent_inner(
        driver,
        storage_path,
        local_path,
        expected_size,
        true,
        checkpoint,
    )
    .await
}

async fn promote_local_file_if_absent_inner(
    driver: &dyn StorageDriver,
    storage_path: &str,
    local_path: &str,
    expected_size: i64,
    preserve_source: bool,
    checkpoint: impl Fn() -> Result<()>,
) -> Result<PromoteLocalFileOutcome> {
    checkpoint()?;
    let local_driver = driver.as_local_path().ok_or_else(|| {
        AsterError::storage_driver_error("local path storage driver not supported")
    })?;
    let target = local_driver.resolve_local_path(storage_path)?;
    checkpoint()?;
    if let Some(parent) = target.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_aster_err(AsterError::storage_driver_error)?;
    }
    checkpoint()?;

    let expected_size = numbers::i64_to_u64(expected_size, "local dedup blob size")?;
    match tokio::fs::hard_link(local_path, &target).await {
        Ok(()) => match validate_existing_local_blob_size(&target, expected_size).await {
            Ok(()) => {
                cleanup_promoted_source(local_path, preserve_source).await;
                Ok(PromoteLocalFileOutcome::Created)
            }
            Err(error) => {
                if let Err(cleanup_error) = tokio::fs::remove_file(&target).await
                    && cleanup_error.kind() != std::io::ErrorKind::NotFound
                {
                    tracing::warn!(
                        target = %target.display(),
                        "failed to cleanup invalid promoted local blob: {cleanup_error}"
                    );
                }
                Err(error)
            }
        },
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            validate_existing_local_blob_size(&target, expected_size).await?;
            cleanup_promoted_source(local_path, preserve_source).await;
            Ok(PromoteLocalFileOutcome::AlreadyExists)
        }
        Err(link_error) => {
            promote_local_file_via_temp_copy(
                local_path,
                &target,
                expected_size,
                link_error,
                preserve_source,
                checkpoint,
            )
            .await
        }
    }
}

async fn promote_local_file_via_temp_copy(
    local_path: &str,
    target: &Path,
    expected_size: u64,
    link_error: std::io::Error,
    preserve_source: bool,
    checkpoint: impl Fn() -> Result<()>,
) -> Result<PromoteLocalFileOutcome> {
    let Some(parent) = target.parent() else {
        return Err(AsterError::storage_driver_error(format!(
            "local dedup target has no parent: {}",
            target.display()
        )));
    };
    let temp_name = format!(".aster-promote-{}.tmp", crate::utils::id::new_uuid());
    let temp_path = parent.join(temp_name);

    match copy_file_to_temp(local_path, &temp_path, expected_size, &checkpoint).await {
        Ok(()) => {}
        Err(error) => {
            if let Err(cleanup_error) = tokio::fs::remove_file(&temp_path).await
                && cleanup_error.kind() != std::io::ErrorKind::NotFound
            {
                tracing::warn!(
                    temp_path = %temp_path.display(),
                    "failed to cleanup local dedup temp copy after copy error: {cleanup_error}"
                );
            }
            return Err(error);
        }
    }
    checkpoint()?;

    let result = match tokio::fs::hard_link(&temp_path, target).await {
        Ok(()) => {
            cleanup_promoted_source(local_path, preserve_source).await;
            Ok(PromoteLocalFileOutcome::Created)
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            validate_existing_local_blob_size(target, expected_size).await?;
            checkpoint()?;
            cleanup_promoted_source(local_path, preserve_source).await;
            Ok(PromoteLocalFileOutcome::AlreadyExists)
        }
        Err(error) => Err(AsterError::storage_driver_error(format!(
            "promote local dedup blob with no-clobber link failed after initial link error ({link_error}): {error}"
        ))),
    };

    if let Err(cleanup_error) = tokio::fs::remove_file(&temp_path).await
        && cleanup_error.kind() != std::io::ErrorKind::NotFound
    {
        tracing::warn!(
            temp_path = %temp_path.display(),
            "failed to cleanup local dedup temp copy: {cleanup_error}"
        );
    }
    result
}

async fn cleanup_promoted_source(local_path: &str, preserve_source: bool) {
    if !preserve_source {
        crate::utils::cleanup_temp_file(local_path).await;
    }
}

async fn copy_file_to_temp(
    local_path: &str,
    temp_path: &Path,
    expected_size: u64,
    checkpoint: impl Fn() -> Result<()>,
) -> Result<()> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    checkpoint()?;
    let mut source = tokio::fs::File::open(local_path)
        .await
        .map_aster_err_ctx("open local dedup source", AsterError::storage_driver_error)?;
    let mut target = tokio::fs::File::create(temp_path)
        .await
        .map_aster_err_ctx("create local dedup temp", AsterError::storage_driver_error)?;
    let mut buf = vec![0_u8; LOCAL_COPY_BUF_SIZE];
    let mut copied = 0_u64;

    loop {
        checkpoint()?;
        let read = source
            .read(&mut buf)
            .await
            .map_aster_err_ctx("read local dedup source", AsterError::storage_driver_error)?;
        if read == 0 {
            break;
        }
        target
            .write_all(&buf[..read])
            .await
            .map_aster_err_ctx("write local dedup temp", AsterError::storage_driver_error)?;
        let read = u64::try_from(read).map_err(|_| {
            AsterError::storage_driver_error("local dedup read size exceeds u64 range")
        })?;
        copied = copied
            .checked_add(read)
            .ok_or_else(|| AsterError::storage_driver_error("local dedup copy size overflow"))?;
    }
    target
        .flush()
        .await
        .map_aster_err_ctx("flush local dedup temp", AsterError::storage_driver_error)?;
    checkpoint()?;

    if copied != expected_size {
        return Err(AsterError::storage_driver_error(format!(
            "local dedup temp copy size mismatch: expected {expected_size}, copied {copied}"
        )));
    }
    Ok(())
}

async fn validate_existing_local_blob_size(target: &Path, expected_size: u64) -> Result<()> {
    let metadata = tokio::fs::metadata(target).await.map_aster_err_ctx(
        "inspect existing local dedup blob",
        AsterError::storage_driver_error,
    )?;
    if metadata.len() != expected_size {
        return Err(AsterError::storage_driver_error(format!(
            "existing local dedup blob size mismatch for {}: expected {}, actual {}",
            target.display(),
            expected_size,
            metadata.len()
        )));
    }
    Ok(())
}
