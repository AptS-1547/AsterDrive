//! 用户资料服务子模块：`avatar_storage`。

use std::path::Component;
use std::path::{Path, PathBuf};

use crate::config::avatar;
use crate::entities::user_profile;
use crate::runtime::PrimaryAppState;
use crate::types::AvatarSource;

use super::shared::{AVATAR_SIZE_LG, AVATAR_SIZE_SM, stored_avatar_prefix};

pub(super) fn avatar_variant_file_path(prefix: &Path, size: u32) -> PathBuf {
    prefix.join(format!("{size}.webp"))
}

fn user_avatar_prefix(user_id: i64, version: i32) -> String {
    format!("user/{user_id}/v{version}")
}

pub(super) fn user_avatar_dir(root_dir: &Path, user_id: i64, version: i32) -> PathBuf {
    root_dir.join(user_avatar_prefix(user_id, version))
}

fn normalize_absolute_path(path: &Path) -> Option<PathBuf> {
    if !path.is_absolute() {
        return None;
    }

    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    return None;
                }
            }
            Component::Normal(part) => normalized.push(part),
        }
    }

    Some(normalized)
}

async fn cleanup_empty_avatar_dirs(prefix_dir: &Path, root_dir: &Path) {
    let Some(mut current) = normalize_absolute_path(prefix_dir) else {
        tracing::warn!(
            "skip avatar dir cleanup for non-absolute prefix {}",
            prefix_dir.display()
        );
        return;
    };
    let Some(root_dir) = normalize_absolute_path(root_dir) else {
        tracing::warn!(
            "skip avatar dir cleanup for non-absolute root {}",
            root_dir.display()
        );
        return;
    };

    if current == root_dir || !current.starts_with(&root_dir) {
        tracing::warn!(
            "skip avatar dir cleanup outside avatar root: prefix={}, root={}",
            current.display(),
            root_dir.display()
        );
        return;
    }

    while current != root_dir {
        match tokio::fs::remove_dir(&current).await {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) if e.kind() == std::io::ErrorKind::DirectoryNotEmpty => break,
            Err(e) => {
                tracing::warn!("failed to cleanup avatar dir {}: {e}", current.display());
                break;
            }
        }

        let Some(parent) = current.parent() else {
            break;
        };
        current = parent.to_path_buf();
    }
}

async fn delete_local_avatar_files(prefix: &Path) {
    for size in [AVATAR_SIZE_SM, AVATAR_SIZE_LG] {
        let path = avatar_variant_file_path(prefix, size);
        if let Err(e) = tokio::fs::remove_file(&path).await
            && e.kind() != std::io::ErrorKind::NotFound
        {
            tracing::warn!("failed to delete avatar file {}: {e}", path.display());
        }
    }
}

pub(super) async fn cleanup_local_avatar_prefix(prefix: &Path, root_dir: &Path) {
    delete_local_avatar_files(prefix).await;
    cleanup_empty_avatar_dirs(prefix, root_dir).await;
}

pub(super) async fn delete_upload_objects(state: &PrimaryAppState, profile: &user_profile::Model) {
    if profile.avatar_source != AvatarSource::Upload {
        return;
    }

    let Some(prefix) = stored_avatar_prefix(Some(profile)) else {
        return;
    };

    let prefix = Path::new(prefix);
    delete_local_avatar_files(prefix).await;

    match avatar::resolve_local_avatar_root_dir(&state.runtime_config) {
        Ok(root_dir) => cleanup_empty_avatar_dirs(prefix, &root_dir).await,
        Err(e) => {
            tracing::warn!(
                "failed to resolve avatar root for local avatar cleanup {}: {e}",
                prefix.display()
            );
        }
    }
}
