use std::collections::HashMap;
use std::io::Cursor;
use std::path::Component;
use std::path::{Path, PathBuf};

use actix_multipart::Multipart;
use actix_web::HttpResponse;
use chrono::Utc;
use futures::StreamExt;
use image::ImageFormat;
use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView, ImageReader, Limits};
use md5::{Digest, Md5};
use sea_orm::Set;
use serde::Serialize;
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::ToSchema;

use crate::api::constants::YEAR_SECS;
use crate::config::avatar;
use crate::db::repository::{user_profile_repo, user_repo};
use crate::entities::{user, user_profile};
use crate::errors::{AsterError, MapAsterErr, Result};
use crate::runtime::AppState;
use crate::types::AvatarSource;

const MAX_AVATAR_UPLOAD_SIZE: usize = 10 * 1024 * 1024;
const MAX_AVATAR_DECODE_ALLOC: u64 = 128 * 1024 * 1024;
const AVATAR_SIZE_SM: u32 = 512;
const AVATAR_SIZE_LG: u32 = 1024;

#[derive(Debug, Clone, Copy)]
pub enum AvatarAudience {
    SelfUser,
    AdminUser,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct AvatarInfo {
    pub source: AvatarSource,
    pub url_512: Option<String>,
    pub url_1024: Option<String>,
    pub version: i32,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct UserProfileInfo {
    pub display_name: Option<String>,
    pub avatar: AvatarInfo,
}

const DEFAULT_GRAVATAR_BASE_URL: &str = "https://www.gravatar.com/avatar";

pub fn resolve_gravatar_base_url(state: &AppState) -> String {
    let base_url = state
        .runtime_config
        .get_string_or("gravatar_base_url", DEFAULT_GRAVATAR_BASE_URL);

    if base_url.trim().is_empty() {
        DEFAULT_GRAVATAR_BASE_URL.to_string()
    } else {
        base_url
    }
}

fn gravatar_hash(email: &str) -> String {
    let normalized = email.trim().to_lowercase();
    let mut hasher = Md5::new();
    hasher.update(normalized.as_bytes());
    crate::utils::hash::bytes_to_hex(&hasher.finalize())
}

fn gravatar_url(email: &str, size: u32, base_url: &str) -> String {
    let hash = gravatar_hash(email);
    let base = base_url.trim_end_matches('/');
    format!("{base}/{hash}?d=identicon&s={size}&r=g")
}

fn avatar_api_path(user_id: i64, version: i32, size: u32, audience: AvatarAudience) -> String {
    match audience {
        AvatarAudience::SelfUser => format!("/auth/profile/avatar/{size}?v={version}"),
        AvatarAudience::AdminUser => {
            format!("/admin/users/{user_id}/avatar/{size}?v={version}")
        }
    }
}

fn share_public_avatar_api_path(share_token: &str, version: i32, size: u32) -> String {
    format!("/s/{share_token}/avatar/{size}?v={version}")
}

fn avatar_object_key(prefix: &str, size: u32) -> String {
    format!("{prefix}/{size}.webp")
}

fn avatar_variant_file_path(prefix: &Path, size: u32) -> PathBuf {
    prefix.join(format!("{size}.webp"))
}

fn user_avatar_prefix(user_id: i64, version: i32) -> String {
    format!("user/{user_id}/v{version}")
}

fn stored_avatar_prefix(profile: Option<&user_profile::Model>) -> Option<&str> {
    profile
        .and_then(|profile| profile.avatar_key.as_deref())
        .map(str::trim)
        .filter(|prefix| !prefix.is_empty())
}

#[allow(deprecated)]
fn legacy_avatar_policy_id(profile: &user_profile::Model) -> Option<i64> {
    profile.avatar_policy_id
}

fn user_avatar_dir(root_dir: &Path, user_id: i64, version: i32) -> PathBuf {
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

async fn cleanup_local_avatar_prefix(prefix: &Path, root_dir: &Path) {
    delete_local_avatar_files(prefix).await;
    cleanup_empty_avatar_dirs(prefix, root_dir).await;
}

fn normalize_display_name(value: &str) -> Result<Option<String>> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    let char_count = trimmed.chars().count();
    if char_count > 64 {
        return Err(AsterError::validation_error(
            "display name must be 64 characters or fewer",
        ));
    }

    Ok(Some(trimmed.to_string()))
}

fn encode_webp(img: &DynamicImage) -> Result<Vec<u8>> {
    let mut buf = Cursor::new(Vec::new());
    img.write_to(&mut buf, ImageFormat::WebP)
        .map_aster_err_ctx("encode webp", AsterError::file_upload_failed)?;
    Ok(buf.into_inner())
}

fn process_avatar_image(data: Vec<u8>) -> Result<(Vec<u8>, Vec<u8>)> {
    let mut reader = ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .map_aster_err_ctx("guess avatar format", AsterError::file_type_not_allowed)?;

    let mut limits = Limits::default();
    limits.max_alloc = Some(MAX_AVATAR_DECODE_ALLOC);
    reader.limits(limits);

    let img = reader
        .decode()
        .map_aster_err_ctx("decode avatar", AsterError::file_type_not_allowed)?;

    let (width, height) = img.dimensions();
    if width == 0 || height == 0 {
        return Err(AsterError::validation_error("empty image"));
    }

    let side = width.min(height);
    let left = (width - side) / 2;
    let top = (height - side) / 2;
    let square = img.crop_imm(left, top, side, side);

    let large = square.resize_exact(AVATAR_SIZE_LG, AVATAR_SIZE_LG, FilterType::Triangle);
    let small = square.resize_exact(AVATAR_SIZE_SM, AVATAR_SIZE_SM, FilterType::Triangle);

    let large_bytes = encode_webp(&large)?;
    let small_bytes = encode_webp(&small)?;
    Ok((small_bytes, large_bytes))
}

async fn read_avatar_upload(payload: &mut Multipart) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut saw_file = false;

    while let Some(field) = payload.next().await {
        let mut field = field.map_aster_err(AsterError::file_upload_failed)?;
        let has_filename = field
            .content_disposition()
            .and_then(|cd| cd.get_filename())
            .is_some();
        if !has_filename {
            while let Some(chunk) = field.next().await {
                chunk.map_aster_err(AsterError::file_upload_failed)?;
            }
            continue;
        }

        saw_file = true;
        while let Some(chunk) = field.next().await {
            let chunk = chunk.map_aster_err(AsterError::file_upload_failed)?;
            if bytes.len() + chunk.len() > MAX_AVATAR_UPLOAD_SIZE {
                return Err(AsterError::file_too_large(format!(
                    "avatar upload exceeds {} bytes",
                    MAX_AVATAR_UPLOAD_SIZE
                )));
            }
            bytes.extend_from_slice(&chunk);
        }
        break;
    }

    if !saw_file || bytes.is_empty() {
        return Err(AsterError::validation_error("avatar file is required"));
    }

    Ok(bytes)
}

async fn write_local_avatar(path: &Path, data: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_aster_err(AsterError::storage_driver_error)?;
    }

    tokio::fs::write(path, data)
        .await
        .map_aster_err(AsterError::storage_driver_error)?;
    Ok(())
}

async fn delete_upload_objects(state: &AppState, profile: &user_profile::Model) {
    if profile.avatar_source != AvatarSource::Upload {
        return;
    }

    let Some(prefix) = stored_avatar_prefix(Some(profile)) else {
        return;
    };

    if let Some(policy_id) = legacy_avatar_policy_id(profile) {
        let Some(policy) = state.policy_snapshot.get_policy(policy_id) else {
            return;
        };
        let Ok(driver) = state.driver_registry.get_driver(&policy) else {
            return;
        };

        for size in [AVATAR_SIZE_SM, AVATAR_SIZE_LG] {
            let path = avatar_object_key(prefix, size);
            if let Err(e) = driver.delete(&path).await {
                tracing::warn!("failed to delete avatar object {path}: {e}");
            }
        }
        return;
    }

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

pub async fn cleanup_avatar_upload(state: &AppState, user_id: i64) -> Result<()> {
    let profile = user_profile_repo::find_by_user_id(&state.db, user_id).await?;
    if let Some(profile) = profile.as_ref() {
        delete_upload_objects(state, profile).await;
    }
    Ok(())
}

fn build_avatar_info(
    user: &user::Model,
    profile: Option<&user_profile::Model>,
    audience: AvatarAudience,
    gravatar_base_url: &str,
) -> AvatarInfo {
    let source = profile
        .map(|p| p.avatar_source)
        .unwrap_or(AvatarSource::None);
    let version = profile.map(|p| p.avatar_version).unwrap_or(0);

    match source {
        AvatarSource::None => AvatarInfo {
            source,
            url_512: None,
            url_1024: None,
            version,
        },
        AvatarSource::Gravatar => AvatarInfo {
            source,
            url_512: Some(gravatar_url(&user.email, AVATAR_SIZE_SM, gravatar_base_url)),
            url_1024: Some(gravatar_url(&user.email, AVATAR_SIZE_LG, gravatar_base_url)),
            version,
        },
        AvatarSource::Upload => {
            let has_upload = stored_avatar_prefix(profile).is_some();

            AvatarInfo {
                source,
                url_512: has_upload
                    .then(|| avatar_api_path(user.id, version, AVATAR_SIZE_SM, audience)),
                url_1024: has_upload
                    .then(|| avatar_api_path(user.id, version, AVATAR_SIZE_LG, audience)),
                version,
            }
        }
    }
}

pub fn build_profile_info(
    user: &user::Model,
    profile: Option<&user_profile::Model>,
    audience: AvatarAudience,
    gravatar_base_url: &str,
) -> UserProfileInfo {
    UserProfileInfo {
        display_name: profile.and_then(|p| p.display_name.clone()),
        avatar: build_avatar_info(user, profile, audience, gravatar_base_url),
    }
}

pub fn build_share_public_avatar_info(
    user: &user::Model,
    profile: Option<&user_profile::Model>,
    share_token: &str,
    gravatar_base_url: &str,
) -> AvatarInfo {
    let source = profile
        .map(|p| p.avatar_source)
        .unwrap_or(AvatarSource::None);
    let version = profile.map(|p| p.avatar_version).unwrap_or(0);

    match source {
        AvatarSource::None => AvatarInfo {
            source,
            url_512: None,
            url_1024: None,
            version,
        },
        AvatarSource::Gravatar => AvatarInfo {
            source,
            url_512: Some(gravatar_url(&user.email, AVATAR_SIZE_SM, gravatar_base_url)),
            url_1024: Some(gravatar_url(&user.email, AVATAR_SIZE_LG, gravatar_base_url)),
            version,
        },
        AvatarSource::Upload => {
            let has_upload = stored_avatar_prefix(profile).is_some();

            AvatarInfo {
                source,
                url_512: has_upload
                    .then(|| share_public_avatar_api_path(share_token, version, AVATAR_SIZE_SM)),
                url_1024: has_upload
                    .then(|| share_public_avatar_api_path(share_token, version, AVATAR_SIZE_LG)),
                version,
            }
        }
    }
}

pub async fn get_profile_info(
    state: &AppState,
    user: &user::Model,
    audience: AvatarAudience,
) -> Result<UserProfileInfo> {
    let profile = user_profile_repo::find_by_user_id(&state.db, user.id).await?;
    let gravatar_base_url = resolve_gravatar_base_url(state);
    Ok(build_profile_info(
        user,
        profile.as_ref(),
        audience,
        &gravatar_base_url,
    ))
}

pub async fn get_profile_info_map(
    state: &AppState,
    users: &[user::Model],
    audience: AvatarAudience,
) -> Result<HashMap<i64, UserProfileInfo>> {
    let user_ids: Vec<i64> = users.iter().map(|user| user.id).collect();
    let profiles = user_profile_repo::find_by_user_ids(&state.db, &user_ids).await?;
    let gravatar_base_url = resolve_gravatar_base_url(state);

    Ok(users
        .iter()
        .map(|user| {
            (
                user.id,
                build_profile_info(user, profiles.get(&user.id), audience, &gravatar_base_url),
            )
        })
        .collect())
}

pub async fn upload_avatar(
    state: &AppState,
    user_id: i64,
    payload: &mut Multipart,
) -> Result<UserProfileInfo> {
    let user = user_repo::find_by_id(&state.db, user_id).await?;
    let existing = user_profile_repo::find_by_user_id(&state.db, user_id).await?;
    let upload_data = read_avatar_upload(payload).await?;
    let (small_bytes, large_bytes) = process_avatar_image(upload_data)?;
    user_repo::check_quota(
        &state.db,
        user_id,
        i64::try_from(small_bytes.len() + large_bytes.len()).unwrap_or(i64::MAX),
    )
    .await?;
    let version = existing
        .as_ref()
        .map(|profile| profile.avatar_version.saturating_add(1))
        .unwrap_or(1);
    let avatar_root_dir = avatar::resolve_local_avatar_root_dir(&state.runtime_config)?;
    let prefix = user_avatar_dir(&avatar_root_dir, user_id, version);
    let prefix_value = prefix.to_string_lossy().into_owned();
    let small_path = avatar_variant_file_path(&prefix, AVATAR_SIZE_SM);
    let large_path = avatar_variant_file_path(&prefix, AVATAR_SIZE_LG);

    write_local_avatar(&small_path, &small_bytes).await?;
    if let Err(e) = write_local_avatar(&large_path, &large_bytes).await {
        cleanup_local_avatar_prefix(&prefix, &avatar_root_dir).await;
        return Err(e);
    }

    let now = Utc::now();
    let saved = match existing.clone() {
        Some(current) => {
            let mut active: user_profile::ActiveModel = current.into();
            active.avatar_source = Set(AvatarSource::Upload);
            active.avatar_policy_id = Set(None);
            active.avatar_key = Set(Some(prefix_value.clone()));
            active.avatar_version = Set(version);
            active.updated_at = Set(now);
            user_profile_repo::update(&state.db, active).await
        }
        None => {
            user_profile_repo::create(
                &state.db,
                user_profile::ActiveModel {
                    user_id: Set(user_id),
                    display_name: Set(None),
                    avatar_source: Set(AvatarSource::Upload),
                    avatar_policy_id: Set(None),
                    avatar_key: Set(Some(prefix_value.clone())),
                    avatar_version: Set(version),
                    created_at: Set(now),
                    updated_at: Set(now),
                },
            )
            .await
        }
    };

    let saved = match saved {
        Ok(model) => model,
        Err(err) => {
            cleanup_local_avatar_prefix(&prefix, &avatar_root_dir).await;
            return Err(err);
        }
    };

    if let Some(previous) = existing.as_ref() {
        delete_upload_objects(state, previous).await;
    }

    let gravatar_base_url = resolve_gravatar_base_url(state);
    Ok(build_profile_info(
        &user,
        Some(&saved),
        AvatarAudience::SelfUser,
        &gravatar_base_url,
    ))
}

pub async fn set_avatar_source(
    state: &AppState,
    user_id: i64,
    source: AvatarSource,
) -> Result<UserProfileInfo> {
    if source == AvatarSource::Upload {
        return Err(AsterError::validation_error(
            "upload avatar source must use the upload endpoint",
        ));
    }

    let user = user_repo::find_by_id(&state.db, user_id).await?;
    let existing = user_profile_repo::find_by_user_id(&state.db, user_id).await?;
    let gravatar_base_url = resolve_gravatar_base_url(state);

    if existing.is_none() && source == AvatarSource::None {
        return Ok(build_profile_info(
            &user,
            None,
            AvatarAudience::SelfUser,
            &gravatar_base_url,
        ));
    }

    let now = Utc::now();
    let saved = match existing.clone() {
        Some(current) => {
            let next_version = current.avatar_version.saturating_add(1);
            let mut active: user_profile::ActiveModel = current.into();
            active.avatar_source = Set(source);
            active.avatar_policy_id = Set(None);
            active.avatar_key = Set(None);
            active.avatar_version = Set(next_version);
            active.updated_at = Set(now);
            user_profile_repo::update(&state.db, active).await?
        }
        None => {
            user_profile_repo::create(
                &state.db,
                user_profile::ActiveModel {
                    user_id: Set(user_id),
                    display_name: Set(None),
                    avatar_source: Set(source),
                    avatar_policy_id: Set(None),
                    avatar_key: Set(None),
                    avatar_version: Set(0),
                    created_at: Set(now),
                    updated_at: Set(now),
                },
            )
            .await?
        }
    };

    if let Some(previous) = existing.as_ref() {
        delete_upload_objects(state, previous).await;
    }

    Ok(build_profile_info(
        &user,
        Some(&saved),
        AvatarAudience::SelfUser,
        &gravatar_base_url,
    ))
}

pub async fn update_profile(
    state: &AppState,
    user_id: i64,
    display_name: Option<String>,
) -> Result<UserProfileInfo> {
    let user = user_repo::find_by_id(&state.db, user_id).await?;
    let existing = user_profile_repo::find_by_user_id(&state.db, user_id).await?;
    let gravatar_base_url = resolve_gravatar_base_url(state);

    let Some(display_name) = display_name else {
        return Ok(build_profile_info(
            &user,
            existing.as_ref(),
            AvatarAudience::SelfUser,
            &gravatar_base_url,
        ));
    };

    let normalized = normalize_display_name(&display_name)?;
    let now = Utc::now();

    let saved = match existing {
        Some(current) => {
            if current.display_name == normalized {
                current
            } else {
                let mut active: user_profile::ActiveModel = current.into();
                active.display_name = Set(normalized);
                active.updated_at = Set(now);
                user_profile_repo::update(&state.db, active).await?
            }
        }
        None => {
            if normalized.is_none() {
                return Ok(build_profile_info(
                    &user,
                    None,
                    AvatarAudience::SelfUser,
                    &gravatar_base_url,
                ));
            }

            user_profile_repo::create(
                &state.db,
                user_profile::ActiveModel {
                    user_id: Set(user_id),
                    display_name: Set(normalized),
                    avatar_source: Set(AvatarSource::None),
                    avatar_policy_id: Set(None),
                    avatar_key: Set(None),
                    avatar_version: Set(0),
                    created_at: Set(now),
                    updated_at: Set(now),
                },
            )
            .await?
        }
    };

    Ok(build_profile_info(
        &user,
        Some(&saved),
        AvatarAudience::SelfUser,
        &gravatar_base_url,
    ))
}

fn validate_avatar_size(size: u32) -> Result<u32> {
    match size {
        AVATAR_SIZE_SM | AVATAR_SIZE_LG => Ok(size),
        _ => Err(AsterError::validation_error(
            "avatar size must be 512 or 1024",
        )),
    }
}

pub async fn get_avatar_bytes(state: &AppState, user_id: i64, size: u32) -> Result<Vec<u8>> {
    let size = validate_avatar_size(size)?;
    user_repo::find_by_id(&state.db, user_id).await?;
    let profile = user_profile_repo::find_by_user_id(&state.db, user_id)
        .await?
        .ok_or_else(|| AsterError::record_not_found(format!("profile for user #{user_id}")))?;

    if profile.avatar_source != AvatarSource::Upload {
        return Err(AsterError::record_not_found(format!(
            "user #{user_id} does not have an uploaded avatar"
        )));
    }

    let prefix = stored_avatar_prefix(Some(&profile))
        .ok_or_else(|| AsterError::record_not_found("avatar key missing"))?;
    if let Some(policy_id) = legacy_avatar_policy_id(&profile) {
        let policy = state.policy_snapshot.get_policy_or_err(policy_id)?;
        let driver = state.driver_registry.get_driver(&policy)?;
        let path = avatar_object_key(prefix, size);
        return driver
            .get(&path)
            .await
            .map_err(|_| AsterError::record_not_found(format!("avatar object {path}")));
    }

    let path = avatar_variant_file_path(Path::new(prefix), size);
    tokio::fs::read(&path)
        .await
        .map_err(|_| AsterError::record_not_found(format!("avatar object {}", path.display())))
}

pub fn avatar_image_response(bytes: Vec<u8>) -> HttpResponse {
    HttpResponse::Ok()
        .content_type("image/webp")
        .insert_header((
            "Cache-Control",
            format!("public, max-age={YEAR_SECS}, immutable"),
        ))
        .body(bytes)
}
