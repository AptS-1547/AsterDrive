use crate::api::middleware::auth::JwtAuth;
use crate::api::response::ApiResponse;
use crate::errors::Result;
use crate::runtime::AppState;
use crate::services::{auth_service::Claims, config_service, policy_service, user_service};
use crate::types::{DriverType, UserRole, UserStatus};
use actix_web::{HttpResponse, web};
use serde::Deserialize;
use utoipa::ToSchema;

pub fn routes() -> impl actix_web::dev::HttpServiceFactory {
    web::scope("/admin")
        .wrap(JwtAuth)
        // policies
        .route("/policies", web::get().to(list_policies))
        .route("/policies", web::post().to(create_policy))
        .route("/policies/{id}", web::get().to(get_policy))
        .route("/policies/{id}", web::patch().to(update_policy))
        .route("/policies/{id}", web::delete().to(delete_policy))
        // users
        .route("/users", web::get().to(list_users))
        .route("/users/{id}", web::get().to(get_user))
        .route("/users/{id}", web::patch().to(update_user))
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
        // config
        .route("/config", web::get().to(list_config))
        .route("/config/{key}", web::get().to(get_config))
        .route("/config/{key}", web::put().to(set_config))
        .route("/config/{key}", web::delete().to(delete_config))
}

// ── Policies ─────────────────────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/v1/admin/policies",
    tag = "admin",
    operation_id = "list_policies",
    responses(
        (status = 200, description = "List all storage policies", body = inline(ApiResponse<Vec<crate::entities::storage_policy::Model>>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer" = [])),
)]
pub async fn list_policies(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    require_admin(&claims)?;
    let policies = policy_service::list_all(&state.db).await?;
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
    pub is_default: Option<bool>,
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
        &state.db,
        &body.name,
        body.driver_type,
        body.endpoint.as_deref().unwrap_or_default(),
        body.bucket.as_deref().unwrap_or_default(),
        body.access_key.as_deref().unwrap_or_default(),
        body.secret_key.as_deref().unwrap_or_default(),
        body.base_path.as_deref().unwrap_or_default(),
        body.max_file_size.unwrap_or(0),
        body.is_default.unwrap_or(false),
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
    let policy = policy_service::get(&state.db, *path).await?;
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
    pub is_default: Option<bool>,
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
        &state.db,
        *path,
        body.name,
        body.endpoint,
        body.bucket,
        body.access_key,
        body.secret_key,
        body.base_path,
        body.max_file_size,
        body.is_default,
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
    policy_service::delete(&state.db, *path).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::<()>::ok_empty()))
}

// ── Users ────────────────────────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/v1/admin/users",
    tag = "admin",
    operation_id = "list_users",
    responses(
        (status = 200, description = "List all users", body = inline(ApiResponse<Vec<crate::entities::user::Model>>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer" = [])),
)]
pub async fn list_users(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    require_admin(&claims)?;
    let users = user_service::list_all(&state.db).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(users)))
}

#[utoipa::path(
    get,
    path = "/api/v1/admin/users/{id}",
    tag = "admin",
    operation_id = "get_user",
    params(("id" = i64, Path, description = "User ID")),
    responses(
        (status = 200, description = "User details", body = inline(ApiResponse<crate::entities::user::Model>)),
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
    let user = user_service::get(&state.db, *path).await?;
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
        (status = 200, description = "User updated", body = inline(ApiResponse<crate::entities::user::Model>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "User not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn update_user(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<i64>,
    body: web::Json<PatchUserReq>,
) -> Result<HttpResponse> {
    require_admin(&claims)?;
    let target_id = *path;
    let body = body.into_inner();

    // 禁止降级或禁用初始管理员 (id=1)
    if target_id == 1 {
        if let Some(ref status) = body.status
            && !status.is_active()
        {
            return Err(crate::errors::AsterError::validation_error(
                "cannot disable the initial admin account",
            ));
        }
        if let Some(ref role) = body.role
            && !role.is_admin()
        {
            return Err(crate::errors::AsterError::validation_error(
                "cannot demote the initial admin account",
            ));
        }
    }

    let user = user_service::update(
        &state.db,
        target_id,
        body.role,
        body.status,
        body.storage_quota,
    )
    .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(user)))
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
    params(("user_id" = i64, Path, description = "User ID")),
    responses(
        (status = 200, description = "User policy assignments", body = inline(ApiResponse<Vec<crate::entities::user_storage_policy::Model>>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer" = [])),
)]
pub async fn list_user_policies(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<UserPolicyPath>,
) -> Result<HttpResponse> {
    require_admin(&claims)?;
    let policies = policy_service::list_user_policies(&state.db, path.user_id).await?;
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
        &state.db,
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
        policy_service::update_user_policy(&state.db, path.id, body.is_default, body.quota_bytes)
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
    policy_service::remove_user_policy(&state.db, path.id).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::<()>::ok_empty()))
}

// ── System Config ────────────────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/v1/admin/config",
    tag = "admin",
    operation_id = "list_config",
    responses(
        (status = 200, description = "List all config entries", body = inline(ApiResponse<Vec<crate::entities::system_config::Model>>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer" = [])),
)]
pub async fn list_config(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    require_admin(&claims)?;
    let configs = config_service::list_all(&state.db).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(configs)))
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
    let config = config_service::get_by_key(&state.db, &path).await?;
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
    path: web::Path<String>,
    body: web::Json<SetConfigReq>,
) -> Result<HttpResponse> {
    require_admin(&claims)?;
    let config = config_service::set(&state.db, &path, &body.value, claims.user_id).await?;
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
    config_service::delete(&state.db, &path).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::<()>::ok_empty()))
}

// ── Helpers ──────────────────────────────────────────────────────────

fn require_admin(claims: &Claims) -> Result<()> {
    use crate::errors::AsterError;
    if !claims.role.is_admin() {
        return Err(AsterError::auth_forbidden("admin role required"));
    }
    Ok(())
}
