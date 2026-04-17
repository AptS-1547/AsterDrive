use std::time::SystemTime;

use crate::entities::{file, file_blob, folder};
use crate::webdav::dav::{DavMetaData, FsResult};

/// 将 chrono DateTimeUtc 转换为 SystemTime
fn to_system_time(dt: chrono::DateTime<chrono::Utc>) -> SystemTime {
    let secs = dt.timestamp();
    if secs >= 0 {
        SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(secs as u64)
    } else {
        SystemTime::UNIX_EPOCH
    }
}

#[derive(Debug, Clone)]
pub struct AsterDavMeta {
    is_dir: bool,
    len: u64,
    modified: SystemTime,
    created: SystemTime,
    etag: Option<String>,
}

impl AsterDavMeta {
    pub fn root() -> Self {
        Self {
            is_dir: true,
            len: 0,
            modified: SystemTime::UNIX_EPOCH,
            created: SystemTime::UNIX_EPOCH,
            etag: None,
        }
    }

    pub fn from_folder(folder: &folder::Model) -> Self {
        Self {
            is_dir: true,
            len: 0,
            modified: to_system_time(folder.updated_at),
            created: to_system_time(folder.created_at),
            etag: Some(format!("dir-{}", folder.updated_at.timestamp())),
        }
    }

    pub fn from_file(file: &file::Model, blob: &file_blob::Model) -> Self {
        Self {
            is_dir: false,
            len: blob.size as u64,
            modified: to_system_time(file.updated_at),
            created: to_system_time(file.created_at),
            etag: Some(blob.hash.clone()),
        }
    }
}

impl DavMetaData for AsterDavMeta {
    fn len(&self) -> u64 {
        self.len
    }

    fn modified(&self) -> FsResult<SystemTime> {
        Ok(self.modified)
    }

    fn is_dir(&self) -> bool {
        self.is_dir
    }

    fn etag(&self) -> Option<String> {
        self.etag.clone()
    }

    fn created(&self) -> FsResult<SystemTime> {
        Ok(self.created)
    }
}
