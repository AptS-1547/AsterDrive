//! 管理员 API 路由：`policies`。

use crate::api::dto::admin as dto;
use crate::api::pagination::LimitOffsetQuery;
#[cfg(all(debug_assertions, feature = "openapi"))]
use crate::api::pagination::OffsetPage;
use crate::api::response::ApiResponse;
use crate::errors::Result;
use crate::runtime::AppState;
use crate::services::{audit_service, auth_service::Claims, policy_service};
use crate::types::DriverType;
use actix_web::{HttpRequest, HttpResponse, web};

// Re-export DTOs for backwards compatibility with admin/mod.rs re-exports
pub use dto::{
    CreatePolicyGroupReq, CreatePolicyReq, MigratePolicyGroupUsersReq, PatchPolicyGroupReq,
    PatchPolicyReq, PolicyGroupItemReq, TestPolicyParamsReq,
};

// ── Conversion helpers (must stay here because they use policy_service types) ──────────

fn build_policy_connection_input(
    driver_type: DriverType,
    endpoint: Option<String>,
    bucket: Option<String>,
    access_key: Option<String>,
    secret_key: Option<String>,
    base_path: Option<String>,
) -> policy_service::StoragePolicyConnectionInput {
    policy_service::StoragePolicyConnectionInput {
        driver_type,
        endpoint: endpoint.unwrap_or_default(),
        bucket: bucket.unwrap_or_default(),
        access_key: access_key.unwrap_or_default(),
        secret_key: secret_key.unwrap_or_default(),
        base_path: base_path.unwrap_or_default(),
    }
}

impl From<CreatePolicyReq> for policy_service::CreateStoragePolicyInput {
    fn from(value: CreatePolicyReq) -> Self {
        Self {
            name: value.name,
            connection: build_policy_connection_input(
                value.driver_type,
                value.endpoint,
                value.bucket,
                value.access_key,
                value.secret_key,
                value.base_path,
            ),
            max_file_size: value.max_file_size.unwrap_or(0),
            chunk_size: value.chunk_size,
            is_default: value.is_default.unwrap_or(false),
            allowed_types: value.allowed_types,
            options: value.options,
        }
    }
}

impl From<PatchPolicyReq> for policy_service::UpdateStoragePolicyInput {
    fn from(value: PatchPolicyReq) -> Self {
        Self {
            name: value.name,
            endpoint: value.endpoint,
            bucket: value.bucket,
            access_key: value.access_key,
            secret_key: value.secret_key,
            base_path: value.base_path,
            max_file_size: value.max_file_size,
            chunk_size: value.chunk_size,
            is_default: value.is_default,
            allowed_types: value.allowed_types,
            options: value.options,
        }
    }
}

impl From<TestPolicyParamsReq> for policy_service::StoragePolicyConnectionInput {
    fn from(value: TestPolicyParamsReq) -> Self {
        build_policy_connection_input(
            value.driver_type,
            value.endpoint,
            value.bucket,
            value.access_key,
            value.secret_key,
            value.base_path,
        )
    }
}

fn map_group_items(
    items: Vec<PolicyGroupItemReq>,
) -> Vec<policy_service::StoragePolicyGroupItemInput> {
    items.into_iter().map(Into::into).collect()
}

impl From<PolicyGroupItemReq> for policy_service::StoragePolicyGroupItemInput {
    fn from(value: PolicyGroupItemReq) -> Self {
        Self {
            policy_id: value.policy_id,
            priority: value.priority,
            min_file_size: value.min_file_size,
            max_file_size: value.max_file_size,
        }
    }
}

impl From<CreatePolicyGroupReq> for policy_service::CreateStoragePolicyGroupInput {
    fn from(value: CreatePolicyGroupReq) -> Self {
        Self {
            name: value.name,
            description: value.description,
            is_enabled: value.is_enabled,
            is_default: value.is_default,
            items: map_group_items(value.items),
        }
    }
}

impl From<PatchPolicyGroupReq> for policy_service::UpdateStoragePolicyGroupInput {
    fn from(value: PatchPolicyGroupReq) -> Self {
        Self {
            name: value.name,
            description: value.description,
            is_enabled: value.is_enabled,
            is_default: value.is_default,
            items: value.items.map(map_group_items),
        }
    }
}

#[api_docs_macros::path(
    get,
    path = "/api/v1/admin/policies",
    tag = "admin",
    operation_id = "list_policies",
    params(LimitOffsetQuery),
    responses(
        (status = 200, description = "List storage policies", body = inline(ApiResponse<OffsetPage<policy_service::StoragePolicy>>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer" = [])),
)]
pub async fn list_policies(
    state: web::Data<AppState>,
    query: web::Query<LimitOffsetQuery>,
) -> Result<HttpResponse> {
    let policies =
        policy_service::list_paginated(&state, query.limit_or(50, 100), query.offset()).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(policies)))
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/admin/policies",
    tag = "admin",
    operation_id = "create_policy",
    request_body = CreatePolicyReq,
    responses(
        (status = 201, description = "Policy created", body = inline(ApiResponse<policy_service::StoragePolicy>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer" = [])),
)]
pub async fn create_policy(
    state: web::Data<AppState>,
    body: web::Json<CreatePolicyReq>,
) -> Result<HttpResponse> {
    let policy = policy_service::create(&state, body.into_inner().into()).await?;
    Ok(HttpResponse::Created().json(ApiResponse::ok(policy)))
}

#[api_docs_macros::path(
    get,
    path = "/api/v1/admin/policies/{id}",
    tag = "admin",
    operation_id = "get_policy",
    params(("id" = i64, Path, description = "Policy ID")),
    responses(
        (status = 200, description = "Policy details", body = inline(ApiResponse<policy_service::StoragePolicy>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Policy not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn get_policy(state: web::Data<AppState>, path: web::Path<i64>) -> Result<HttpResponse> {
    let policy = policy_service::get(&state, *path).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(policy)))
}

#[api_docs_macros::path(
    patch,
    path = "/api/v1/admin/policies/{id}",
    tag = "admin",
    operation_id = "update_policy",
    params(("id" = i64, Path, description = "Policy ID")),
    request_body = PatchPolicyReq,
    responses(
        (status = 200, description = "Policy updated", body = inline(ApiResponse<policy_service::StoragePolicy>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Policy not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn update_policy(
    state: web::Data<AppState>,
    path: web::Path<i64>,
    body: web::Json<PatchPolicyReq>,
) -> Result<HttpResponse> {
    let policy = policy_service::update(&state, *path, body.into_inner().into()).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(policy)))
}

#[api_docs_macros::path(
    delete,
    path = "/api/v1/admin/policies/{id}",
    tag = "admin",
    operation_id = "delete_policy",
    params(("id" = i64, Path, description = "Policy ID")),
    responses(
        (status = 200, description = "Policy deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Policy not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn delete_policy(
    state: web::Data<AppState>,
    path: web::Path<i64>,
) -> Result<HttpResponse> {
    policy_service::delete(&state, *path).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::<()>::ok_empty()))
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/admin/policies/{id}/test",
    tag = "admin",
    operation_id = "test_policy_connection",
    params(("id" = i64, Path, description = "Policy ID")),
    responses(
        (status = 200, description = "Connection successful"),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Connection failed"),
    ),
    security(("bearer" = [])),
)]
pub async fn test_policy_connection(
    state: web::Data<AppState>,
    path: web::Path<i64>,
) -> Result<HttpResponse> {
    policy_service::test_connection(&state, *path).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::<()>::ok_empty()))
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/admin/policies/test",
    tag = "admin",
    operation_id = "test_policy_params",
    request_body = TestPolicyParamsReq,
    responses(
        (status = 200, description = "Connection successful"),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Connection failed"),
    ),
    security(("bearer" = [])),
)]
pub async fn test_policy_params(body: web::Json<TestPolicyParamsReq>) -> Result<HttpResponse> {
    policy_service::test_connection_params(body.into_inner().into()).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::<()>::ok_empty()))
}

#[api_docs_macros::path(
    get,
    path = "/api/v1/admin/policy-groups",
    tag = "admin",
    operation_id = "list_policy_groups",
    params(LimitOffsetQuery),
    responses(
        (status = 200, description = "List storage policy groups", body = inline(ApiResponse<OffsetPage<crate::services::policy_service::StoragePolicyGroupInfo>>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer" = [])),
)]
pub async fn list_policy_groups(
    state: web::Data<AppState>,
    query: web::Query<LimitOffsetQuery>,
) -> Result<HttpResponse> {
    let groups =
        policy_service::list_groups_paginated(&state, query.limit_or(50, 100), query.offset())
            .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(groups)))
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/admin/policy-groups",
    tag = "admin",
    operation_id = "create_policy_group",
    request_body = CreatePolicyGroupReq,
    responses(
        (status = 201, description = "Policy group created", body = inline(ApiResponse<crate::services::policy_service::StoragePolicyGroupInfo>)),
        (status = 400, description = "Bad Request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer" = [])),
)]
pub async fn create_policy_group(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    req: HttpRequest,
    body: web::Json<CreatePolicyGroupReq>,
) -> Result<HttpResponse> {
    let group = policy_service::create_group(&state, body.into_inner().into()).await?;
    let ctx = audit_service::AuditContext::from_request(&req, &claims);
    audit_service::log(
        &state,
        &ctx,
        audit_service::AuditAction::AdminCreatePolicyGroup,
        Some("policy_group"),
        Some(group.id),
        Some(&group.name),
        audit_service::details(audit_service::PolicyGroupAuditDetails {
            is_default: group.is_default,
            is_enabled: group.is_enabled,
            item_count: group.items.len(),
        }),
    )
    .await;
    Ok(HttpResponse::Created().json(ApiResponse::ok(group)))
}

#[api_docs_macros::path(
    get,
    path = "/api/v1/admin/policy-groups/{id}",
    tag = "admin",
    operation_id = "get_policy_group",
    params(("id" = i64, Path, description = "Policy group ID")),
    responses(
        (status = 200, description = "Policy group details", body = inline(ApiResponse<crate::services::policy_service::StoragePolicyGroupInfo>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Policy group not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn get_policy_group(
    state: web::Data<AppState>,
    path: web::Path<i64>,
) -> Result<HttpResponse> {
    let group = policy_service::get_group(&state, *path).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(group)))
}

#[api_docs_macros::path(
    patch,
    path = "/api/v1/admin/policy-groups/{id}",
    tag = "admin",
    operation_id = "update_policy_group",
    params(("id" = i64, Path, description = "Policy group ID")),
    request_body = PatchPolicyGroupReq,
    responses(
        (status = 200, description = "Policy group updated", body = inline(ApiResponse<crate::services::policy_service::StoragePolicyGroupInfo>)),
        (status = 400, description = "Bad Request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Policy group not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn update_policy_group(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    req: HttpRequest,
    path: web::Path<i64>,
    body: web::Json<PatchPolicyGroupReq>,
) -> Result<HttpResponse> {
    let group = policy_service::update_group(&state, *path, body.into_inner().into()).await?;
    let ctx = audit_service::AuditContext::from_request(&req, &claims);
    audit_service::log(
        &state,
        &ctx,
        audit_service::AuditAction::AdminUpdatePolicyGroup,
        Some("policy_group"),
        Some(group.id),
        Some(&group.name),
        audit_service::details(audit_service::PolicyGroupAuditDetails {
            is_default: group.is_default,
            is_enabled: group.is_enabled,
            item_count: group.items.len(),
        }),
    )
    .await;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(group)))
}

#[api_docs_macros::path(
    delete,
    path = "/api/v1/admin/policy-groups/{id}",
    tag = "admin",
    operation_id = "delete_policy_group",
    params(("id" = i64, Path, description = "Policy group ID")),
    responses(
        (status = 200, description = "Policy group removed"),
        (status = 400, description = "Bad Request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Policy group not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn delete_policy_group(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    req: HttpRequest,
    path: web::Path<i64>,
) -> Result<HttpResponse> {
    let group = policy_service::get_group(&state, *path).await?;
    policy_service::delete_group(&state, *path).await?;
    let ctx = audit_service::AuditContext::from_request(&req, &claims);
    audit_service::log(
        &state,
        &ctx,
        audit_service::AuditAction::AdminDeletePolicyGroup,
        Some("policy_group"),
        Some(group.id),
        Some(&group.name),
        audit_service::details(audit_service::PolicyGroupAuditDetails {
            is_default: group.is_default,
            is_enabled: group.is_enabled,
            item_count: group.items.len(),
        }),
    )
    .await;
    Ok(HttpResponse::Ok().json(ApiResponse::<()>::ok_empty()))
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/admin/policy-groups/{id}/migrate-users",
    tag = "admin",
    operation_id = "migrate_policy_group_users",
    params(("id" = i64, Path, description = "Source policy group ID")),
    request_body = MigratePolicyGroupUsersReq,
    responses(
        (status = 200, description = "Policy group users migrated", body = inline(ApiResponse<crate::services::policy_service::PolicyGroupUserMigrationResult>)),
        (status = 400, description = "Bad Request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Policy group not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn migrate_policy_group_users(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    req: HttpRequest,
    path: web::Path<i64>,
    body: web::Json<MigratePolicyGroupUsersReq>,
) -> Result<HttpResponse> {
    let source_group = policy_service::get_group(&state, *path).await?;
    let target_group = policy_service::get_group(&state, body.target_group_id).await?;
    let result = policy_service::migrate_group_users(&state, *path, body.target_group_id).await?;
    let ctx = audit_service::AuditContext::from_request(&req, &claims);
    audit_service::log(
        &state,
        &ctx,
        audit_service::AuditAction::AdminMigratePolicyGroupUsers,
        Some("policy_group"),
        Some(source_group.id),
        Some(&source_group.name),
        audit_service::details(audit_service::PolicyGroupMigrationDetails {
            source_group_id: source_group.id,
            source_group_name: &source_group.name,
            target_group_id: target_group.id,
            target_group_name: &target_group.name,
            affected_users: result.affected_users,
            migrated_assignments: result.migrated_assignments,
        }),
    )
    .await;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(result)))
}
