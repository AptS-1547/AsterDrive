use crate::runtime::AppState;
use crate::services::wopi_service;
use actix_web::{HttpRequest, HttpResponse, http::StatusCode, web};
use serde::Deserialize;

pub fn routes() -> impl actix_web::dev::HttpServiceFactory + use<> {
    web::scope("/wopi")
        .route("/files/{id}", web::get().to(check_file_info))
        .route("/files/{id}", web::post().to(file_operation))
        .route("/files/{id}/contents", web::get().to(get_file_contents))
        .route("/files/{id}/contents", web::post().to(put_file_contents))
}

#[derive(Deserialize)]
pub struct WopiAccessQuery {
    pub access_token: String,
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
    state: web::Data<AppState>,
    req: HttpRequest,
    path: web::Path<i64>,
    query: web::Query<WopiAccessQuery>,
) -> HttpResponse {
    match wopi_service::check_file_info(&state, *path, &query.access_token, request_source(&req))
        .await
    {
        Ok(info) => HttpResponse::Ok().json(info),
        Err(error) => protocol_error_response(error),
    }
}

pub async fn get_file_contents(
    state: web::Data<AppState>,
    req: HttpRequest,
    path: web::Path<i64>,
    query: web::Query<WopiAccessQuery>,
) -> HttpResponse {
    let if_none_match = req
        .headers()
        .get("If-None-Match")
        .and_then(|value| value.to_str().ok());
    match wopi_service::get_file_contents(
        &state,
        *path,
        &query.access_token,
        if_none_match,
        request_source(&req),
    )
    .await
    {
        Ok(response) => response,
        Err(error) => protocol_error_response(error),
    }
}

pub async fn put_file_contents(
    state: web::Data<AppState>,
    req: HttpRequest,
    path: web::Path<i64>,
    query: web::Query<WopiAccessQuery>,
    body: web::Bytes,
) -> HttpResponse {
    let override_value = header_value(&req, "X-WOPI-Override");
    if !override_value.eq_ignore_ascii_case("PUT") {
        return HttpResponse::NotImplemented().finish();
    }

    match wopi_service::put_file_contents(
        &state,
        *path,
        &query.access_token,
        body,
        optional_header_value(&req, "X-WOPI-Lock"),
        request_source(&req),
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
    state: web::Data<AppState>,
    req: HttpRequest,
    path: web::Path<i64>,
    query: web::Query<WopiAccessQuery>,
) -> HttpResponse {
    let override_value = header_value(&req, "X-WOPI-Override");
    let requested_lock = optional_header_value(&req, "X-WOPI-Lock").unwrap_or_default();

    let result = if override_value.eq_ignore_ascii_case("LOCK") {
        wopi_service::lock_file(
            &state,
            *path,
            &query.access_token,
            requested_lock,
            request_source(&req),
        )
        .await
    } else if override_value.eq_ignore_ascii_case("UNLOCK") {
        wopi_service::unlock_file(
            &state,
            *path,
            &query.access_token,
            requested_lock,
            request_source(&req),
        )
        .await
    } else if override_value.eq_ignore_ascii_case("REFRESH_LOCK") {
        wopi_service::refresh_lock(
            &state,
            *path,
            &query.access_token,
            requested_lock,
            request_source(&req),
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

fn protocol_error_response(error: crate::errors::AsterError) -> HttpResponse {
    let status = match error.http_status() {
        StatusCode::FORBIDDEN => StatusCode::UNAUTHORIZED,
        other => other,
    };
    HttpResponse::build(status)
        .content_type("text/plain; charset=utf-8")
        .body(error.message().to_string())
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

fn request_source(req: &HttpRequest) -> wopi_service::WopiRequestSource<'_> {
    wopi_service::WopiRequestSource {
        origin: optional_header_value(req, "Origin"),
        referer: optional_header_value(req, "Referer"),
    }
}
