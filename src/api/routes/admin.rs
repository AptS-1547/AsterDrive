use crate::api::middleware::auth::JwtAuth;
use crate::api::middleware::rate_limit;
use crate::api::pagination::{LimitOffsetQuery, OffsetPage};
use crate::api::response::{ApiResponse, RemovedCountResponse};
use crate::config::RateLimitConfig;
use crate::errors::Result;
use crate::runtime::AppState;
use crate::services::{
    audit_service, auth_service::Claims, config_service, policy_service, profile_service,
    share_service, user_service,
};
use crate::types::{DriverType, UserRole, UserStatus};
use actix_governor::Governor;
use actix_web::middleware::Condition;
use actix_web::{HttpResponse, web};
use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};

pub fn routes(rl: &RateLimitConfig) -> impl actix_web::dev::HttpServiceFactory + use<> {
    let limiter = rate_limit::build_governor(&rl.write);

    web::scope("/admin")
        .wrap(JwtAuth)
        .wrap(Condition::new(rl.enabled, Governor::new(&limiter)))
        // policies
        .route("/policies", web::get().to(list_policies))
        .route("/policies", web::post().to(create_policy))
        .route("/policies/{id}", web::get().to(get_policy))
        .route("/policies/{id}", web::patch().to(update_policy))
        .route("/policies/{id}", web::delete().to(delete_policy))
        .route(
            "/policies/{id}/test",
            web::post().to(test_policy_connection),
        )
        .route("/policies/test", web::post().to(test_policy_params))
        // users
        .route("/users", web::get().to(list_users))
        .route("/users", web::post().to(create_user))
        .route("/users/{id}", web::get().to(get_user))
        .route("/users/{id}", web::patch().to(update_user))
        .route("/users/{id}", web::delete().to(force_delete_user))
        .route("/users/{id}/avatar/{size}", web::get().to(get_user_avatar))
        // user storage policies
        .route(
            "/users/{user_id}/policies",
            web::get().to(list_user_policies),
        )
        .route(
            "/users/{user_id}/policies",
            web::post().to(assign_user_policy),
        )
        .route(
            "/users/{user_id}/policies/{id}",
            web::patch().to(update_user_policy),
        )
        .route(
            "/users/{user_id}/policies/{id}",
            web::delete().to(remove_user_policy),
        )
        // shares
        .route("/shares", web::get().to(list_all_shares))
        .route("/shares/{id}", web::delete().to(admin_delete_share))
        // config
        .route("/config", web::get().to(list_config))
        .route("/config/schema", web::get().to(config_schema))
        .route("/config/{key}", web::get().to(get_config))
        .route("/config/{key}", web::put().to(set_config))
        .route("/config/{key}", web::delete().to(delete_config))
        // audit logs
        .route("/audit-logs", web::get().to(list_audit_logs))
        // webdav locks
        .route("/locks", web::get().to(list_locks))
        .route("/locks/expired", web::delete().to(cleanup_expired_locks))
        .route("/locks/{id}", web::delete().to(force_unlock))
}

// ── Policies ─────────────────────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/v1/admin/policies",
    tag = "admin",
    operation_id = "list_policies",
    params(LimitOffsetQuery),
    responses(
        (status = 200, description = "List storage policies", body = inline(ApiResponse<OffsetPage<crate::entities::storage_policy::Model>>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer" = [])),
)]
pub async fn list_policies(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    query: web::Query<LimitOffsetQuery>,
) -> Result<HttpResponse> {
    require_admin(&claims)?;
    let policies =
        policy_service::list_paginated(&state, query.limit_or(50, 100), query.offset()).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(policies)))
}

#[derive(Deserialize, ToSchema)]
pub struct CreatePolicyReq {
    pub name: String,
    pub driver_type: DriverType,
    pub endpoint: Option<String>,
    pub bucket: Option<String>,
    pub access_key: Option<String>,
    pub secret_key: Option<String>,
    pub base_path: Option<String>,
    pub max_file_size: Option<i64>,
    pub chunk_size: Option<i64>,
    pub is_default: Option<bool>,
    pub options: Option<String>,
}

#[utoipa::path(
    post,
    path = "/api/v1/admin/policies",
    tag = "admin",
    operation_id = "create_policy",
    request_body = CreatePolicyReq,
    responses(
        (status = 201, description = "Policy created", body = inline(ApiResponse<crate::entities::storage_policy::Model>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer" = [])),
)]
pub async fn create_policy(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    body: web::Json<CreatePolicyReq>,
) -> Result<HttpResponse> {
    require_admin(&claims)?;
    let policy = policy_service::create(
        &state,
        &body.name,
        body.driver_type,
        body.endpoint.as_deref().unwrap_or_default(),
        body.bucket.as_deref().unwrap_or_default(),
        body.access_key.as_deref().unwrap_or_default(),
        body.secret_key.as_deref().unwrap_or_default(),
        body.base_path.as_deref().unwrap_or_default(),
        body.max_file_size.unwrap_or(0),
        body.chunk_size,
        body.is_default.unwrap_or(false),
        body.options.clone(),
    )
    .await?;
    Ok(HttpResponse::Created().json(ApiResponse::ok(policy)))
}

#[utoipa::path(
    get,
    path = "/api/v1/admin/policies/{id}",
    tag = "admin",
    operation_id = "get_policy",
    params(("id" = i64, Path, description = "Policy ID")),
    responses(
        (status = 200, description = "Policy details", body = inline(ApiResponse<crate::entities::storage_policy::Model>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Policy not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn get_policy(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<i64>,
) -> Result<HttpResponse> {
    require_admin(&claims)?;
    let policy = policy_service::get(&state, *path).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(policy)))
}

#[derive(Deserialize, ToSchema)]
pub struct PatchPolicyReq {
    pub name: Option<String>,
    pub endpoint: Option<String>,
    pub bucket: Option<String>,
    pub access_key: Option<String>,
    pub secret_key: Option<String>,
    pub base_path: Option<String>,
    pub max_file_size: Option<i64>,
    pub chunk_size: Option<i64>,
    pub is_default: Option<bool>,
    pub options: Option<String>,
}

#[utoipa::path(
    patch,
    path = "/api/v1/admin/policies/{id}",
    tag = "admin",
    operation_id = "update_policy",
    params(("id" = i64, Path, description = "Policy ID")),
    request_body = PatchPolicyReq,
    responses(
        (status = 200, description = "Policy updated", body = inline(ApiResponse<crate::entities::storage_policy::Model>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Policy not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn update_policy(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<i64>,
    body: web::Json<PatchPolicyReq>,
) -> Result<HttpResponse> {
    require_admin(&claims)?;
    let body = body.into_inner();
    let policy = policy_service::update(
        &state,
        *path,
        body.name,
        body.endpoint,
        body.bucket,
        body.access_key,
        body.secret_key,
        body.base_path,
        body.max_file_size,
        body.chunk_size,
        body.is_default,
        body.options,
    )
    .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(policy)))
}

#[utoipa::path(
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
    claims: web::ReqData<Claims>,
    path: web::Path<i64>,
) -> Result<HttpResponse> {
    require_admin(&claims)?;
    policy_service::delete(&state, *path).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::<()>::ok_empty()))
}

#[derive(Deserialize, ToSchema)]
pub struct TestPolicyParamsReq {
    pub driver_type: DriverType,
    pub endpoint: Option<String>,
    pub bucket: Option<String>,
    pub access_key: Option<String>,
    pub secret_key: Option<String>,
    pub base_path: Option<String>,
}

#[utoipa::path(
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
    claims: web::ReqData<Claims>,
    path: web::Path<i64>,
) -> Result<HttpResponse> {
    require_admin(&claims)?;
    policy_service::test_connection(&state, *path).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::<()>::ok_empty()))
}

#[utoipa::path(
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
pub async fn test_policy_params(
    claims: web::ReqData<Claims>,
    body: web::Json<TestPolicyParamsReq>,
) -> Result<HttpResponse> {
    require_admin(&claims)?;
    policy_service::test_connection_params(
        body.driver_type,
        body.endpoint.as_deref().unwrap_or_default(),
        body.bucket.as_deref().unwrap_or_default(),
        body.access_key.as_deref().unwrap_or_default(),
        body.secret_key.as_deref().unwrap_or_default(),
        body.base_path.as_deref().unwrap_or_default(),
    )
    .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::<()>::ok_empty()))
}

// ── Users ────────────────────────────────────────────────────────────

#[derive(Deserialize, IntoParams)]
pub struct AdminUserListQuery {
    pub keyword: Option<String>,
    pub role: Option<UserRole>,
    pub status: Option<UserStatus>,
}

#[derive(Deserialize, ToSchema)]
pub struct CreateUserReq {
    pub username: String,
    pub email: String,
    pub password: String,
}

#[utoipa::path(
    post,
    path = "/api/v1/admin/users",
    tag = "admin",
    operation_id = "create_user",
    request_body = CreateUserReq,
    responses(
        (status = 201, description = "User created", body = inline(ApiResponse<crate::services::user_service::UserInfo>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 400, description = "Validation error"),
    ),
    security(("bearer" = [])),
)]
pub async fn create_user(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    req: actix_web::HttpRequest,
    body: web::Json<CreateUserReq>,
) -> Result<HttpResponse> {
    require_admin(&claims)?;
    let ctx = audit_service::AuditContext::from_request(&req, &claims);
    let user = user_service::create(&state, &body.username, &body.email, &body.password).await?;
    audit_service::log(
        &state,
        &ctx,
        audit_service::AuditAction::AdminCreateUser,
        Some("user"),
        Some(user.id),
        Some(&user.username),
        audit_service::details(audit_service::AdminCreateUserDetails {
            email: &user.email,
            role: user.role,
            status: user.status,
            storage_quota: user.storage_quota,
        }),
    )
    .await;
    Ok(HttpResponse::Created().json(ApiResponse::ok(user)))
}

#[utoipa::path(
    get,
    path = "/api/v1/admin/users",
    tag = "admin",
    operation_id = "list_users",
    params(LimitOffsetQuery, AdminUserListQuery),
    responses(
        (status = 200, description = "List users", body = inline(ApiResponse<OffsetPage<crate::services::user_service::UserInfo>>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer" = [])),
)]
pub async fn list_users(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    page: web::Query<LimitOffsetQuery>,
    query: web::Query<AdminUserListQuery>,
) -> Result<HttpResponse> {
    require_admin(&claims)?;
    let users = user_service::list_paginated(
        &state,
        page.limit_or(50, 100),
        page.offset(),
        query.keyword.as_deref(),
        query.role,
        query.status,
    )
    .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(users)))
}

#[utoipa::path(
    get,
    path = "/api/v1/admin/users/{id}",
    tag = "admin",
    operation_id = "get_user",
    params(("id" = i64, Path, description = "User ID")),
    responses(
        (status = 200, description = "User details", body = inline(ApiResponse<crate::services::user_service::UserInfo>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "User not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn get_user(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<i64>,
) -> Result<HttpResponse> {
    require_admin(&claims)?;
    let user = user_service::get(&state, *path).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(user)))
}

#[derive(Deserialize, ToSchema)]
pub struct PatchUserReq {
    pub role: Option<UserRole>,
    pub status: Option<UserStatus>,
    pub storage_quota: Option<i64>,
}

#[utoipa::path(
    patch,
    path = "/api/v1/admin/users/{id}",
    tag = "admin",
    operation_id = "update_user",
    params(("id" = i64, Path, description = "User ID")),
    request_body = PatchUserReq,
    responses(
        (status = 200, description = "User updated", body = inline(ApiResponse<crate::services::user_service::UserInfo>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "User not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn update_user(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    req: actix_web::HttpRequest,
    path: web::Path<i64>,
    body: web::Json<PatchUserReq>,
) -> Result<HttpResponse> {
    require_admin(&claims)?;
    let target_id = *path;
    let body = body.into_inner();
    let ctx = audit_service::AuditContext::from_request(&req, &claims);
    let user = user_service::update(
        &state,
        target_id,
        body.role,
        body.status,
        body.storage_quota,
    )
    .await?;
    audit_service::log(
        &state,
        &ctx,
        audit_service::AuditAction::AdminUpdateUser,
        Some("user"),
        Some(user.id),
        Some(&user.username),
        audit_service::details(audit_service::AdminUpdateUserDetails {
            role: user.role,
            status: user.status,
            storage_quota: user.storage_quota,
        }),
    )
    .await;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(user)))
}

#[utoipa::path(
    delete,
    path = "/api/v1/admin/users/{id}",
    tag = "admin",
    operation_id = "force_delete_user",
    params(("id" = i64, Path, description = "User ID")),
    responses(
        (status = 200, description = "User and all data permanently deleted"),
        (status = 400, description = "Cannot delete admin user"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Admin required"),
        (status = 404, description = "User not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn force_delete_user(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<i64>,
) -> Result<HttpResponse> {
    require_admin(&claims)?;
    user_service::force_delete(&state, *path).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::<()>::ok_empty()))
}

#[utoipa::path(
    get,
    path = "/api/v1/admin/users/{id}/avatar/{size}",
    tag = "admin",
    operation_id = "get_user_avatar",
    params(
        ("id" = i64, Path, description = "User ID"),
        ("size" = u32, Path, description = "Avatar size (512 or 1024)")
    ),
    responses(
        (status = 200, description = "Avatar image (WebP)"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Avatar not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn get_user_avatar(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<(i64, u32)>,
) -> Result<HttpResponse> {
    require_admin(&claims)?;
    let (user_id, size) = path.into_inner();
    let bytes = profile_service::get_avatar_bytes(&state, user_id, size).await?;
    Ok(profile_service::avatar_image_response(bytes))
}

// ── User Storage Policies ───────────────────────────────────────────

#[derive(Deserialize, ToSchema)]
pub struct UserPolicyPath {
    pub user_id: i64,
}

#[derive(Deserialize, ToSchema)]
pub struct UserPolicyItemPath {
    pub user_id: i64,
    pub id: i64,
}

#[utoipa::path(
    get,
    path = "/api/v1/admin/users/{user_id}/policies",
    tag = "admin",
    operation_id = "list_user_policies",
    params(("user_id" = i64, Path, description = "User ID"), LimitOffsetQuery),
    responses(
        (status = 200, description = "User policy assignments", body = inline(ApiResponse<OffsetPage<crate::entities::user_storage_policy::Model>>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer" = [])),
)]
pub async fn list_user_policies(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<UserPolicyPath>,
    query: web::Query<LimitOffsetQuery>,
) -> Result<HttpResponse> {
    require_admin(&claims)?;
    let policies = policy_service::list_user_policies_paginated(
        &state,
        path.user_id,
        query.limit_or(50, 100),
        query.offset(),
    )
    .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(policies)))
}

#[derive(Deserialize, ToSchema)]
pub struct AssignUserPolicyReq {
    pub policy_id: i64,
    #[serde(default)]
    pub is_default: bool,
    #[serde(default)]
    pub quota_bytes: i64,
}

#[utoipa::path(
    post,
    path = "/api/v1/admin/users/{user_id}/policies",
    tag = "admin",
    operation_id = "assign_user_policy",
    params(("user_id" = i64, Path, description = "User ID")),
    request_body = AssignUserPolicyReq,
    responses(
        (status = 201, description = "Policy assigned", body = inline(ApiResponse<crate::entities::user_storage_policy::Model>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Policy not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn assign_user_policy(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<UserPolicyPath>,
    body: web::Json<AssignUserPolicyReq>,
) -> Result<HttpResponse> {
    require_admin(&claims)?;
    let usp = policy_service::assign_user_policy(
        &state,
        path.user_id,
        body.policy_id,
        body.is_default,
        body.quota_bytes,
    )
    .await?;
    Ok(HttpResponse::Created().json(ApiResponse::ok(usp)))
}

#[derive(Deserialize, ToSchema)]
pub struct PatchUserPolicyReq {
    pub is_default: Option<bool>,
    pub quota_bytes: Option<i64>,
}

#[utoipa::path(
    patch,
    path = "/api/v1/admin/users/{user_id}/policies/{id}",
    tag = "admin",
    operation_id = "update_user_policy",
    params(
        ("user_id" = i64, Path, description = "User ID"),
        ("id" = i64, Path, description = "User storage policy assignment ID"),
    ),
    request_body = PatchUserPolicyReq,
    responses(
        (status = 200, description = "Assignment updated", body = inline(ApiResponse<crate::entities::user_storage_policy::Model>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Assignment not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn update_user_policy(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<UserPolicyItemPath>,
    body: web::Json<PatchUserPolicyReq>,
) -> Result<HttpResponse> {
    require_admin(&claims)?;
    let usp =
        policy_service::update_user_policy(&state, path.id, body.is_default, body.quota_bytes)
            .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(usp)))
}

#[utoipa::path(
    delete,
    path = "/api/v1/admin/users/{user_id}/policies/{id}",
    tag = "admin",
    operation_id = "remove_user_policy",
    params(
        ("user_id" = i64, Path, description = "User ID"),
        ("id" = i64, Path, description = "User storage policy assignment ID"),
    ),
    responses(
        (status = 200, description = "Assignment removed"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Assignment not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn remove_user_policy(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<UserPolicyItemPath>,
) -> Result<HttpResponse> {
    require_admin(&claims)?;
    policy_service::remove_user_policy(&state, path.id).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::<()>::ok_empty()))
}

// ── Shares ──────────────────────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/v1/admin/shares",
    tag = "admin",
    operation_id = "list_all_shares",
    params(LimitOffsetQuery),
    responses(
        (status = 200, description = "All shares", body = inline(ApiResponse<OffsetPage<crate::entities::share::Model>>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer" = [])),
)]
pub async fn list_all_shares(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    query: web::Query<LimitOffsetQuery>,
) -> Result<HttpResponse> {
    require_admin(&claims)?;
    let shares =
        share_service::list_paginated(&state, query.limit_or(50, 100), query.offset()).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(shares)))
}

#[utoipa::path(
    delete,
    path = "/api/v1/admin/shares/{id}",
    tag = "admin",
    operation_id = "admin_delete_share",
    params(("id" = i64, Path, description = "Share ID")),
    responses(
        (status = 200, description = "Share deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Share not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn admin_delete_share(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<i64>,
) -> Result<HttpResponse> {
    require_admin(&claims)?;
    share_service::admin_delete_share(&state, *path).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::<()>::ok_empty()))
}

// ── System Config ────────────────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/v1/admin/config",
    tag = "admin",
    operation_id = "list_config",
    params(LimitOffsetQuery),
    responses(
        (status = 200, description = "List config entries", body = inline(ApiResponse<OffsetPage<crate::entities::system_config::Model>>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer" = [])),
)]
pub async fn list_config(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    query: web::Query<LimitOffsetQuery>,
) -> Result<HttpResponse> {
    require_admin(&claims)?;
    let configs =
        config_service::list_paginated(&state, query.limit_or(50, 100), query.offset()).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(configs)))
}

#[utoipa::path(
    get,
    path = "/api/v1/admin/config/schema",
    tag = "admin",
    operation_id = "config_schema",
    responses(
        (status = 200, description = "Config schema", body = inline(ApiResponse<Vec<config_service::ConfigSchemaItem>>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer" = [])),
)]
pub async fn config_schema(claims: web::ReqData<Claims>) -> Result<HttpResponse> {
    require_admin(&claims)?;
    let schema = config_service::get_schema();
    Ok(HttpResponse::Ok().json(ApiResponse::ok(schema)))
}

#[utoipa::path(
    get,
    path = "/api/v1/admin/config/{key}",
    tag = "admin",
    operation_id = "get_config",
    params(("key" = String, Path, description = "Config key")),
    responses(
        (status = 200, description = "Config entry", body = inline(ApiResponse<crate::entities::system_config::Model>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Config key not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn get_config(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<String>,
) -> Result<HttpResponse> {
    require_admin(&claims)?;
    let config = config_service::get_by_key(&state, &path).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(config)))
}

#[derive(Deserialize, ToSchema)]
pub struct SetConfigReq {
    pub value: String,
}

#[utoipa::path(
    put,
    path = "/api/v1/admin/config/{key}",
    tag = "admin",
    operation_id = "set_config",
    params(("key" = String, Path, description = "Config key")),
    request_body = SetConfigReq,
    responses(
        (status = 200, description = "Config value set", body = inline(ApiResponse<crate::entities::system_config::Model>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer" = [])),
)]
pub async fn set_config(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    req: actix_web::HttpRequest,
    path: web::Path<String>,
    body: web::Json<SetConfigReq>,
) -> Result<HttpResponse> {
    require_admin(&claims)?;
    let ctx = audit_service::AuditContext::from_request(&req, &claims);
    let config =
        config_service::set_with_audit(&state, &path, &body.value, claims.user_id, &ctx).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(config)))
}

#[utoipa::path(
    delete,
    path = "/api/v1/admin/config/{key}",
    tag = "admin",
    operation_id = "delete_config",
    params(("key" = String, Path, description = "Config key")),
    responses(
        (status = 200, description = "Config entry deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Config key not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn delete_config(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<String>,
) -> Result<HttpResponse> {
    require_admin(&claims)?;
    config_service::delete(&state, &path).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::<()>::ok_empty()))
}

// ── Helpers ──────────────────────────────────────────────────────────

// ── WebDAV Locks ────────────────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/v1/admin/locks",
    tag = "admin",
    operation_id = "list_locks",
    params(LimitOffsetQuery),
    responses(
        (status = 200, description = "All WebDAV locks", body = inline(ApiResponse<OffsetPage<crate::entities::resource_lock::Model>>)),
        (status = 403, description = "Admin required"),
    ),
    security(("bearer" = [])),
)]
pub async fn list_locks(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    query: web::Query<LimitOffsetQuery>,
) -> Result<HttpResponse> {
    require_admin(&claims)?;
    let locks = crate::services::lock_service::list_paginated(
        &state,
        query.limit_or(50, 100),
        query.offset(),
    )
    .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(locks)))
}

#[utoipa::path(
    delete,
    path = "/api/v1/admin/locks/{id}",
    tag = "admin",
    operation_id = "force_unlock",
    params(("id" = i64, Path, description = "Lock ID")),
    responses(
        (status = 200, description = "Lock released"),
        (status = 403, description = "Admin required"),
        (status = 404, description = "Lock not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn force_unlock(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<i64>,
) -> Result<HttpResponse> {
    require_admin(&claims)?;
    crate::services::lock_service::force_unlock(&state, *path).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::<()>::ok_empty()))
}

#[utoipa::path(
    delete,
    path = "/api/v1/admin/locks/expired",
    tag = "admin",
    operation_id = "cleanup_expired_locks",
    responses(
        (status = 200, description = "Expired locks cleaned up", body = inline(ApiResponse<crate::api::response::RemovedCountResponse>)),
        (status = 403, description = "Admin required"),
    ),
    security(("bearer" = [])),
)]
pub async fn cleanup_expired_locks(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    require_admin(&claims)?;
    let count = crate::services::lock_service::cleanup_expired(&state).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(RemovedCountResponse { removed: count })))
}

// ── Audit Logs ─────────────────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/v1/admin/audit-logs",
    tag = "admin",
    operation_id = "list_audit_logs",
    params(LimitOffsetQuery, audit_service::AuditLogFilterQuery),
    responses(
        (status = 200, description = "Audit log entries", body = inline(ApiResponse<OffsetPage<crate::entities::audit_log::Model>>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer" = [])),
)]
pub async fn list_audit_logs(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    page: web::Query<LimitOffsetQuery>,
    query: web::Query<audit_service::AuditLogFilterQuery>,
) -> Result<HttpResponse> {
    require_admin(&claims)?;

    let filters = audit_service::AuditLogFilters::from_query(&query);
    let page = audit_service::query(&state, filters, page.limit_or(50, 200), page.offset()).await?;

    Ok(HttpResponse::Ok().json(ApiResponse::ok(page)))
}

// ── Helpers ──────────────────────────────────────────────────────────

fn require_admin(claims: &Claims) -> Result<()> {
    use crate::errors::AsterError;
    if !claims.role.is_admin() {
        return Err(AsterError::auth_forbidden("admin role required"));
    }
    Ok(())
}
