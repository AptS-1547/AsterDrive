//! 存储驱动实现：`local`。

use crate::entities::storage_policy;
use crate::errors::{AsterError, MapAsterErr, Result};
use crate::storage::driver::{BlobMetadata, StorageDriver, StoragePathVisitor};
use crate::storage::extensions::{ListStorageDriver, LocalPathStorageDriver, StreamUploadDriver};
use crate::utils::numbers;
use async_trait::async_trait;
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};
use tokio::io::{AsyncRead, AsyncSeekExt, AsyncWriteExt};

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

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();

    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => match normalized.components().next_back() {
                Some(Component::Normal(_)) => {
                    normalized.pop();
                }
                Some(Component::RootDir) | Some(Component::Prefix(_)) => {}
                _ => normalized.push(component.as_os_str()),
            },
            Component::RootDir | Component::Prefix(_) | Component::Normal(_) => {
                normalized.push(component.as_os_str());
            }
        }
    }

    if normalized.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        normalized
    }
}

fn absolute_path(path: &Path) -> Result<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_aster_err_ctx(
                "resolve local storage current_dir",
                AsterError::storage_driver_error,
            )?
            .join(path)
    };
    Ok(normalize_path(&absolute))
}

fn resolve_existing_path(path: &Path) -> Result<PathBuf> {
    let mut probe = absolute_path(path)?;
    let mut missing_suffix = Vec::<OsString>::new();

    loop {
        match std::fs::symlink_metadata(&probe) {
            Ok(_) => break,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let Some(name) = probe.file_name() else {
                    return Err(AsterError::storage_driver_error(format!(
                        "local path has no existing ancestor: {}",
                        path.display()
                    )));
                };
                missing_suffix.push(name.to_os_string());
                let Some(parent) = probe.parent() else {
                    return Err(AsterError::storage_driver_error(format!(
                        "local path has no parent: {}",
                        path.display()
                    )));
                };
                probe = parent.to_path_buf();
            }
            Err(error) => {
                return Err(AsterError::storage_driver_error(format!(
                    "inspect local path {}: {error}",
                    probe.display()
                )));
            }
        }
    }

    let mut resolved = std::fs::canonicalize(&probe)
        .map_aster_err_ctx("canonicalize local path", AsterError::storage_driver_error)?;
    for segment in missing_suffix.into_iter().rev() {
        resolved.push(segment);
    }
    Ok(resolved)
}

fn resolve_path_within_root(root: &Path, relative: &Path, requested_path: &str) -> Result<PathBuf> {
    let candidate = root.join(relative);
    let resolved = resolve_existing_path(&candidate)?;
    if resolved.starts_with(root) {
        Ok(resolved)
    } else {
        Err(AsterError::storage_driver_error(format!(
            "resolved storage path escapes base path: {requested_path}"
        )))
    }
}

pub fn resolved_base_path(policy: &storage_policy::Model) -> Result<PathBuf> {
    resolve_existing_path(&effective_base_path(policy))
}

/// 校验 driver 输入路径，拒绝绝对路径、Windows 盘符前缀以及任何 `..` 段，
/// 防止攻击者通过污染 storage_path 逃出 base_path。
fn sanitize_relative_path(path: &str) -> Result<PathBuf> {
    let trimmed = path.trim_start_matches('/');
    let candidate = Path::new(trimmed);
    let mut safe = PathBuf::new();
    for component in candidate.components() {
        match component {
            Component::Normal(segment) => safe.push(segment),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(AsterError::storage_driver_error(format!(
                    "invalid storage path: {path}"
                )));
            }
        }
    }
    Ok(safe)
}

pub fn upload_staging_path(policy: &storage_policy::Model, name: &str) -> Result<PathBuf> {
    let root = resolved_base_path(policy)?;
    let safe = sanitize_relative_path(name).unwrap_or_else(|_| PathBuf::from("_invalid"));
    resolve_path_within_root(&root, &Path::new(".staging").join(safe), name)
}

impl LocalDriver {
    pub fn new(policy: &storage_policy::Model) -> Result<Self> {
        Ok(Self {
            base_path: resolved_base_path(policy)?,
        })
    }

    fn full_path(&self, path: &str) -> Result<PathBuf> {
        resolve_path_within_root(&self.base_path, &sanitize_relative_path(path)?, path)
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
        let full = self.full_path(path)?;
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
        tokio::fs::read(self.full_path(path)?)
            .await
            .map_aster_err(AsterError::storage_driver_error)
    }

    async fn get_stream(&self, path: &str) -> Result<Box<dyn AsyncRead + Unpin + Send>> {
        let file = tokio::fs::File::open(self.full_path(path)?)
            .await
            .map_aster_err(AsterError::storage_driver_error)?;
        Ok(Box::new(file))
    }

    async fn get_range(
        &self,
        path: &str,
        offset: u64,
        length: Option<u64>,
    ) -> Result<Box<dyn AsyncRead + Unpin + Send>> {
        use tokio::io::AsyncReadExt;
        let mut file = tokio::fs::File::open(self.full_path(path)?)
            .await
            .map_aster_err(AsterError::storage_driver_error)?;
        if offset > 0 {
            file.seek(std::io::SeekFrom::Start(offset))
                .await
                .map_aster_err_ctx("local seek", AsterError::storage_driver_error)?;
        }
        Ok(match length {
            Some(len) => Box::new(file.take(len)),
            None => Box::new(file),
        })
    }

    async fn delete(&self, path: &str) -> Result<()> {
        tokio::fs::remove_file(self.full_path(path)?)
            .await
            .map_aster_err(AsterError::storage_driver_error)
    }

    async fn exists(&self, path: &str) -> Result<bool> {
        Ok(self.full_path(path)?.exists())
    }

    async fn metadata(&self, path: &str) -> Result<BlobMetadata> {
        let meta = tokio::fs::metadata(self.full_path(path)?)
            .await
            .map_aster_err(AsterError::storage_driver_error)?;
        Ok(BlobMetadata {
            size: meta.len(),
            content_type: None,
        })
    }

    async fn copy_object(&self, src_path: &str, dest_path: &str) -> Result<String> {
        let src_full = self.full_path(src_path)?;
        let dest_full = self.full_path(dest_path)?;
        if let Some(parent) = dest_full.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_aster_err(AsterError::storage_driver_error)?;
        }
        tokio::fs::copy(&src_full, &dest_full)
            .await
            .map_aster_err_ctx("copy_object", AsterError::storage_driver_error)?;
        Ok(dest_path.to_string())
    }

    fn as_list(&self) -> Option<&dyn ListStorageDriver> {
        Some(self)
    }

    fn as_stream_upload(&self) -> Option<&dyn StreamUploadDriver> {
        Some(self)
    }

    fn as_local_path(&self) -> Option<&dyn LocalPathStorageDriver> {
        Some(self)
    }
}

impl LocalPathStorageDriver for LocalDriver {
    fn resolve_local_path(&self, path: &str) -> Result<PathBuf> {
        self.full_path(path)
    }
}

#[async_trait]
impl ListStorageDriver for LocalDriver {
    async fn list_paths(&self, prefix: Option<&str>) -> Result<Vec<String>> {
        let root = self.base_path.clone();
        let start = match prefix {
            Some(prefix) => self.full_path(prefix)?,
            None => root.clone(),
        };

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
        let start = match prefix {
            Some(prefix) => self.full_path(prefix)?,
            None => root.clone(),
        };
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
}

#[async_trait]
impl StreamUploadDriver for LocalDriver {
    async fn put_reader(
        &self,
        storage_path: &str,
        mut reader: Box<dyn AsyncRead + Unpin + Send + Sync>,
        size: i64,
    ) -> Result<String> {
        let declared_size = numbers::i64_to_u64(size, "local put_reader declared size")?;

        // 创建临时文件
        let temp_path = std::env::temp_dir().join(format!(
            "aster_put_reader_{}_{}",
            std::process::id(),
            rand::random::<u64>()
        ));

        // 流式写入临时文件
        let mut file = tokio::fs::File::create(&temp_path)
            .await
            .map_aster_err(AsterError::storage_driver_error)?;

        let written = tokio::io::copy(&mut reader, &mut file)
            .await
            .map_aster_err_ctx("write temp file", AsterError::storage_driver_error)?;

        // 验证实际写入大小与声明大小一致
        if written != declared_size {
            let _ = tokio::fs::remove_file(&temp_path).await;
            return Err(AsterError::storage_driver_error(format!(
                "size mismatch: declared {}, actual written {}",
                size, written
            )));
        }

        // 确保数据落盘
        file.flush()
            .await
            .map_aster_err(AsterError::storage_driver_error)?;
        drop(file);

        // 使用 put_file 完成上传
        let temp_path_str = temp_path.to_str().ok_or_else(|| {
            AsterError::storage_driver_error("temp upload path is not valid UTF-8")
        })?;
        let result = self.put_file(storage_path, temp_path_str).await;

        // 清理临时文件（忽略错误）
        let _ = tokio::fs::remove_file(&temp_path).await;

        result
    }

    async fn put_file(&self, storage_path: &str, local_path: &str) -> Result<String> {
        let full = self.full_path(storage_path)?;
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
}

#[cfg(test)]
mod tests {
    use super::sanitize_relative_path;
    use std::path::{Path, PathBuf};

    fn build_policy(base: &Path) -> crate::entities::storage_policy::Model {
        crate::entities::storage_policy::Model {
            id: 1,
            name: "local".into(),
            driver_type: crate::types::DriverType::Local,
            endpoint: String::new(),
            bucket: String::new(),
            access_key: String::new(),
            secret_key: String::new(),
            base_path: base.to_string_lossy().into(),
            remote_node_id: None,
            max_file_size: 0,
            allowed_types: crate::types::StoredStoragePolicyAllowedTypes::empty(),
            options: crate::types::StoredStoragePolicyOptions::empty(),
            is_default: false,
            chunk_size: 0,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn sanitize_accepts_normal_paths() {
        assert_eq!(
            sanitize_relative_path("ab/cd/abcdef").unwrap(),
            PathBuf::from("ab/cd/abcdef")
        );
        assert_eq!(
            sanitize_relative_path("/leading/slash").unwrap(),
            PathBuf::from("leading/slash")
        );
        assert_eq!(
            sanitize_relative_path("nested/./path").unwrap(),
            PathBuf::from("nested/path")
        );
    }

    #[test]
    fn sanitize_rejects_parent_dir() {
        assert!(sanitize_relative_path("../etc/passwd").is_err());
        assert!(sanitize_relative_path("ab/../../../etc/passwd").is_err());
        assert!(sanitize_relative_path("ab/..").is_err());
    }

    #[test]
    fn sanitize_rejects_absolute_paths() {
        assert!(sanitize_relative_path("/etc/passwd").is_ok()); // stripped leading slash
        // Path that starts with non-trim '/' after components would be normalized; real absolute
        // only triggers on Windows prefixes or re-rooting. Ensure multi-slash doesn't bypass.
        assert!(sanitize_relative_path("//../etc").is_err());
    }

    #[tokio::test]
    async fn get_range_returns_partial_bytes() {
        use crate::storage::driver::StorageDriver;
        use tokio::io::AsyncReadExt;

        let base = std::env::temp_dir().join(format!(
            "aster-range-test-{}-{}",
            std::process::id(),
            rand::random::<u64>()
        ));
        tokio::fs::create_dir_all(&base).await.unwrap();

        let policy = build_policy(&base);
        let driver = super::LocalDriver::new(&policy).unwrap();
        driver.put("sample.txt", b"Hello, world!").await.unwrap();

        // offset=7, length=5 -> "world"
        let mut reader = driver.get_range("sample.txt", 7, Some(5)).await.unwrap();
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf).await.unwrap();
        assert_eq!(buf, b"world");

        // offset=7, length=None -> "world!"
        let mut reader = driver.get_range("sample.txt", 7, None).await.unwrap();
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf).await.unwrap();
        assert_eq!(buf, b"world!");

        // offset=0, length=5 -> "Hello"
        let mut reader = driver.get_range("sample.txt", 0, Some(5)).await.unwrap();
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf).await.unwrap();
        assert_eq!(buf, b"Hello");

        let _ = tokio::fs::remove_dir_all(&base).await;
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn put_rejects_symlink_escape_inside_storage_root() {
        use crate::storage::driver::StorageDriver;

        let temp_root = std::env::temp_dir().join(format!(
            "aster-local-symlink-test-{}-{}",
            std::process::id(),
            rand::random::<u64>()
        ));
        let base = temp_root.join("storage");
        let outside = temp_root.join("outside");
        std::fs::create_dir_all(&base).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        std::os::unix::fs::symlink(&outside, base.join("escape")).unwrap();

        let policy = build_policy(&base);
        let driver = super::LocalDriver::new(&policy).unwrap();
        let result = driver.put("escape/pwned.txt", b"nope").await;

        assert!(result.is_err());
        assert!(!outside.join("pwned.txt").exists());

        let _ = tokio::fs::remove_dir_all(&temp_root).await;
    }

    #[cfg(unix)]
    #[test]
    fn staging_path_rejects_symlink_escape() {
        let temp_root = std::env::temp_dir().join(format!(
            "aster-local-staging-symlink-test-{}-{}",
            std::process::id(),
            rand::random::<u64>()
        ));
        let base = temp_root.join("storage");
        let outside = temp_root.join("outside");
        std::fs::create_dir_all(&base).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        std::os::unix::fs::symlink(&outside, base.join(".staging")).unwrap();

        let policy = build_policy(&base);
        let result = super::upload_staging_path(&policy, "token.upload");

        assert!(result.is_err());

        let _ = std::fs::remove_dir_all(&temp_root);
    }
}
