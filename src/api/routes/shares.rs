use crate::api::middleware::auth::JwtAuth;
use crate::api::response::ApiResponse;
use crate::errors::Result;
use crate::runtime::AppState;
use crate::services::{
    audit_service::{self, AuditContext},
    auth_service::Claims,
    share_service,
};
use actix_web::{HttpRequest, HttpResponse, web};
use serde::Deserialize;
use utoipa::ToSchema;

pub fn routes() -> impl actix_web::dev::HttpServiceFactory {
    web::scope("/shares")
        .wrap(JwtAuth)
        .route("", web::post().to(create_share))
        .route("", web::get().to(list_shares))
        .route("/{id}", web::delete().to(delete_share))
}

#[derive(Deserialize, ToSchema)]
pub struct CreateShareReq {
    pub file_id: Option<i64>,
    pub folder_id: Option<i64>,
    pub password: Option<String>,
    #[schema(value_type = Option<String>)]
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    pub max_downloads: i64,
}

#[utoipa::path(
    post,
    path = "/api/v1/shares",
    tag = "shares",
    operation_id = "create_share",
    request_body = CreateShareReq,
    responses(
        (status = 201, description = "Share created", body = inline(ApiResponse<crate::entities::share::Model>)),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer" = [])),
)]
pub async fn create_share(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    req: HttpRequest,
    body: web::Json<CreateShareReq>,
) -> Result<HttpResponse> {
    let share = share_service::create_share(
        &state,
        claims.user_id,
        body.file_id,
        body.folder_id,
        body.password.clone(),
        body.expires_at,
        body.max_downloads,
    )
    .await?;
    let ctx = AuditContext::from_request(&req, &claims);
    audit_service::log(
        &state,
        &ctx,
        "share_create",
        None,
        Some(share.id),
        None,
        None,
    )
    .await;
    Ok(HttpResponse::Created().json(ApiResponse::ok(share)))
}

#[utoipa::path(
    get,
    path = "/api/v1/shares",
    tag = "shares",
    operation_id = "list_my_shares",
    responses(
        (status = 200, description = "My shares", body = inline(ApiResponse<Vec<crate::entities::share::Model>>)),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer" = [])),
)]
pub async fn list_shares(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    let shares = share_service::list_my_shares(&state, claims.user_id).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(shares)))
}

#[utoipa::path(
    delete,
    path = "/api/v1/shares/{id}",
    tag = "shares",
    operation_id = "delete_share",
    params(("id" = i64, Path, description = "Share ID")),
    responses(
        (status = 200, description = "Share deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Share not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn delete_share(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    req: HttpRequest,
    path: web::Path<i64>,
) -> Result<HttpResponse> {
    let share_id = *path;
    share_service::delete_share(&state, share_id, claims.user_id).await?;
    let ctx = AuditContext::from_request(&req, &claims);
    audit_service::log(
        &state,
        &ctx,
        "share_delete",
        None,
        Some(share_id),
        None,
        None,
    )
    .await;
    Ok(HttpResponse::Ok().json(ApiResponse::<()>::ok_empty()))
}
