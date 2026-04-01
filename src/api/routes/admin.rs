use crate::api::middleware::{admin::RequireAdmin, auth::JwtAuth, rate_limit};
use crate::api::pagination::LimitOffsetQuery;
#[cfg(all(debug_assertions, feature = "openapi"))]
use crate::api::pagination::OffsetPage;
use crate::api::response::{ApiResponse, RemovedCountResponse};
use crate::config::RateLimitConfig;
use crate::errors::Result;
use crate::runtime::AppState;
use crate::services::{
    admin_service, audit_service,
    auth_service::{self, Claims},
    config_service, policy_service, profile_service, share_service, user_service,
};
use crate::types::{DriverType, UserRole, UserStatus};
use actix_governor::Governor;
use actix_web::middleware::Condition;
use actix_web::{HttpResponse, web};
use serde::{Deserialize, de::Error as DeError};
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::{IntoParams, ToSchema};

pub fn routes(rl: &RateLimitConfig) -> impl actix_web::dev::HttpServiceFactory + use<> {
    let limiter = rate_limit::build_governor(&rl.write);

    web::scope("/admin")
        .wrap(Condition::new(rl.enabled, Governor::new(&limiter)))
        .service(
            web::scope("").wrap(JwtAuth).service(
                web::scope("")
                    .wrap(RequireAdmin)
                    .route("/overview", web::get().to(get_overview))
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
                    // policy groups
                    .route("/policy-groups", web::get().to(list_policy_groups))
                    .route("/policy-groups", web::post().to(create_policy_group))
                    .route("/policy-groups/{id}", web::get().to(get_policy_group))
                    .route("/policy-groups/{id}", web::patch().to(update_policy_group))
                    .route("/policy-groups/{id}", web::delete().to(delete_policy_group))
                    .route(
                        "/policy-groups/{id}/migrate-users",
                        web::post().to(migrate_policy_group_users),
                    )
                    // users
                    .route("/users", web::get().to(list_users))
                    .route("/users", web::post().to(create_user))
                    .route("/users/{id}", web::get().to(get_user))
                    .route("/users/{id}", web::patch().to(update_user))
                    .route("/users/{id}/password", web::put().to(reset_user_password))
                    .route(
                        "/users/{id}/sessions/revoke",
                        web::post().to(revoke_user_sessions),
                    )
                    .route("/users/{id}", web::delete().to(force_delete_user))
                    .route("/users/{id}/avatar/{size}", web::get().to(get_user_avatar))
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
                    .route("/locks/{id}", web::delete().to(force_unlock)),
            ),
        )
}

// ── Policies ─────────────────────────────────────────────────────────

#[api_docs_macros::path(
    get,
    path = "/api/v1/admin/overview",
    tag = "admin",
    operation_id = "get_admin_overview",
    params(admin_service::AdminOverviewQuery),
    responses(
        (status = 200, description = "Admin overview", body = inline(ApiResponse<crate::services::admin_service::AdminOverview>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer" = [])),
)]
pub async fn get_overview(
    state: web::Data<AppState>,
    query: web::Query<admin_service::AdminOverviewQuery>,
) -> Result<HttpResponse> {
    let overview = admin_service::get_overview(
        &state,
        query.days_or_default(),
        query.timezone_name(),
        query.event_limit_or_default(),
    )
    .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(overview)))
}

#[api_docs_macros::path(
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
    query: web::Query<LimitOffsetQuery>,
) -> Result<HttpResponse> {
    let policies =
        policy_service::list_paginated(&state, query.limit_or(50, 100), query.offset()).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(policies)))
}

#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
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
            options: value.options,
        }
    }
}

#[api_docs_macros::path(
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
        (status = 200, description = "Policy details", body = inline(ApiResponse<crate::entities::storage_policy::Model>)),
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

#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
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
            options: value.options,
        }
    }
}

#[api_docs_macros::path(
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

#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct TestPolicyParamsReq {
    pub driver_type: DriverType,
    pub endpoint: Option<String>,
    pub bucket: Option<String>,
    pub access_key: Option<String>,
    pub secret_key: Option<String>,
    pub base_path: Option<String>,
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

// ── Policy Groups ───────────────────────────────────────────────────

#[derive(Clone, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct PolicyGroupItemReq {
    pub policy_id: i64,
    pub priority: i32,
    #[serde(default)]
    pub min_file_size: i64,
    #[serde(default)]
    pub max_file_size: i64,
}

#[derive(Clone, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct CreatePolicyGroupReq {
    pub name: String,
    pub description: Option<String>,
    #[serde(default = "default_true")]
    pub is_enabled: bool,
    #[serde(default)]
    pub is_default: bool,
    pub items: Vec<PolicyGroupItemReq>,
}

#[derive(Clone, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct PatchPolicyGroupReq {
    pub name: Option<String>,
    pub description: Option<String>,
    pub is_enabled: Option<bool>,
    pub is_default: Option<bool>,
    pub items: Option<Vec<PolicyGroupItemReq>>,
}

#[derive(Clone, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct MigratePolicyGroupUsersReq {
    pub target_group_id: i64,
}

fn default_true() -> bool {
    true
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
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer" = [])),
)]
pub async fn create_policy_group(
    state: web::Data<AppState>,
    body: web::Json<CreatePolicyGroupReq>,
) -> Result<HttpResponse> {
    let group = policy_service::create_group(&state, body.into_inner().into()).await?;
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
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Policy group not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn update_policy_group(
    state: web::Data<AppState>,
    path: web::Path<i64>,
    body: web::Json<PatchPolicyGroupReq>,
) -> Result<HttpResponse> {
    let group = policy_service::update_group(&state, *path, body.into_inner().into()).await?;
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
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Policy group not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn delete_policy_group(
    state: web::Data<AppState>,
    path: web::Path<i64>,
) -> Result<HttpResponse> {
    policy_service::delete_group(&state, *path).await?;
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
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Policy group not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn migrate_policy_group_users(
    state: web::Data<AppState>,
    path: web::Path<i64>,
    body: web::Json<MigratePolicyGroupUsersReq>,
) -> Result<HttpResponse> {
    let result = policy_service::migrate_group_users(&state, *path, body.target_group_id).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(result)))
}

// ── Users ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(IntoParams))]
pub struct AdminUserListQuery {
    pub keyword: Option<String>,
    pub role: Option<UserRole>,
    pub status: Option<UserStatus>,
}

#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct CreateUserReq {
    pub username: String,
    pub email: String,
    pub password: String,
}

#[api_docs_macros::path(
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
            policy_group_id: user.policy_group_id,
        }),
    )
    .await;
    Ok(HttpResponse::Created().json(ApiResponse::ok(user)))
}

#[api_docs_macros::path(
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
    page: web::Query<LimitOffsetQuery>,
    query: web::Query<AdminUserListQuery>,
) -> Result<HttpResponse> {
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

#[api_docs_macros::path(
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
pub async fn get_user(state: web::Data<AppState>, path: web::Path<i64>) -> Result<HttpResponse> {
    let user = user_service::get(&state, *path).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(user)))
}

#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct PatchUserReq {
    pub role: Option<UserRole>,
    pub status: Option<UserStatus>,
    pub storage_quota: Option<i64>,
    /// Omitted means "leave unchanged". Explicit `null` is rejected because this
    /// endpoint only supports assigning a policy group, not unassigning one.
    #[serde(default, deserialize_with = "deserialize_non_null_policy_group_id")]
    #[cfg_attr(
        all(debug_assertions, feature = "openapi"),
        schema(value_type = Option<i64>, nullable = false)
    )]
    pub policy_group_id: Option<i64>,
}

fn deserialize_non_null_policy_group_id<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<i64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    match Option::<i64>::deserialize(deserializer)? {
        Some(policy_group_id) => Ok(Some(policy_group_id)),
        None => Err(D::Error::custom("policy_group_id cannot be null")),
    }
}

#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct ResetUserPasswordReq {
    pub password: String,
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/admin/users/{id}/sessions/revoke",
    tag = "admin",
    operation_id = "revoke_user_sessions",
    params(("id" = i64, Path, description = "User ID")),
    responses(
        (status = 200, description = "User sessions revoked"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "User not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn revoke_user_sessions(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    req: actix_web::HttpRequest,
    path: web::Path<i64>,
) -> Result<HttpResponse> {
    let user = auth_service::revoke_user_sessions(&state, *path).await?;
    let ctx = audit_service::AuditContext::from_request(&req, &claims);
    audit_service::log(
        &state,
        &ctx,
        audit_service::AuditAction::AdminRevokeUserSessions,
        Some("user"),
        Some(user.id),
        Some(&user.username),
        None,
    )
    .await;
    Ok(HttpResponse::Ok().json(ApiResponse::<()>::ok_empty()))
}

#[api_docs_macros::path(
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
    let target_id = *path;
    let body = body.into_inner();
    let ctx = audit_service::AuditContext::from_request(&req, &claims);
    let user = user_service::update(
        &state,
        target_id,
        body.role,
        body.status,
        body.storage_quota,
        body.policy_group_id,
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
            policy_group_id: user.policy_group_id,
        }),
    )
    .await;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(user)))
}

#[api_docs_macros::path(
    put,
    path = "/api/v1/admin/users/{id}/password",
    tag = "admin",
    operation_id = "reset_user_password",
    params(("id" = i64, Path, description = "User ID")),
    request_body = ResetUserPasswordReq,
    responses(
        (status = 200, description = "User password reset"),
        (status = 400, description = "Validation error"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "User not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn reset_user_password(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    req: actix_web::HttpRequest,
    path: web::Path<i64>,
    body: web::Json<ResetUserPasswordReq>,
) -> Result<HttpResponse> {
    let user = auth_service::set_password(&state, *path, &body.password).await?;
    let ctx = audit_service::AuditContext::from_request(&req, &claims);
    audit_service::log(
        &state,
        &ctx,
        audit_service::AuditAction::AdminResetUserPassword,
        Some("user"),
        Some(user.id),
        Some(&user.username),
        None,
    )
    .await;
    Ok(HttpResponse::Ok().json(ApiResponse::<()>::ok_empty()))
}

#[api_docs_macros::path(
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
    path: web::Path<i64>,
) -> Result<HttpResponse> {
    user_service::force_delete(&state, *path).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::<()>::ok_empty()))
}

#[api_docs_macros::path(
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
    path: web::Path<(i64, u32)>,
) -> Result<HttpResponse> {
    let (user_id, size) = path.into_inner();
    let bytes = profile_service::get_avatar_bytes(&state, user_id, size).await?;
    Ok(profile_service::avatar_image_response(bytes))
}

// ── Shares ──────────────────────────────────────────────────────────

#[api_docs_macros::path(
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
    query: web::Query<LimitOffsetQuery>,
) -> Result<HttpResponse> {
    let shares =
        share_service::list_paginated(&state, query.limit_or(50, 100), query.offset()).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(shares)))
}

#[api_docs_macros::path(
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
    path: web::Path<i64>,
) -> Result<HttpResponse> {
    share_service::admin_delete_share(&state, *path).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::<()>::ok_empty()))
}

// ── System Config ────────────────────────────────────────────────────

#[api_docs_macros::path(
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
    query: web::Query<LimitOffsetQuery>,
) -> Result<HttpResponse> {
    let configs =
        config_service::list_paginated(&state, query.limit_or(50, 100), query.offset()).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(configs)))
}

#[api_docs_macros::path(
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
pub async fn config_schema() -> Result<HttpResponse> {
    let schema = config_service::get_schema();
    Ok(HttpResponse::Ok().json(ApiResponse::ok(schema)))
}

#[api_docs_macros::path(
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
    path: web::Path<String>,
) -> Result<HttpResponse> {
    let config = config_service::get_by_key(&state, &path).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(config)))
}

#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct SetConfigReq {
    pub value: String,
}

#[api_docs_macros::path(
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
    let ctx = audit_service::AuditContext::from_request(&req, &claims);
    let config =
        config_service::set_with_audit(&state, &path, &body.value, claims.user_id, &ctx).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(config)))
}

#[api_docs_macros::path(
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
    path: web::Path<String>,
) -> Result<HttpResponse> {
    config_service::delete(&state, &path).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::<()>::ok_empty()))
}

// ── WebDAV Locks ────────────────────────────────────────────────────

#[api_docs_macros::path(
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
    query: web::Query<LimitOffsetQuery>,
) -> Result<HttpResponse> {
    let locks = crate::services::lock_service::list_paginated(
        &state,
        query.limit_or(50, 100),
        query.offset(),
    )
    .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(locks)))
}

#[api_docs_macros::path(
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
    path: web::Path<i64>,
) -> Result<HttpResponse> {
    crate::services::lock_service::force_unlock(&state, *path).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::<()>::ok_empty()))
}

#[api_docs_macros::path(
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
pub async fn cleanup_expired_locks(state: web::Data<AppState>) -> Result<HttpResponse> {
    let count = crate::services::lock_service::cleanup_expired(&state).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(RemovedCountResponse { removed: count })))
}

// ── Audit Logs ─────────────────────────────────────────────────────

#[api_docs_macros::path(
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
    page: web::Query<LimitOffsetQuery>,
    query: web::Query<audit_service::AuditLogFilterQuery>,
) -> Result<HttpResponse> {
    let filters = audit_service::AuditLogFilters::from_query(&query);
    let page = audit_service::query(&state, filters, page.limit_or(50, 200), page.offset()).await?;

    Ok(HttpResponse::Ok().json(ApiResponse::ok(page)))
}
