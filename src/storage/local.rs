use super::driver::{BlobMetadata, StorageDriver, StoragePathVisitor};
use crate::entities::storage_policy;
use crate::errors::{AsterError, MapAsterErr, Result};
use async_trait::async_trait;
use std::path::PathBuf;
use std::time::Duration;
use tokio::io::AsyncRead;

pub struct LocalDriver {
    base_path: PathBuf,
}

pub fn effective_base_path(policy: &storage_policy::Model) -> PathBuf {
    if policy.base_path.is_empty() {
        PathBuf::from("./data")
    } else {
        PathBuf::from(&policy.base_path)
    }
}

pub fn upload_staging_path(policy: &storage_policy::Model, name: &str) -> PathBuf {
    effective_base_path(policy)
        .join(".staging")
        .join(name.trim_start_matches('/'))
}

impl LocalDriver {
    pub fn new(policy: &storage_policy::Model) -> Result<Self> {
        Ok(Self {
            base_path: effective_base_path(policy),
        })
    }

    fn full_path(&self, path: &str) -> PathBuf {
        self.base_path.join(path.trim_start_matches('/'))
    }
}

fn collect_local_paths(
    root: &std::path::Path,
    current: &std::path::Path,
    output: &mut Vec<String>,
) -> std::io::Result<()> {
    if !current.exists() {
        return Ok(());
    }

    for entry in std::fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_local_paths(root, &path, output)?;
            continue;
        }
        if !file_type.is_file() {
            continue;
        }

        let relative = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        output.push(relative);
    }

    Ok(())
}

#[async_trait]
impl StorageDriver for LocalDriver {
    async fn put(&self, path: &str, data: &[u8]) -> Result<String> {
        let full = self.full_path(path);
        if let Some(parent) = full.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_aster_err(AsterError::storage_driver_error)?;
        }
        tokio::fs::write(&full, data)
            .await
            .map_aster_err(AsterError::storage_driver_error)?;
        Ok(path.to_string())
    }

    async fn get(&self, path: &str) -> Result<Vec<u8>> {
        tokio::fs::read(self.full_path(path))
            .await
            .map_aster_err(AsterError::storage_driver_error)
    }

    async fn get_stream(&self, path: &str) -> Result<Box<dyn AsyncRead + Unpin + Send>> {
        let file = tokio::fs::File::open(self.full_path(path))
            .await
            .map_aster_err(AsterError::storage_driver_error)?;
        Ok(Box::new(file))
    }

    async fn delete(&self, path: &str) -> Result<()> {
        tokio::fs::remove_file(self.full_path(path))
            .await
            .map_aster_err(AsterError::storage_driver_error)
    }

    async fn exists(&self, path: &str) -> Result<bool> {
        Ok(self.full_path(path).exists())
    }

    async fn metadata(&self, path: &str) -> Result<BlobMetadata> {
        let meta = tokio::fs::metadata(self.full_path(path))
            .await
            .map_aster_err(AsterError::storage_driver_error)?;
        Ok(BlobMetadata {
            size: meta.len(),
            content_type: None,
        })
    }

    async fn list_paths(&self, prefix: Option<&str>) -> Result<Vec<String>> {
        let root = self.base_path.clone();
        let start = prefix.map_or_else(|| root.clone(), |prefix| self.full_path(prefix));

        tokio::task::spawn_blocking(move || {
            let mut paths = Vec::new();
            collect_local_paths(&root, &start, &mut paths)?;
            paths.sort();
            Ok::<Vec<String>, std::io::Error>(paths)
        })
        .await
        .map_aster_err_ctx("list local paths", AsterError::storage_driver_error)?
        .map_aster_err_ctx("list local paths", AsterError::storage_driver_error)
    }

    async fn scan_paths(
        &self,
        prefix: Option<&str>,
        visitor: &mut dyn StoragePathVisitor,
    ) -> Result<()> {
        let root = self.base_path.clone();
        let start = prefix.map_or_else(|| root.clone(), |prefix| self.full_path(prefix));
        let metadata = match tokio::fs::metadata(&start).await {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => {
                return Err(AsterError::storage_driver_error(format!(
                    "scan local paths metadata: {error}"
                )));
            }
        };

        if metadata.is_file() {
            let relative = start
                .strip_prefix(&root)
                .unwrap_or(&start)
                .to_string_lossy()
                .replace('\\', "/");
            visitor.visit_path(relative)?;
            return Ok(());
        }

        let mut pending_dirs = vec![start];
        while let Some(current_dir) = pending_dirs.pop() {
            let mut entries = tokio::fs::read_dir(&current_dir).await.map_aster_err_ctx(
                "scan local paths read_dir",
                AsterError::storage_driver_error,
            )?;
            let mut child_dirs = Vec::new();
            let mut child_files = Vec::new();

            while let Some(entry) = entries.next_entry().await.map_aster_err_ctx(
                "scan local paths next_entry",
                AsterError::storage_driver_error,
            )? {
                let path = entry.path();
                let file_type = entry.file_type().await.map_aster_err_ctx(
                    "scan local paths file_type",
                    AsterError::storage_driver_error,
                )?;

                if file_type.is_dir() {
                    child_dirs.push(path);
                } else if file_type.is_file() {
                    child_files.push(path);
                }
            }

            child_dirs.sort();
            child_files.sort();

            for file_path in child_files {
                let relative = file_path
                    .strip_prefix(&root)
                    .unwrap_or(&file_path)
                    .to_string_lossy()
                    .replace('\\', "/");
                visitor.visit_path(relative)?;
            }

            for child_dir in child_dirs.into_iter().rev() {
                pending_dirs.push(child_dir);
            }
        }

        Ok(())
    }

    async fn put_file(&self, storage_path: &str, local_path: &str) -> Result<String> {
        let full = self.full_path(storage_path);
        if let Some(parent) = full.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_aster_err(AsterError::storage_driver_error)?;
        }
        // rename 是零拷贝（同一文件系统），跨文件系统 fallback 到 copy + delete
        if tokio::fs::rename(local_path, &full).await.is_err() {
            tokio::fs::copy(local_path, &full)
                .await
                .map_aster_err_ctx("copy file", AsterError::storage_driver_error)?;
            let _ = tokio::fs::remove_file(local_path).await;
        }
        Ok(storage_path.to_string())
    }

    async fn presigned_url(&self, _path: &str, _expires: Duration) -> Result<Option<String>> {
        Ok(None)
    }
}
