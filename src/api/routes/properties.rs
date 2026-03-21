use crate::api::middleware::auth::JwtAuth;
use crate::api::response::ApiResponse;
use crate::errors::Result;
use crate::runtime::AppState;
use crate::services::{auth_service::Claims, property_service};
use actix_web::{HttpResponse, web};
use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};

pub fn routes() -> impl actix_web::dev::HttpServiceFactory {
    web::scope("/properties")
        .wrap(JwtAuth)
        .route("/{entity_type}/{entity_id}", web::get().to(list_props))
        .route("/{entity_type}/{entity_id}", web::put().to(set_prop))
        .route(
            "/{entity_type}/{entity_id}/{namespace}/{name}",
            web::delete().to(delete_prop),
        )
}

#[derive(Deserialize, IntoParams)]
pub struct EntityPath {
    pub entity_type: String,
    pub entity_id: i64,
}

#[derive(Deserialize, IntoParams)]
pub struct PropPath {
    pub entity_type: String,
    pub entity_id: i64,
    pub namespace: String,
    pub name: String,
}

#[derive(Deserialize, ToSchema)]
pub struct SetPropReq {
    pub namespace: String,
    pub name: String,
    pub value: Option<String>,
}

#[utoipa::path(
    get,
    path = "/api/v1/properties/{entity_type}/{entity_id}",
    tag = "properties",
    operation_id = "list_properties",
    params(
        ("entity_type" = String, Path, description = "Entity type: 'file' or 'folder'"),
        ("entity_id" = i64, Path, description = "Entity ID"),
    ),
    responses(
        (status = 200, description = "Properties list", body = inline(ApiResponse<Vec<crate::entities::entity_property::Model>>)),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Entity not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn list_props(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<EntityPath>,
) -> Result<HttpResponse> {
    let props =
        property_service::list(&state, &path.entity_type, path.entity_id, claims.user_id).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(props)))
}

#[utoipa::path(
    put,
    path = "/api/v1/properties/{entity_type}/{entity_id}",
    tag = "properties",
    operation_id = "set_property",
    params(
        ("entity_type" = String, Path, description = "Entity type: 'file' or 'folder'"),
        ("entity_id" = i64, Path, description = "Entity ID"),
    ),
    request_body = SetPropReq,
    responses(
        (status = 200, description = "Property set", body = inline(ApiResponse<crate::entities::entity_property::Model>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "DAV: namespace is read-only"),
        (status = 404, description = "Entity not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn set_prop(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<EntityPath>,
    body: web::Json<SetPropReq>,
) -> Result<HttpResponse> {
    let prop = property_service::set(
        &state,
        &path.entity_type,
        path.entity_id,
        claims.user_id,
        &body.namespace,
        &body.name,
        body.value.as_deref(),
    )
    .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(prop)))
}

#[utoipa::path(
    delete,
    path = "/api/v1/properties/{entity_type}/{entity_id}/{namespace}/{name}",
    tag = "properties",
    operation_id = "delete_property",
    params(
        ("entity_type" = String, Path, description = "Entity type: 'file' or 'folder'"),
        ("entity_id" = i64, Path, description = "Entity ID"),
        ("namespace" = String, Path, description = "Property namespace"),
        ("name" = String, Path, description = "Property name"),
    ),
    responses(
        (status = 200, description = "Property deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "DAV: namespace is read-only"),
        (status = 404, description = "Entity not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn delete_prop(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<PropPath>,
) -> Result<HttpResponse> {
    property_service::delete(
        &state,
        &path.entity_type,
        path.entity_id,
        claims.user_id,
        &path.namespace,
        &path.name,
    )
    .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::<()>::ok_empty()))
}
