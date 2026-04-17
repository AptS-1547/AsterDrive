//! WOPI 文件操作入口。
//!
//! 这些函数把 WOPI 协议动作翻译回项目内部的 file/profile service。
//! 重点不是重新实现一套文件系统，而是复用已有的文件主链路，同时补上
//! WOPI 专用的 lock、rename、PUT_RELATIVE 语义。

use crate::db::repository::file_repo;
use crate::errors::{AsterError, MapAsterErr, Result};
use crate::runtime::AppState;
use crate::services::{file_service, profile_service};
use crate::types::NullablePatch;

use super::locks::{active_wopi_lock_value, ensure_wopi_lock_matches, load_active_lock};
use super::session::{resolve_access_token, scope_from_payload};
use super::targets::{
    PutRelativeTargetMode, build_put_relative_response, encode_wopi_filename,
    find_file_by_name_in_scope, normalize_requested_rename_target, parse_put_relative_request,
    resolve_available_rename_target, response_name_for_rename, store_relative_target_from_bytes,
    suggest_available_relative_target,
};
use super::types::{
    MAX_WOPI_USER_INFO_LEN, WOPI_FILE_NAME_MAX_LEN, WopiCheckFileInfo, WopiPutFileResult,
    WopiPutRelativeRequest, WopiPutRelativeResult, WopiRenameFileResponse, WopiRenameFileResult,
    WopiRequestSource,
};

pub async fn check_file_info(
    state: &AppState,
    file_id: i64,
    access_token: &str,
    request_source: WopiRequestSource<'_>,
) -> Result<WopiCheckFileInfo> {
    let resolved = resolve_access_token(state, file_id, access_token, request_source).await?;
    let blob = file_repo::find_blob_by_id(&state.db, resolved.file.blob_id).await?;
    let user_info =
        profile_service::get_wopi_user_info(state, resolved.payload.actor_user_id).await?;

    Ok(WopiCheckFileInfo {
        base_file_name: resolved.file.name.clone(),
        file_name_max_length: Some(WOPI_FILE_NAME_MAX_LEN),
        owner_id: resolved.file.user_id.to_string(),
        size: resolved.file.size,
        user_id: resolved.payload.actor_user_id.to_string(),
        user_can_not_write_relative: false,
        user_can_rename: true,
        user_info,
        user_can_write: true,
        read_only: false,
        supports_get_lock: true,
        supports_locks: true,
        supports_rename: true,
        supports_user_info: Some(true),
        supports_update: true,
        version: blob.hash,
    })
}

pub async fn get_file_contents(
    state: &AppState,
    file_id: i64,
    access_token: &str,
    if_none_match: Option<&str>,
    request_source: WopiRequestSource<'_>,
) -> Result<actix_web::HttpResponse> {
    let resolved = resolve_access_token(state, file_id, access_token, request_source).await?;
    let blob = file_repo::find_blob_by_id(&state.db, resolved.file.blob_id).await?;
    file_service::build_stream_response_with_disposition(
        state,
        &resolved.file,
        &blob,
        file_service::DownloadDisposition::Inline,
        if_none_match,
    )
    .await
}

pub async fn put_file_contents(
    state: &AppState,
    file_id: i64,
    access_token: &str,
    body: actix_web::web::Bytes,
    requested_lock: Option<&str>,
    request_source: WopiRequestSource<'_>,
) -> Result<WopiPutFileResult> {
    let resolved = resolve_access_token(state, file_id, access_token, request_source).await?;
    // WOPI 写回必须先走锁校验；否则外部编辑器会绕过项目内部锁系统直接覆盖内容。
    if let Some(conflict) =
        ensure_wopi_lock_matches(state, &resolved.payload, resolved.file.id, requested_lock).await?
    {
        return Ok(WopiPutFileResult::Conflict(conflict));
    }

    let (updated, item_version) = file_service::update_content_in_scope(
        state,
        scope_from_payload(&resolved.payload),
        resolved.file.id,
        body,
        None,
    )
    .await?;

    Ok(WopiPutFileResult::Success {
        item_version: item_version_if_present(updated.id, item_version),
    })
}

pub async fn put_relative_file(
    state: &AppState,
    req: WopiPutRelativeRequest<'_>,
) -> Result<WopiPutRelativeResult> {
    let WopiPutRelativeRequest {
        file_id,
        access_token,
        body,
        suggested_target,
        relative_target,
        overwrite_relative_target,
        size_header,
        request_source,
    } = req;
    let resolved = resolve_access_token(state, file_id, access_token, request_source).await?;
    let request = parse_put_relative_request(
        &resolved.file.name,
        suggested_target,
        relative_target,
        overwrite_relative_target,
        size_header,
        body.len(),
    )?;
    let scope = scope_from_payload(&resolved.payload);

    let target_file = match request.target_mode {
        PutRelativeTargetMode::Suggested(target_name) => {
            // SuggestedTarget 永远表示“新建一个可用名称”，不会覆盖现有文件。
            store_relative_target_from_bytes(
                state,
                scope,
                resolved.file.folder_id,
                &target_name,
                None,
                &body,
                false,
            )
            .await?
        }
        PutRelativeTargetMode::Relative {
            target_name,
            overwrite,
        } => {
            // RelativeTarget 先找目标名是否已存在，再根据 overwrite / 锁状态决定
            // 冲突、覆盖还是新建。
            let existing =
                find_file_by_name_in_scope(&state.db, scope, resolved.file.folder_id, &target_name)
                    .await?;

            let existing = match existing {
                Some(existing) => existing,
                None => {
                    store_relative_target_from_bytes(
                        state,
                        scope,
                        resolved.file.folder_id,
                        &target_name,
                        None,
                        &body,
                        true,
                    )
                    .await?
                }
            };

            if existing.id == resolved.file.id {
                return Err(AsterError::validation_error(
                    "PUT_RELATIVE target must differ from source file",
                ));
            }

            if !overwrite {
                let valid_target = encode_wopi_filename(
                    &suggest_available_relative_target(
                        state,
                        scope,
                        resolved.file.folder_id,
                        &target_name,
                    )
                    .await?,
                );
                return Ok(WopiPutRelativeResult::Conflict(
                    super::types::WopiPutRelativeConflict {
                        current_lock: Some(String::new()),
                        reason: "target file already exists".to_string(),
                        valid_target: Some(valid_target),
                    },
                ));
            }

            if let Some(active_lock) = load_active_lock(state, existing.id).await? {
                return Ok(WopiPutRelativeResult::Conflict(
                    super::types::WopiPutRelativeConflict {
                        current_lock: Some(
                            active_wopi_lock_value(&active_lock).unwrap_or_default(),
                        ),
                        reason: "target file is locked".to_string(),
                        valid_target: None,
                    },
                ));
            }

            store_relative_target_from_bytes(
                state,
                scope,
                resolved.file.folder_id,
                &target_name,
                Some(existing.id),
                &body,
                true,
            )
            .await?
        }
    };

    let response =
        build_put_relative_response(state, &resolved.payload, &target_file.name, target_file.id)
            .await?;
    Ok(WopiPutRelativeResult::Success(response))
}

pub async fn rename_file(
    state: &AppState,
    file_id: i64,
    access_token: &str,
    requested_name: Option<&str>,
    requested_lock: Option<&str>,
    request_source: WopiRequestSource<'_>,
) -> Result<WopiRenameFileResult> {
    let resolved = resolve_access_token(state, file_id, access_token, request_source).await?;
    if let Some(conflict) =
        ensure_wopi_lock_matches(state, &resolved.payload, resolved.file.id, requested_lock).await?
    {
        return Ok(WopiRenameFileResult::Conflict(conflict));
    }

    let requested_name =
        match normalize_requested_rename_target(&resolved.file.name, requested_name) {
            Ok(name) => name,
            Err(reason) => return Ok(WopiRenameFileResult::InvalidName { reason }),
        };
    let scope = scope_from_payload(&resolved.payload);
    let mut final_name = resolve_available_rename_target(
        state,
        scope,
        resolved.file.folder_id,
        resolved.file.id,
        &requested_name,
    )
    .await?;

    let updated = match file_service::update_in_scope(
        state,
        scope,
        resolved.file.id,
        Some(final_name.clone()),
        NullablePatch::Absent,
    )
    .await
    {
        Ok(updated) => updated,
        Err(err) if file_repo::is_duplicate_name_error(&err, &final_name) => {
            // 即使前面已经算过一个可用名字，这里仍然要接受并发重命名造成的唯一键冲突，
            // 然后再退一步建议新的可用名。
            final_name = suggest_available_relative_target(
                state,
                scope,
                resolved.file.folder_id,
                &final_name,
            )
            .await?;
            file_service::update_in_scope(
                state,
                scope,
                resolved.file.id,
                Some(final_name),
                NullablePatch::Absent,
            )
            .await?
        }
        Err(err) => return Err(err),
    };

    Ok(WopiRenameFileResult::Success(WopiRenameFileResponse {
        name: response_name_for_rename(&resolved.file.name, &updated.name).to_string(),
    }))
}

pub async fn put_user_info(
    state: &AppState,
    file_id: i64,
    access_token: &str,
    body: actix_web::web::Bytes,
    request_source: WopiRequestSource<'_>,
) -> Result<()> {
    let resolved = resolve_access_token(state, file_id, access_token, request_source).await?;
    let user_info = normalize_wopi_user_info(&body)?;
    profile_service::update_wopi_user_info(state, resolved.payload.actor_user_id, user_info).await
}

fn item_version_if_present(_file_id: i64, item_version: String) -> String {
    item_version
}

fn normalize_wopi_user_info(body: &actix_web::web::Bytes) -> Result<String> {
    let user_info = std::str::from_utf8(body).map_aster_err_with(|| {
        AsterError::validation_error("PUT_USER_INFO body must be valid UTF-8")
    })?;
    if !user_info.is_ascii() {
        return Err(AsterError::validation_error(
            "PUT_USER_INFO body must contain ASCII characters only",
        ));
    }
    if user_info.len() > MAX_WOPI_USER_INFO_LEN {
        return Err(AsterError::validation_error(format!(
            "PUT_USER_INFO body must be {MAX_WOPI_USER_INFO_LEN} bytes or fewer"
        )));
    }
    Ok(user_info.to_string())
}
