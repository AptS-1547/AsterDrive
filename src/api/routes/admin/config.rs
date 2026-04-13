use crate::api::pagination::LimitOffsetQuery;
#[cfg(all(debug_assertions, feature = "openapi"))]
use crate::api::pagination::OffsetPage;
use crate::api::response::ApiResponse;
use crate::errors::Result;
use crate::runtime::AppState;
use crate::services::{audit_service, auth_service::Claims, config_service};
use actix_web::{HttpRequest, HttpResponse, web};
use serde::Deserialize;
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::ToSchema;

#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct SetConfigReq {
    pub value: String,
}

#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct ExecuteConfigActionReq {
    pub action: config_service::ConfigActionType,
    pub discovery_url: Option<String>,
    pub target_email: Option<String>,
    pub value: Option<String>,
}

#[derive(serde::Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct ExecuteConfigActionResp {
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

#[api_docs_macros::path(
    get,
    path = "/api/v1/admin/config",
    tag = "admin",
    operation_id = "list_config",
    params(LimitOffsetQuery),
    responses(
        (status = 200, description = "List config entries", body = inline(ApiResponse<OffsetPage<config_service::SystemConfig>>)),
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
        (status = 200, description = "Config schema", body = ApiResponse<Vec<config_service::ConfigSchemaItem>>),
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
    path = "/api/v1/admin/config/template-variables",
    tag = "admin",
    operation_id = "config_template_variables",
    responses(
        (status = 200, description = "Template variables", body = ApiResponse<Vec<config_service::TemplateVariableGroup>>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer" = [])),
)]
pub async fn config_template_variables() -> Result<HttpResponse> {
    let groups = config_service::list_template_variable_groups();
    Ok(HttpResponse::Ok().json(ApiResponse::ok(groups)))
}

#[api_docs_macros::path(
    get,
    path = "/api/v1/admin/config/{key}",
    tag = "admin",
    operation_id = "get_config",
    params(("key" = String, Path, description = "Config key")),
    responses(
        (status = 200, description = "Config entry", body = inline(ApiResponse<config_service::SystemConfig>)),
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

#[api_docs_macros::path(
    put,
    path = "/api/v1/admin/config/{key}",
    tag = "admin",
    operation_id = "set_config",
    params(("key" = String, Path, description = "Config key")),
    request_body = SetConfigReq,
    responses(
        (status = 200, description = "Config value set", body = inline(ApiResponse<config_service::SystemConfig>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer" = [])),
)]
pub async fn set_config(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    req: HttpRequest,
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

#[api_docs_macros::path(
    post,
    path = "/api/v1/admin/config/{key}/action",
    tag = "admin",
    operation_id = "execute_config_action",
    params(("key" = String, Path, description = "Config action target key")),
    request_body = ExecuteConfigActionReq,
    responses(
        (status = 200, description = "Config action executed", body = inline(ApiResponse<ExecuteConfigActionResp>)),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Config action target not found"),
        (status = 503, description = "Mail service unavailable"),
    ),
    security(("bearer" = [])),
)]
pub async fn execute_config_action(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    req: HttpRequest,
    path: web::Path<String>,
    body: web::Json<ExecuteConfigActionReq>,
) -> Result<HttpResponse> {
    let key = path.into_inner();
    let action_result = config_service::execute_action(
        &state,
        &key,
        body.action,
        claims.user_id,
        body.target_email.as_deref(),
        body.value.as_deref(),
        body.discovery_url.as_deref(),
    )
    .await?;

    let ctx = audit_service::AuditContext::from_request(&req, &claims);
    audit_service::log(
        &state,
        &ctx,
        audit_service::AuditAction::ConfigActionExecute,
        None,
        None,
        Some(&key),
        audit_service::details(audit_service::ConfigActionDetails {
            action: body.action.as_str(),
            target_email: action_result.target_email.as_deref(),
        }),
    )
    .await;

    Ok(
        HttpResponse::Ok().json(ApiResponse::ok(ExecuteConfigActionResp {
            message: action_result.message,
            value: action_result.value,
        })),
    )
}
