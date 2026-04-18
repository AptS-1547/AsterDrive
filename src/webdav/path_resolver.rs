//! WebDAV 子模块：`path_resolver`。

use sea_orm::DatabaseConnection;

use crate::db::repository::{file_repo, folder_repo};
use crate::entities::{file, folder};
use crate::webdav::dav::{DavPath, FsError};

/// 路径解析结果
#[derive(Debug, Clone)]
pub enum ResolvedNode {
    /// 根目录 (parent_id = None)
    Root,
    /// 数据库中的文件夹
    Folder(folder::Model),
    /// 数据库中的文件
    File(file::Model),
}

/// 从 DavPath 提取路径段（已解码）
fn path_segments(path: &DavPath) -> Vec<String> {
    // as_bytes() 返回不含前缀、已解码的原始字节
    let raw = path.as_bytes();
    let path_str = String::from_utf8_lossy(raw);
    path_str
        .split('/')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

/// 解析 WebDAV 路径到数据库实体
///
/// 路径中的 folder 前缀通过单次递归查询解析，只有最后一个 file 候选需要额外查库。
pub async fn resolve_path(
    db: &DatabaseConnection,
    user_id: i64,
    path: &DavPath,
    root_folder_id: Option<i64>,
) -> Result<ResolvedNode, FsError> {
    let segments = path_segments(path);

    if segments.is_empty() {
        return Ok(ResolvedNode::Root);
    }

    let folders = folder_repo::resolve_path_chain(db, user_id, root_folder_id, &segments)
        .await
        .map_err(|_| FsError::GeneralFailure)?;

    if folders.len() == segments.len() {
        return Ok(ResolvedNode::Folder(
            folders
                .last()
                .cloned()
                .expect("non-empty path must have a last segment"),
        ));
    }

    // Only the last segment may be a file; anything earlier must have resolved as a folder chain.
    if folders.len() + 1 < segments.len() {
        return Err(FsError::NotFound);
    }

    let current_parent = folders.last().map(|folder| folder.id).or(root_folder_id);
    let last = segments
        .last()
        .expect("non-empty path must have a last segment");

    if let Some(file) = file_repo::find_by_name_in_folder(db, user_id, current_parent, last)
        .await
        .map_err(|_| FsError::GeneralFailure)?
    {
        return Ok(ResolvedNode::File(file));
    }

    Err(FsError::NotFound)
}

/// 解析路径的父目录，返回 (parent_folder_id, 末段文件名)
///
/// `/Documents/file.txt` → (Some(docs_id), "file.txt")
/// `/file.txt` → (None, "file.txt")
pub async fn resolve_parent(
    db: &DatabaseConnection,
    user_id: i64,
    path: &DavPath,
    root_folder_id: Option<i64>,
) -> Result<(Option<i64>, String), FsError> {
    let segments = path_segments(path);

    if segments.is_empty() {
        return Err(FsError::Forbidden); // 不能操作根目录本身
    }

    let parent_segments = &segments[..segments.len() - 1];
    let folders = folder_repo::resolve_path_chain(db, user_id, root_folder_id, parent_segments)
        .await
        .map_err(|_| FsError::GeneralFailure)?;

    if folders.len() != parent_segments.len() {
        return Err(FsError::NotFound);
    }

    let current_parent = folders.last().map(|folder| folder.id).or(root_folder_id);
    let last = segments[segments.len() - 1].clone();
    Ok((current_parent, last))
}
