use std::io::SeekFrom;

use bytes::Bytes;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

use crate::runtime::AppState;
use crate::webdav::dav::{DavFile, DavMetaData, FsError, FsFuture};
use crate::webdav::metadata::AsterDavMeta;

/// DavFile 实现，使用临时文件避免大文件内存爆炸
pub struct AsterDavFile {
    mode: FileMode,
}

impl std::fmt::Debug for AsterDavFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.mode {
            FileMode::Read { temp_path, .. } => f
                .debug_struct("AsterDavFile::Read")
                .field("temp_path", temp_path)
                .finish(),
            FileMode::Write {
                filename,
                temp_path,
                ..
            } => f
                .debug_struct("AsterDavFile::Write")
                .field("filename", filename)
                .field("temp_path", temp_path)
                .finish(),
        }
    }
}

enum FileMode {
    Read {
        file: tokio::fs::File,
        temp_path: String,
        meta: AsterDavMeta,
    },
    Write {
        state: AppState,
        user_id: i64,
        folder_id: Option<i64>,
        filename: String,
        existing_file_id: Option<i64>,
        file: tokio::fs::File,
        temp_path: String,
        written: u64,
        meta: AsterDavMeta,
    },
}

impl AsterDavFile {
    /// 创建读模式文件（持有临时文件句柄）
    pub fn for_read(file: tokio::fs::File, temp_path: String, meta: AsterDavMeta) -> Self {
        Self {
            mode: FileMode::Read {
                file,
                temp_path,
                meta,
            },
        }
    }

    /// 创建写模式文件（持有临时文件句柄）
    pub async fn for_write(
        state: AppState,
        user_id: i64,
        folder_id: Option<i64>,
        filename: String,
        existing_file_id: Option<i64>,
    ) -> Result<Self, FsError> {
        let temp_dir = state.config.server.temp_dir.clone();
        let runtime_temp_dir = crate::utils::paths::runtime_temp_dir(&temp_dir);
        let temp_path = crate::utils::paths::runtime_temp_file_path(
            &temp_dir,
            &uuid::Uuid::new_v4().to_string(),
        );
        tokio::fs::create_dir_all(&runtime_temp_dir)
            .await
            .map_err(|_| FsError::GeneralFailure)?;
        let file = tokio::fs::File::create(&temp_path)
            .await
            .map_err(|_| FsError::GeneralFailure)?;

        Ok(Self {
            mode: FileMode::Write {
                state,
                user_id,
                folder_id,
                filename,
                existing_file_id,
                file,
                temp_path,
                written: 0,
                meta: AsterDavMeta::root(),
            },
        })
    }

    /// 清理临时文件（best-effort，异步后台执行）
    fn cleanup_temp(temp_path: &str) {
        let path = temp_path.to_string();
        tokio::spawn(async move {
            crate::utils::cleanup_temp_file(&path).await;
        });
    }
}

impl Drop for AsterDavFile {
    fn drop(&mut self) {
        let temp_path = match &self.mode {
            FileMode::Read { temp_path, .. } => temp_path.clone(),
            FileMode::Write { temp_path, .. } => temp_path.clone(),
        };
        Self::cleanup_temp(&temp_path);
    }
}

impl DavFile for AsterDavFile {
    fn metadata<'a>(&'a mut self) -> FsFuture<'a, Box<dyn DavMetaData>> {
        let meta: Box<dyn DavMetaData> = match &self.mode {
            FileMode::Read { meta, .. } => Box::new(meta.clone()),
            FileMode::Write { meta, .. } => Box::new(meta.clone()),
        };
        Box::pin(async move { Ok(meta) })
    }

    fn read_bytes(&mut self, count: usize) -> FsFuture<'_, Bytes> {
        Box::pin(async move {
            match &mut self.mode {
                FileMode::Read { file, .. } => {
                    let mut buf = vec![0u8; count];
                    let n = file
                        .read(&mut buf)
                        .await
                        .map_err(|_| FsError::GeneralFailure)?;
                    if n == 0 {
                        return Ok(Bytes::new());
                    }
                    buf.truncate(n);
                    Ok(Bytes::from(buf))
                }
                FileMode::Write { .. } => Err(FsError::Forbidden),
            }
        })
    }

    fn write_bytes(&mut self, buf: Bytes) -> FsFuture<'_, ()> {
        Box::pin(async move {
            match &mut self.mode {
                FileMode::Write { file, written, .. } => {
                    file.write_all(&buf)
                        .await
                        .map_err(|_| FsError::GeneralFailure)?;
                    *written += buf.len() as u64;
                    Ok(())
                }
                FileMode::Read { .. } => Err(FsError::Forbidden),
            }
        })
    }

    fn write_buf(&mut self, mut buf: Box<dyn bytes::Buf + Send>) -> FsFuture<'_, ()> {
        Box::pin(async move {
            match &mut self.mode {
                FileMode::Write { file, written, .. } => {
                    while buf.has_remaining() {
                        let chunk = buf.chunk();
                        file.write_all(chunk)
                            .await
                            .map_err(|_| FsError::GeneralFailure)?;
                        *written += chunk.len() as u64;
                        let len = chunk.len();
                        buf.advance(len);
                    }
                    Ok(())
                }
                FileMode::Read { .. } => Err(FsError::Forbidden),
            }
        })
    }

    fn seek(&mut self, pos: SeekFrom) -> FsFuture<'_, u64> {
        Box::pin(async move {
            match &mut self.mode {
                FileMode::Read { file, .. } => {
                    file.seek(pos).await.map_err(|_| FsError::GeneralFailure)
                }
                FileMode::Write { file, .. } => {
                    file.seek(pos).await.map_err(|_| FsError::GeneralFailure)
                }
            }
        })
    }

    fn flush(&mut self) -> FsFuture<'_, ()> {
        Box::pin(async move {
            let FileMode::Write {
                state,
                user_id,
                folder_id,
                filename,
                existing_file_id,
                file,
                temp_path,
                written,
                ..
            } = &mut self.mode
            else {
                return Ok(());
            };

            file.flush().await.map_err(|_| FsError::GeneralFailure)?;

            if *written == 0 {
                return Ok(());
            }

            // 调用公共函数，不重复 hash/dedup/quota 逻辑
            crate::services::file_service::store_from_temp(
                state,
                *user_id,
                *folder_id,
                filename,
                temp_path,
                *written as i64,
                *existing_file_id,
                true, // WebDAV: skip lock check, handler 已验证 lock token
            )
            .await
            .map_err(|e| {
                tracing::warn!("WebDAV store_from_temp failed: {e}");
                match &e {
                    crate::errors::AsterError::FileTooLarge(_) => FsError::TooLarge,
                    crate::errors::AsterError::StorageQuotaExceeded(_) => {
                        FsError::InsufficientStorage
                    }
                    crate::errors::AsterError::ValidationError(msg)
                        if msg.contains("already exists") =>
                    {
                        FsError::Exists
                    }
                    _ => FsError::GeneralFailure,
                }
            })?;

            Ok(())
        })
    }
}
