//! WOPI HTTP 路由映射。
//!
//! 本地路径固定对应官方 WOPI file endpoints：
//! - `GET  /api/v1/wopi/files/{id}` -> CheckFileInfo
//! - `POST /api/v1/wopi/files/{id}` -> Lock / Unlock / RefreshLock / RenameFile / PutRelativeFile / PutUserInfo
//! - `GET  /api/v1/wopi/files/{id}/contents` -> GetFile
//! - `POST /api/v1/wopi/files/{id}/contents` -> PutFile
//!
//! 维护这组路由时务必同时对照：
//! - https://learn.microsoft.com/en-us/microsoft-365/cloud-storage-partner-program/rest/files/checkfileinfo
//! - https://learn.microsoft.com/en-us/microsoft-365/cloud-storage-partner-program/rest/files/getfile
//! - https://learn.microsoft.com/en-us/microsoft-365/cloud-storage-partner-program/rest/files/putfile
//! - https://learn.microsoft.com/en-us/microsoft-365/cloud-storage-partner-program/rest/files/renamefile
//! - https://learn.microsoft.com/en-us/microsoft-365/cloud-storage-partner-program/rest/files/putrelativefile
//! - https://learn.microsoft.com/en-us/microsoft-365/cloud-storage-partner-program/rest/files/putuserinfo
//!
//! 这些路径还和 `session::build_public_wopi_src()`、PUT_RELATIVE 返回 URL 直接耦合，
//! 不能只改路由而不改 launch / response 生成逻辑。

use crate::api::dto::validate_request;
use crate::api::dto::wopi::WopiAccessQuery;
use crate::runtime::PrimaryAppState;
use crate::services::{file_service, wopi_service};
use actix_web::{HttpRequest, HttpResponse, web};

pub fn routes() -> impl actix_web::dev::HttpServiceFactory + use<> {
    web::scope("/wopi")
        .route("/files/{id}", web::get().to(check_file_info))
        .route("/files/{id}", web::post().to(file_operation))
        .route("/files/{id}/contents", web::get().to(get_file_contents))
        .route("/files/{id}/contents", web::post().to(put_file_contents))
}

#[api_docs_macros::path(
    get,
    path = "/api/v1/wopi/files/{id}",
    tag = "public",
    operation_id = "wopi_check_file_info",
    params(
        ("id" = i64, Path, description = "File ID"),
        ("access_token" = String, Query, description = "WOPI access token")
    ),
    responses((status = 200, description = "WOPI CheckFileInfo response")),
)]
pub async fn check_file_info(
    state: web::Data<PrimaryAppState>,
    req: HttpRequest,
    path: web::Path<i64>,
    query: web::Query<WopiAccessQuery>,
) -> HttpResponse {
    if let Err(error) = validate_request(&*query) {
        return protocol_error_response(error);
    }
    match wopi_service::check_file_info(
        &state,
        *path,
        &query.access_token,
        request_source(&state, &req),
    )
    .await
    {
        Ok(info) => HttpResponse::Ok().json(info),
        Err(error) => protocol_error_response(error),
    }
}

pub async fn get_file_contents(
    state: web::Data<PrimaryAppState>,
    req: HttpRequest,
    path: web::Path<i64>,
    query: web::Query<WopiAccessQuery>,
) -> HttpResponse {
    if let Err(error) = validate_request(&*query) {
        return protocol_error_response(error);
    }
    let if_none_match = req
        .headers()
        .get("If-None-Match")
        .and_then(|value| value.to_str().ok());
    let max_expected_size = optional_header_value(&req, "X-WOPI-MaxExpectedSize");
    match wopi_service::get_file_contents(
        &state,
        *path,
        &query.access_token,
        if_none_match,
        max_expected_size,
        request_source(&state, &req),
    )
    .await
    {
        Ok(result) => {
            let mut response = file_service::outcome_to_response(result.outcome);
            // X-WOPI-ItemVersion は 304 / 302 でも添付する（WOPI 仕様要求）
            if let Ok(version_value) =
                actix_web::http::header::HeaderValue::from_str(&result.item_version)
            {
                response.headers_mut().insert(
                    actix_web::http::header::HeaderName::from_static("x-wopi-itemversion"),
                    version_value,
                );
            }
            response
        }
        Err(error) => protocol_error_response(error),
    }
}

pub async fn put_file_contents(
    state: web::Data<PrimaryAppState>,
    req: HttpRequest,
    path: web::Path<i64>,
    query: web::Query<WopiAccessQuery>,
    mut payload: web::Payload,
) -> HttpResponse {
    if let Err(error) = validate_request(&*query) {
        return protocol_error_response(error);
    }
    let override_value = header_value(&req, "X-WOPI-Override");
    if !override_value.eq_ignore_ascii_case("PUT") {
        return HttpResponse::NotImplemented().finish();
    }

    match wopi_service::put_file_contents(
        &state,
        *path,
        &query.access_token,
        &mut payload,
        request_content_length(&req),
        optional_header_value(&req, "X-WOPI-Lock"),
        request_source(&state, &req),
    )
    .await
    {
        Ok(wopi_service::WopiPutFileResult::Success { item_version }) => HttpResponse::Ok()
            .insert_header(("X-WOPI-ItemVersion", item_version))
            .finish(),
        Ok(wopi_service::WopiPutFileResult::Conflict(conflict)) => conflict_response(&conflict),
        Err(error) => protocol_error_response(error),
    }
}

pub async fn file_operation(
    state: web::Data<PrimaryAppState>,
    req: HttpRequest,
    path: web::Path<i64>,
    query: web::Query<WopiAccessQuery>,
    mut payload: web::Payload,
) -> HttpResponse {
    if let Err(error) = validate_request(&*query) {
        return protocol_error_response(error);
    }
    let override_value = header_value(&req, "X-WOPI-Override");
    let requested_lock = optional_header_value(&req, "X-WOPI-Lock").unwrap_or_default();
    let old_lock = optional_header_value(&req, "X-WOPI-OldLock").unwrap_or_default();

    if override_value.eq_ignore_ascii_case("PUT_RELATIVE") {
        return match wopi_service::put_relative_file(
            &state,
            wopi_service::WopiPutRelativeRequest {
                file_id: *path,
                access_token: &query.access_token,
                payload: &mut payload,
                suggested_target: optional_header_value(&req, "X-WOPI-SuggestedTarget"),
                relative_target: optional_header_value(&req, "X-WOPI-RelativeTarget"),
                overwrite_relative_target: optional_header_value(
                    &req,
                    "X-WOPI-OverwriteRelativeTarget",
                ),
                size_header: optional_header_value(&req, "X-WOPI-Size"),
                content_length: request_content_length(&req),
                request_source: request_source(&state, &req),
            },
        )
        .await
        {
            Ok(wopi_service::WopiPutRelativeResult::Success(response)) => {
                HttpResponse::Ok().json(response)
            }
            Ok(wopi_service::WopiPutRelativeResult::Conflict(conflict)) => {
                put_relative_conflict_response(&conflict)
            }
            Err(error) => protocol_error_response(error),
        };
    }

    if override_value.eq_ignore_ascii_case("GET_LOCK") {
        return match wopi_service::get_lock(
            &state,
            *path,
            &query.access_token,
            request_source(&state, &req),
        )
        .await
        {
            Ok(wopi_service::WopiGetLockResult::Success { current_lock }) => HttpResponse::Ok()
                .insert_header(("X-WOPI-Lock", current_lock))
                .finish(),
            Ok(wopi_service::WopiGetLockResult::Conflict(conflict)) => conflict_response(&conflict),
            Err(error) => protocol_error_response(error),
        };
    }

    if override_value.eq_ignore_ascii_case("RENAME_FILE") {
        return match wopi_service::rename_file(
            &state,
            *path,
            &query.access_token,
            optional_header_value(&req, "X-WOPI-RequestedName"),
            optional_header_value(&req, "X-WOPI-Lock"),
            request_source(&state, &req),
        )
        .await
        {
            Ok(wopi_service::WopiRenameFileResult::Success(response)) => {
                HttpResponse::Ok().json(response)
            }
            Ok(wopi_service::WopiRenameFileResult::Conflict(conflict)) => {
                conflict_response(&conflict)
            }
            Ok(wopi_service::WopiRenameFileResult::InvalidName { reason }) => {
                invalid_file_name_response(&reason)
            }
            Err(error) => protocol_error_response(error),
        };
    }

    if override_value.eq_ignore_ascii_case("PUT_USER_INFO") {
        return match wopi_service::put_user_info(
            &state,
            *path,
            &query.access_token,
            &mut payload,
            request_source(&state, &req),
        )
        .await
        {
            Ok(()) => HttpResponse::Ok().finish(),
            Err(error) => protocol_error_response(error),
        };
    }

    let result = if override_value.eq_ignore_ascii_case("LOCK") && !old_lock.is_empty() {
        wopi_service::unlock_and_relock_file(
            &state,
            *path,
            &query.access_token,
            requested_lock,
            old_lock,
            request_source(&state, &req),
        )
        .await
    } else if override_value.eq_ignore_ascii_case("LOCK") {
        wopi_service::lock_file(
            &state,
            *path,
            &query.access_token,
            requested_lock,
            request_source(&state, &req),
        )
        .await
    } else if override_value.eq_ignore_ascii_case("UNLOCK") {
        wopi_service::unlock_file(
            &state,
            *path,
            &query.access_token,
            requested_lock,
            request_source(&state, &req),
        )
        .await
    } else if override_value.eq_ignore_ascii_case("REFRESH_LOCK") {
        wopi_service::refresh_lock(
            &state,
            *path,
            &query.access_token,
            requested_lock,
            request_source(&state, &req),
        )
        .await
    } else {
        return HttpResponse::NotImplemented().finish();
    };

    match result {
        Ok(wopi_service::WopiLockOperationResult::Success) => HttpResponse::Ok().finish(),
        Ok(wopi_service::WopiLockOperationResult::Conflict(conflict)) => {
            conflict_response(&conflict)
        }
        Err(error) => protocol_error_response(error),
    }
}

fn conflict_response(conflict: &wopi_service::WopiConflict) -> HttpResponse {
    let mut response = HttpResponse::Conflict();
    if let Some(current_lock) = &conflict.current_lock {
        response.insert_header(("X-WOPI-Lock", current_lock.as_str()));
    }
    response
        .insert_header(("X-WOPI-LockFailureReason", conflict.reason.as_str()))
        .finish()
}

fn put_relative_conflict_response(
    conflict: &wopi_service::WopiPutRelativeConflict,
) -> HttpResponse {
    let mut response = HttpResponse::Conflict();
    response.insert_header((
        "X-WOPI-Lock",
        conflict.current_lock.as_deref().unwrap_or_default(),
    ));
    if let Some(valid_target) = &conflict.valid_target {
        response.insert_header(("X-WOPI-ValidRelativeTarget", valid_target.as_str()));
    }
    response
        .insert_header(("X-WOPI-LockFailureReason", conflict.reason.as_str()))
        .finish()
}

fn invalid_file_name_response(reason: &str) -> HttpResponse {
    HttpResponse::BadRequest()
        .insert_header(("X-WOPI-InvalidFileNameError", reason))
        .finish()
}

fn protocol_error_response(error: crate::errors::AsterError) -> HttpResponse {
    actix_web::ResponseError::error_response(&error)
}

fn header_value(req: &HttpRequest, name: &str) -> String {
    optional_header_value(req, name)
        .unwrap_or_default()
        .to_string()
}

fn optional_header_value<'a>(req: &'a HttpRequest, name: &str) -> Option<&'a str> {
    req.headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn request_source<'a>(
    state: &PrimaryAppState,
    req: &'a HttpRequest,
) -> wopi_service::WopiRequestSource<'a> {
    wopi_service::WopiRequestSource {
        origin: optional_header_value(req, "Origin"),
        referer: optional_header_value(req, "Referer"),
        proof: optional_header_value(req, "X-WOPI-Proof"),
        proof_old: optional_header_value(req, "X-WOPI-ProofOld"),
        timestamp: optional_header_value(req, "X-WOPI-TimeStamp"),
        public_url: crate::config::site_url::public_site_url(&state.runtime_config)
            .map(|base| format!("{base}{}", req.uri())),
    }
}

fn request_content_length(req: &HttpRequest) -> Option<i64> {
    req.headers()
        .get(actix_web::http::header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .and_then(|value| crate::utils::numbers::u64_to_i64(value, "content length").ok())
}
