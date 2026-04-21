//! 管理员 API 路由：`remote_nodes`。

use crate::api::dto::admin::{CreateRemoteNodeReq, PatchRemoteNodeReq, TestRemoteNodeParamsReq};
use crate::api::dto::validate_request;
use crate::api::pagination::LimitOffsetQuery;
#[cfg(all(debug_assertions, feature = "openapi"))]
use crate::api::pagination::OffsetPage;
use crate::api::response::ApiResponse;
use crate::errors::Result;
use crate::runtime::PrimaryAppState;
use crate::services::{managed_follower_enrollment_service, managed_follower_service};
use actix_web::{HttpResponse, web};

impl From<CreateRemoteNodeReq> for managed_follower_service::CreateRemoteNodeInput {
    fn from(value: CreateRemoteNodeReq) -> Self {
        Self {
            name: value.name,
            base_url: value.base_url.unwrap_or_default(),
            namespace: value.namespace,
            is_enabled: value.is_enabled,
        }
    }
}

impl From<PatchRemoteNodeReq> for managed_follower_service::UpdateRemoteNodeInput {
    fn from(value: PatchRemoteNodeReq) -> Self {
        Self {
            name: value.name,
            base_url: value.base_url,
            namespace: value.namespace,
            is_enabled: value.is_enabled,
        }
    }
}

impl From<TestRemoteNodeParamsReq> for managed_follower_service::TestRemoteNodeInput {
    fn from(value: TestRemoteNodeParamsReq) -> Self {
        Self {
            base_url: value.base_url,
            access_key: value.access_key,
            secret_key: value.secret_key,
        }
    }
}

#[api_docs_macros::path(
    get,
    path = "/api/v1/admin/remote-nodes",
    tag = "admin",
    operation_id = "list_remote_nodes",
    params(LimitOffsetQuery),
    responses(
        (status = 200, description = "List remote nodes", body = inline(ApiResponse<OffsetPage<managed_follower_service::RemoteNodeInfo>>)),
        (status = 401, description = crate::api::constants::OPENAPI_UNAUTHORIZED),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer" = [])),
)]
pub async fn list_remote_nodes(
    state: web::Data<PrimaryAppState>,
    query: web::Query<LimitOffsetQuery>,
) -> Result<HttpResponse> {
    let nodes =
        managed_follower_service::list_paginated(&state, query.limit_or(50, 100), query.offset())
            .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(nodes)))
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/admin/remote-nodes",
    tag = "admin",
    operation_id = "create_remote_node",
    request_body = CreateRemoteNodeReq,
    responses(
        (status = 201, description = "Remote node created", body = inline(ApiResponse<managed_follower_service::RemoteNodeInfo>)),
        (status = 401, description = crate::api::constants::OPENAPI_UNAUTHORIZED),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer" = [])),
)]
pub async fn create_remote_node(
    state: web::Data<PrimaryAppState>,
    body: web::Json<CreateRemoteNodeReq>,
) -> Result<HttpResponse> {
    validate_request(&*body)?;
    let node = managed_follower_service::create(&state, body.into_inner().into()).await?;
    Ok(HttpResponse::Created().json(ApiResponse::ok(node)))
}

#[api_docs_macros::path(
    get,
    path = "/api/v1/admin/remote-nodes/{id}",
    tag = "admin",
    operation_id = "get_remote_node",
    params(("id" = i64, Path, description = "Remote node ID")),
    responses(
        (status = 200, description = "Remote node details", body = inline(ApiResponse<managed_follower_service::RemoteNodeInfo>)),
        (status = 401, description = crate::api::constants::OPENAPI_UNAUTHORIZED),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Remote node not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn get_remote_node(
    state: web::Data<PrimaryAppState>,
    path: web::Path<i64>,
) -> Result<HttpResponse> {
    let node = managed_follower_service::get(&state, *path).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(node)))
}

#[api_docs_macros::path(
    patch,
    path = "/api/v1/admin/remote-nodes/{id}",
    tag = "admin",
    operation_id = "update_remote_node",
    params(("id" = i64, Path, description = "Remote node ID")),
    request_body = PatchRemoteNodeReq,
    responses(
        (status = 200, description = "Remote node updated", body = inline(ApiResponse<managed_follower_service::RemoteNodeInfo>)),
        (status = 401, description = crate::api::constants::OPENAPI_UNAUTHORIZED),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Remote node not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn update_remote_node(
    state: web::Data<PrimaryAppState>,
    path: web::Path<i64>,
    body: web::Json<PatchRemoteNodeReq>,
) -> Result<HttpResponse> {
    validate_request(&*body)?;
    let node = managed_follower_service::update(&state, *path, body.into_inner().into()).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(node)))
}

#[api_docs_macros::path(
    delete,
    path = "/api/v1/admin/remote-nodes/{id}",
    tag = "admin",
    operation_id = "delete_remote_node",
    params(("id" = i64, Path, description = "Remote node ID")),
    responses(
        (status = 200, description = "Remote node deleted"),
        (status = 401, description = crate::api::constants::OPENAPI_UNAUTHORIZED),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Remote node not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn delete_remote_node(
    state: web::Data<PrimaryAppState>,
    path: web::Path<i64>,
) -> Result<HttpResponse> {
    managed_follower_service::delete(&state, *path).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::<()>::ok_empty()))
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/admin/remote-nodes/{id}/test",
    tag = "admin",
    operation_id = "test_remote_node_connection",
    params(("id" = i64, Path, description = "Remote node ID")),
    responses(
        (status = 200, description = "Remote node connection tested", body = inline(ApiResponse<managed_follower_service::RemoteNodeInfo>)),
        (status = 400, description = "Connection failed"),
        (status = 401, description = crate::api::constants::OPENAPI_UNAUTHORIZED),
        (status = 403, description = "Forbidden"),
        (status = 412, description = "Remote node is disabled or not ready"),
        (status = 404, description = "Remote node not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn test_remote_node(
    state: web::Data<PrimaryAppState>,
    path: web::Path<i64>,
) -> Result<HttpResponse> {
    let node = managed_follower_service::test_connection(&state, *path).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(node)))
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/admin/remote-nodes/test",
    tag = "admin",
    operation_id = "test_remote_node_params",
    request_body = TestRemoteNodeParamsReq,
    responses(
        (status = 200, description = "Remote node connection successful", body = inline(ApiResponse<crate::storage::remote_protocol::RemoteStorageCapabilities>)),
        (status = 401, description = crate::api::constants::OPENAPI_UNAUTHORIZED),
        (status = 403, description = "Forbidden"),
        (status = 400, description = "Connection failed"),
    ),
    security(("bearer" = [])),
)]
pub async fn test_remote_node_params(
    body: web::Json<TestRemoteNodeParamsReq>,
) -> Result<HttpResponse> {
    validate_request(&*body)?;
    let capabilities =
        managed_follower_service::test_connection_params(body.into_inner().into()).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(capabilities)))
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/admin/remote-nodes/{id}/enrollment-token",
    tag = "admin",
    operation_id = "create_remote_node_enrollment_token",
    params(("id" = i64, Path, description = "Remote node ID")),
    responses(
        (status = 201, description = "Enrollment command created", body = ApiResponse<managed_follower_enrollment_service::RemoteEnrollmentCommandInfo>),
        (status = 401, description = crate::api::constants::OPENAPI_UNAUTHORIZED),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Remote node not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn create_remote_node_enrollment_token(
    state: web::Data<PrimaryAppState>,
    path: web::Path<i64>,
) -> Result<HttpResponse> {
    let command =
        managed_follower_enrollment_service::create_enrollment_command(&state, *path).await?;
    Ok(HttpResponse::Created().json(ApiResponse::ok(command)))
}
