use crate::api::middleware::auth::JwtAuth;
use crate::api::response::ApiResponse;
use crate::errors::Result;
use crate::runtime::AppState;
use crate::services::{auth_service::Claims, policy_service};
use crate::types::DriverType;
use actix_web::{HttpResponse, web};
use serde::Deserialize;
use utoipa::ToSchema;

pub fn routes() -> impl actix_web::dev::HttpServiceFactory {
    web::scope("/admin")
        .wrap(JwtAuth)
        .route("/policies", web::get().to(list_policies))
        .route("/policies", web::post().to(create_policy))
        .route("/policies/{id}", web::get().to(get_policy))
        .route("/policies/{id}", web::delete().to(delete_policy))
}

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

fn require_admin(claims: &Claims) -> Result<()> {
    use crate::errors::AsterError;
    if !claims.role.is_admin() {
        return Err(AsterError::auth_forbidden("admin role required"));
    }
    Ok(())
}
