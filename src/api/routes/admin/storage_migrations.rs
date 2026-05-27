//! 管理员 API 路由：`storage_migrations`。

use crate::api::dto::admin::CreateStoragePolicyMigrationReq;
use crate::api::dto::validate_request;
use crate::api::response::ApiResponse;
use crate::errors::Result;
use crate::runtime::PrimaryAppState;
use crate::services::{auth_service::Claims, task_service};
use actix_web::{HttpResponse, web};

#[api_docs_macros::path(
    post,
    path = "/api/v1/admin/storage-migrations",
    tag = "admin",
    operation_id = "create_storage_policy_migration",
    request_body = CreateStoragePolicyMigrationReq,
    responses(
        (status = 200, description = "Storage policy migration task created", body = inline(ApiResponse<task_service::TaskInfo>)),
        (status = 400, description = "Validation error"),
        (status = 401, description = crate::api::constants::OPENAPI_UNAUTHORIZED),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer" = [])),
)]
pub async fn create_storage_policy_migration(
    state: web::Data<PrimaryAppState>,
    claims: web::ReqData<Claims>,
    body: web::Json<CreateStoragePolicyMigrationReq>,
) -> Result<HttpResponse> {
    validate_request(&*body)?;
    let task = task_service::create_storage_policy_migration_task(
        &state,
        task_service::CreateStoragePolicyMigrationInput {
            source_policy_id: body.source_policy_id,
            target_policy_id: body.target_policy_id,
            delete_source_after_success: body.delete_source_after_success,
            creator_user_id: claims.user_id,
        },
    )
    .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(task)))
}
