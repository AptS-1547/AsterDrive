use std::sync::Arc;

use std::pin::Pin;

use dav_server::davpath::DavPath;
use dav_server::fs::{
    DavDirEntry, DavFile, DavFileSystem, DavMetaData, DavProp, FsError, FsFuture, FsStream,
    OpenOptions, ReadDirMeta,
};
use futures::stream;
use sea_orm::DatabaseConnection;
use tokio::io::AsyncWriteExt;

use crate::cache::CacheBackend;
use crate::config::Config;
use crate::db::repository::{file_repo, folder_repo, policy_repo, property_repo, user_repo};
use crate::services::{file_service, folder_service, webdav_service};
use crate::storage::DriverRegistry;
use crate::webdav::dir_entry::AsterDavDirEntry;
use crate::webdav::file::AsterDavFile;
use crate::webdav::metadata::AsterDavMeta;
use crate::webdav::path_resolver::{self, ResolvedNode};

/// AsterDrive WebDAV 文件系统，per-user 实例
#[derive(Clone)]
pub struct AsterDavFs {
    db: DatabaseConnection,
    driver_registry: Arc<DriverRegistry>,
    config: Arc<Config>,
    cache: Arc<dyn CacheBackend>,
    user_id: i64,
    /// 限制访问范围：None = 用户全部文件，Some(id) = 只能访问该文件夹及子目录
    root_folder_id: Option<i64>,
}

impl std::fmt::Debug for AsterDavFs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AsterDavFs")
            .field("user_id", &self.user_id)
            .field("root_folder_id", &self.root_folder_id)
            .finish()
    }
}

impl AsterDavFs {
    pub fn new(
        db: DatabaseConnection,
        driver_registry: Arc<DriverRegistry>,
        config: Arc<Config>,
        cache: Arc<dyn CacheBackend>,
        user_id: i64,
        root_folder_id: Option<i64>,
    ) -> Self {
        Self {
            db,
            driver_registry,
            config,
            cache,
            user_id,
            root_folder_id,
        }
    }

    fn app_state(&self) -> crate::runtime::AppState {
        crate::runtime::AppState {
            db: self.db.clone(),
            driver_registry: self.driver_registry.clone(),
            config: self.config.clone(),
            cache: self.cache.clone(),
        }
    }
}

impl DavFileSystem for AsterDavFs {
    fn open<'a>(
        &'a self,
        path: &'a DavPath,
        options: OpenOptions,
    ) -> FsFuture<'a, Box<dyn DavFile>> {
        Box::pin(async move {
            if options.write {
                // 写模式
                let (parent_id, filename) = path_resolver::resolve_parent(
                    &self.db,
                    self.user_id,
                    path,
                    self.root_folder_id,
                )
                .await?;

                let existing_file_id =
                    file_repo::find_by_name_in_folder(&self.db, self.user_id, parent_id, &filename)
                        .await
                        .map_err(|_| FsError::GeneralFailure)?
                        .map(|f| f.id);

                if options.create_new && existing_file_id.is_some() {
                    return Err(FsError::Exists);
                }

                let dav_file = AsterDavFile::for_write(
                    self.db.clone(),
                    self.driver_registry.clone(),
                    self.cache.clone(),
                    self.user_id,
                    parent_id,
                    filename,
                    existing_file_id,
                )
                .await?;

                Ok(Box::new(dav_file) as Box<dyn DavFile>)
            } else {
                // 读模式：从存储复制到临时文件，避免全量加载到内存
                let node =
                    path_resolver::resolve_path(&self.db, self.user_id, path, self.root_folder_id)
                        .await?;

                match node {
                    ResolvedNode::File(f) => {
                        let blob = file_repo::find_blob_by_id(&self.db, f.blob_id)
                            .await
                            .map_err(|_| FsError::GeneralFailure)?;
                        let policy = policy_repo::find_by_id(&self.db, blob.policy_id)
                            .await
                            .map_err(|_| FsError::GeneralFailure)?;
                        let driver = self
                            .driver_registry
                            .get_driver(&policy)
                            .map_err(|_| FsError::GeneralFailure)?;
                        let meta = AsterDavMeta::from_file(&f, &blob);

                        // 流式复制到临时文件
                        let temp_path = format!("data/.tmp/{}", uuid::Uuid::new_v4());
                        tokio::fs::create_dir_all("data/.tmp")
                            .await
                            .map_err(|_| FsError::GeneralFailure)?;

                        let mut stream = driver
                            .get_stream(&blob.storage_path)
                            .await
                            .map_err(|_| FsError::NotFound)?;
                        let mut temp_file = tokio::fs::File::create(&temp_path)
                            .await
                            .map_err(|_| FsError::GeneralFailure)?;
                        tokio::io::copy(&mut stream, &mut temp_file)
                            .await
                            .map_err(|_| FsError::GeneralFailure)?;
                        temp_file
                            .flush()
                            .await
                            .map_err(|_| FsError::GeneralFailure)?;

                        // 重新打开用于读取（seek 到开头）
                        let read_file = tokio::fs::File::open(&temp_path)
                            .await
                            .map_err(|_| FsError::GeneralFailure)?;

                        Ok(Box::new(AsterDavFile::for_read(
                            read_file,
                            temp_path,
                            blob.size as u64,
                            meta,
                        )) as Box<dyn DavFile>)
                    }
                    _ => Err(FsError::Forbidden),
                }
            }
        })
    }

    fn read_dir<'a>(
        &'a self,
        path: &'a DavPath,
        _meta: ReadDirMeta,
    ) -> FsFuture<'a, FsStream<Box<dyn DavDirEntry>>> {
        Box::pin(async move {
            let folder_id = match path_resolver::resolve_path(
                &self.db,
                self.user_id,
                path,
                self.root_folder_id,
            )
            .await?
            {
                ResolvedNode::Root => self.root_folder_id,
                ResolvedNode::Folder(f) => Some(f.id),
                ResolvedNode::File(_) => return Err(FsError::Forbidden),
            };

            let folders = folder_repo::find_children(&self.db, self.user_id, folder_id)
                .await
                .map_err(|_| FsError::GeneralFailure)?;
            let files = file_repo::find_by_folder(&self.db, self.user_id, folder_id)
                .await
                .map_err(|_| FsError::GeneralFailure)?;

            let mut entries: Vec<Box<dyn DavDirEntry>> = Vec::new();

            for folder in &folders {
                if is_hidden_name(&folder.name) {
                    continue;
                }
                entries.push(Box::new(AsterDavDirEntry::from_folder(folder)));
            }

            // 批量查询所有 blob（1 次查询替代 N 次）
            let visible_files: Vec<_> = files.iter().filter(|f| !is_hidden_name(&f.name)).collect();
            let blob_ids: Vec<i64> = visible_files.iter().map(|f| f.blob_id).collect();
            let blobs = file_repo::find_blobs_by_ids(&self.db, &blob_ids)
                .await
                .map_err(|_| FsError::GeneralFailure)?;

            for file in &visible_files {
                if let Some(blob) = blobs.get(&file.blob_id) {
                    entries.push(Box::new(AsterDavDirEntry::from_file(file, blob)));
                }
            }

            Ok(Box::pin(stream::iter(entries.into_iter().map(Ok)))
                as FsStream<Box<dyn DavDirEntry>>)
        })
    }

    fn metadata<'a>(&'a self, path: &'a DavPath) -> FsFuture<'a, Box<dyn DavMetaData>> {
        Box::pin(async move {
            let node =
                path_resolver::resolve_path(&self.db, self.user_id, path, self.root_folder_id)
                    .await?;

            let meta: Box<dyn DavMetaData> = match node {
                ResolvedNode::Root => Box::new(AsterDavMeta::root()),
                ResolvedNode::Folder(f) => Box::new(AsterDavMeta::from_folder(&f)),
                ResolvedNode::File(f) => {
                    let blob = file_repo::find_blob_by_id(&self.db, f.blob_id)
                        .await
                        .map_err(|_| FsError::GeneralFailure)?;
                    Box::new(AsterDavMeta::from_file(&f, &blob))
                }
            };

            Ok(meta)
        })
    }

    fn create_dir<'a>(&'a self, path: &'a DavPath) -> FsFuture<'a, ()> {
        Box::pin(async move {
            let (parent_id, name) =
                path_resolver::resolve_parent(&self.db, self.user_id, path, self.root_folder_id)
                    .await?;

            let state = self.app_state();
            folder_service::create(&state, self.user_id, &name, parent_id)
                .await
                .map_err(to_fs_error)?;

            Ok(())
        })
    }

    fn remove_dir<'a>(&'a self, path: &'a DavPath) -> FsFuture<'a, ()> {
        Box::pin(async move {
            let node =
                path_resolver::resolve_path(&self.db, self.user_id, path, self.root_folder_id)
                    .await?;
            let folder = match node {
                ResolvedNode::Folder(f) => f,
                _ => return Err(FsError::Forbidden),
            };

            let state = self.app_state();
            webdav_service::recursive_soft_delete(&state, self.user_id, folder.id)
                .await
                .map_err(to_fs_error)?;

            Ok(())
        })
    }

    fn remove_file<'a>(&'a self, path: &'a DavPath) -> FsFuture<'a, ()> {
        Box::pin(async move {
            let node =
                path_resolver::resolve_path(&self.db, self.user_id, path, self.root_folder_id)
                    .await?;
            let file = match node {
                ResolvedNode::File(f) => f,
                _ => return Err(FsError::Forbidden),
            };

            let state = self.app_state();
            file_service::delete(&state, file.id, self.user_id)
                .await
                .map_err(to_fs_error)?;

            Ok(())
        })
    }

    fn rename<'a>(&'a self, from: &'a DavPath, to: &'a DavPath) -> FsFuture<'a, ()> {
        Box::pin(async move {
            let node =
                path_resolver::resolve_path(&self.db, self.user_id, from, self.root_folder_id)
                    .await?;
            let (dest_parent_id, dest_name) =
                path_resolver::resolve_parent(&self.db, self.user_id, to, self.root_folder_id)
                    .await?;

            let state = self.app_state();

            match node {
                ResolvedNode::File(f) => {
                    // 如果目标已有同名文件，先删除（WebDAV MOVE 覆盖语义）
                    if let Some(existing) = file_repo::find_by_name_in_folder(
                        &self.db,
                        self.user_id,
                        dest_parent_id,
                        &dest_name,
                    )
                    .await
                    .map_err(|_| FsError::GeneralFailure)?
                    {
                        file_service::delete(&state, existing.id, self.user_id)
                            .await
                            .map_err(to_fs_error)?;
                    }

                    file_service::update(
                        &state,
                        f.id,
                        self.user_id,
                        Some(dest_name),
                        dest_parent_id,
                    )
                    .await
                    .map_err(to_fs_error)?;
                }
                ResolvedNode::Folder(f) => {
                    folder_service::update(
                        &state,
                        f.id,
                        self.user_id,
                        Some(dest_name),
                        dest_parent_id,
                        None,
                    )
                    .await
                    .map_err(to_fs_error)?;
                }
                ResolvedNode::Root => return Err(FsError::Forbidden),
            }

            Ok(())
        })
    }

    fn copy<'a>(&'a self, from: &'a DavPath, to: &'a DavPath) -> FsFuture<'a, ()> {
        Box::pin(async move {
            let node =
                path_resolver::resolve_path(&self.db, self.user_id, from, self.root_folder_id)
                    .await?;
            let (dest_parent_id, dest_name) =
                path_resolver::resolve_parent(&self.db, self.user_id, to, self.root_folder_id)
                    .await?;

            let state = self.app_state();

            match node {
                ResolvedNode::File(f) => {
                    // 复制文件：增加 blob ref_count，创建新文件记录
                    let blob = file_repo::find_blob_by_id(&self.db, f.blob_id)
                        .await
                        .map_err(|_| FsError::GeneralFailure)?;

                    let now = chrono::Utc::now();

                    // 增加引用计数
                    let mut blob_active: crate::entities::file_blob::ActiveModel =
                        blob.clone().into();
                    blob_active.ref_count = sea_orm::Set(blob.ref_count + 1);
                    blob_active.updated_at = sea_orm::Set(now);
                    use sea_orm::ActiveModelTrait;
                    blob_active
                        .update(&self.db)
                        .await
                        .map_err(|_| FsError::GeneralFailure)?;

                    // 如果目标已存在，先删除
                    if let Some(existing) = file_repo::find_by_name_in_folder(
                        &self.db,
                        self.user_id,
                        dest_parent_id,
                        &dest_name,
                    )
                    .await
                    .map_err(|_| FsError::GeneralFailure)?
                    {
                        file_service::delete(&state, existing.id, self.user_id)
                            .await
                            .map_err(to_fs_error)?;
                    }

                    // 创建新文件记录
                    file_repo::create(
                        &self.db,
                        crate::entities::file::ActiveModel {
                            name: sea_orm::Set(dest_name),
                            folder_id: sea_orm::Set(dest_parent_id),
                            blob_id: sea_orm::Set(f.blob_id),
                            user_id: sea_orm::Set(self.user_id),
                            mime_type: sea_orm::Set(f.mime_type),
                            created_at: sea_orm::Set(now),
                            updated_at: sea_orm::Set(now),
                            ..Default::default()
                        },
                    )
                    .await
                    .map_err(|_| FsError::GeneralFailure)?;

                    // 更新用户空间
                    user_repo::update_storage_used(&self.db, self.user_id, blob.size)
                        .await
                        .map_err(|_| FsError::GeneralFailure)?;
                }
                ResolvedNode::Folder(f) => {
                    webdav_service::recursive_copy_folder(
                        &state,
                        self.user_id,
                        f.id,
                        dest_parent_id,
                        &dest_name,
                    )
                    .await
                    .map_err(to_fs_error)?;
                }
                ResolvedNode::Root => return Err(FsError::Forbidden),
            }

            Ok(())
        })
    }

    fn get_quota(&self) -> FsFuture<'_, (u64, Option<u64>)> {
        Box::pin(async move {
            let user = user_repo::find_by_id(&self.db, self.user_id)
                .await
                .map_err(|_| FsError::GeneralFailure)?;

            let used = user.storage_used as u64;
            let total = if user.storage_quota > 0 {
                Some(user.storage_quota as u64)
            } else {
                None // 无限
            };

            Ok((used, total))
        })
    }

    fn have_props<'a>(
        &'a self,
        path: &'a DavPath,
    ) -> Pin<Box<dyn std::future::Future<Output = bool> + Send + 'a>> {
        Box::pin(async move {
            let (entity_type, entity_id) =
                match resolve_entity(&self.db, self.user_id, path, self.root_folder_id).await {
                    Some(v) => v,
                    None => return false,
                };
            property_repo::has_properties(&self.db, &entity_type, entity_id)
                .await
                .unwrap_or(false)
        })
    }

    fn get_props<'a>(&'a self, path: &'a DavPath, do_content: bool) -> FsFuture<'a, Vec<DavProp>> {
        Box::pin(async move {
            let (entity_type, entity_id) =
                resolve_entity(&self.db, self.user_id, path, self.root_folder_id)
                    .await
                    .ok_or(FsError::NotFound)?;

            let props = property_repo::find_by_entity(&self.db, &entity_type, entity_id)
                .await
                .map_err(|_| FsError::GeneralFailure)?;

            Ok(props
                .into_iter()
                .map(|p| DavProp {
                    name: p.name,
                    prefix: None,
                    namespace: if p.namespace.is_empty() {
                        None
                    } else {
                        Some(p.namespace)
                    },
                    xml: if do_content {
                        p.value.map(|v| v.into_bytes())
                    } else {
                        None
                    },
                })
                .collect())
        })
    }

    fn patch_props<'a>(
        &'a self,
        path: &'a DavPath,
        patches: Vec<(bool, DavProp)>,
    ) -> FsFuture<'a, Vec<(http::StatusCode, DavProp)>> {
        Box::pin(async move {
            let (entity_type, entity_id) =
                resolve_entity(&self.db, self.user_id, path, self.root_folder_id)
                    .await
                    .ok_or(FsError::NotFound)?;

            let mut results = Vec::new();

            for (set, prop) in patches {
                let ns = prop.namespace.as_deref().unwrap_or("");

                // DAV: 命名空间只读
                if ns == "DAV:" {
                    results.push((http::StatusCode::FORBIDDEN, prop));
                    continue;
                }

                let status = if set {
                    let value = prop.xml.as_ref().map(|x| String::from_utf8_lossy(x));
                    match property_repo::upsert(
                        &self.db,
                        &entity_type,
                        entity_id,
                        ns,
                        &prop.name,
                        value.as_deref(),
                    )
                    .await
                    {
                        Ok(_) => http::StatusCode::OK,
                        Err(_) => http::StatusCode::INTERNAL_SERVER_ERROR,
                    }
                } else {
                    match property_repo::delete_prop(
                        &self.db,
                        &entity_type,
                        entity_id,
                        ns,
                        &prop.name,
                    )
                    .await
                    {
                        Ok(_) => http::StatusCode::OK,
                        Err(_) => http::StatusCode::INTERNAL_SERVER_ERROR,
                    }
                };

                results.push((status, prop));
            }

            Ok(results)
        })
    }
}

/// 从 DavPath 解析出 (entity_type, entity_id)
async fn resolve_entity(
    db: &DatabaseConnection,
    user_id: i64,
    path: &DavPath,
    root_folder_id: Option<i64>,
) -> Option<(String, i64)> {
    match path_resolver::resolve_path(db, user_id, path, root_folder_id).await {
        Ok(ResolvedNode::File(f)) => Some(("file".to_string(), f.id)),
        Ok(ResolvedNode::Folder(f)) => Some(("folder".to_string(), f.id)),
        _ => None,
    }
}

use crate::utils::is_hidden_name;

/// AsterError → FsError 映射
fn to_fs_error(err: crate::errors::AsterError) -> FsError {
    match &err {
        crate::errors::AsterError::FileNotFound(_)
        | crate::errors::AsterError::FolderNotFound(_)
        | crate::errors::AsterError::RecordNotFound(_) => FsError::NotFound,

        crate::errors::AsterError::AuthForbidden(_) => FsError::Forbidden,

        crate::errors::AsterError::StorageQuotaExceeded(_) => FsError::InsufficientStorage,

        crate::errors::AsterError::FileTooLarge(_) => FsError::TooLarge,

        crate::errors::AsterError::ValidationError(msg) if msg.contains("already exists") => {
            FsError::Exists
        }

        _ => FsError::GeneralFailure,
    }
}
