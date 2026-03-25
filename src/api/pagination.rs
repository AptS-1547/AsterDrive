use serde::Deserialize;
use utoipa::IntoParams;

pub const DEFAULT_FOLDER_LIMIT: u64 = 200;
pub const DEFAULT_FILE_LIMIT: u64 = 100;
pub const MAX_PAGE_SIZE: u64 = 1000;

/// 文件列表分页参数（文件夹和文件分别分页）
#[derive(Debug, Deserialize, IntoParams)]
pub struct FolderListQuery {
    /// 文件夹最大返回数量（默认 200，最大 1000；传 0 跳过文件夹查询）
    pub folder_limit: Option<u64>,
    /// 文件夹偏移量（默认 0）
    pub folder_offset: Option<u64>,
    /// 文件最大返回数量（默认 100，最大 1000；传 0 跳过文件查询）
    pub file_limit: Option<u64>,
    /// 文件偏移量（默认 0）
    pub file_offset: Option<u64>,
}

impl FolderListQuery {
    pub fn folder_limit(&self) -> u64 {
        self.folder_limit
            .map(|v| v.min(MAX_PAGE_SIZE))
            .unwrap_or(DEFAULT_FOLDER_LIMIT)
    }

    pub fn folder_offset(&self) -> u64 {
        self.folder_offset.unwrap_or(0)
    }

    pub fn file_limit(&self) -> u64 {
        self.file_limit
            .map(|v| v.min(MAX_PAGE_SIZE))
            .unwrap_or(DEFAULT_FILE_LIMIT)
    }

    pub fn file_offset(&self) -> u64 {
        self.file_offset.unwrap_or(0)
    }
}
