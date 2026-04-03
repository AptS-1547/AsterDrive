use super::common::deserialize_non_null_policy_group_id;
use crate::api::pagination::LimitOffsetQuery;
#[cfg(all(debug_assertions, feature = "openapi"))]
use crate::api::pagination::OffsetPage;
use crate::api::response::ApiResponse;
use crate::errors::Result;
use crate::runtime::AppState;
use crate::services::{audit_service, auth_service::Claims, team_service};
use actix_web::{HttpRequest, HttpResponse, web};
use serde::Deserialize;
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Deserialize)]
#[cfg_attr(
    all(debug_assertions, feature = "openapi"),
    derive(IntoParams, ToSchema)
)]
pub struct AdminTeamListQuery {
    pub keyword: Option<String>,
    pub archived: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct AdminCreateTeamReq {
    pub name: String,
    pub description: Option<String>,
    pub admin_user_id: Option<i64>,
    pub admin_identifier: Option<String>,
    pub policy_group_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct AdminPatchTeamReq {
    pub name: Option<String>,
    pub description: Option<String>,
    #[serde(default, deserialize_with = "deserialize_non_null_policy_group_id")]
    pub policy_group_id: Option<i64>,
}

fn admin_team_audit_details(team: &team_service::AdminTeamInfo) -> Option<serde_json::Value> {
    audit_service::details(audit_service::TeamAuditDetails {
        description: &team.description,
        member_count: team.member_count,
        storage_quota: team.storage_quota,
        policy_group_id: team.policy_group_id,
        archived_at: team.archived_at,
        actor_role: None,
    })
}

#[api_docs_macros::path(
    get,
    path = "/api/v1/admin/teams",
    tag = "admin",
    operation_id = "admin_list_teams",
    params(LimitOffsetQuery, AdminTeamListQuery),
    responses(
        (status = 200, description = "List active teams", body = inline(ApiResponse<OffsetPage<crate::services::team_service::AdminTeamInfo>>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer" = [])),
)]
pub async fn list_teams(
    state: web::Data<AppState>,
    page: web::Query<LimitOffsetQuery>,
    query: web::Query<AdminTeamListQuery>,
) -> Result<HttpResponse> {
    let teams = team_service::list_admin_teams(
        &state,
        page.limit_or(50, 100),
        page.offset(),
        query.keyword.as_deref(),
        query.archived.unwrap_or(false),
    )
    .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(teams)))
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/admin/teams",
    tag = "admin",
    operation_id = "admin_create_team",
    request_body = AdminCreateTeamReq,
    responses(
        (status = 201, description = "Team created", body = inline(ApiResponse<crate::services::team_service::AdminTeamInfo>)),
        (status = 400, description = "Validation error"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer" = [])),
)]
pub async fn create_team(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    req: HttpRequest,
    body: web::Json<AdminCreateTeamReq>,
) -> Result<HttpResponse> {
    let team = team_service::create_admin_team(
        &state,
        claims.user_id,
        team_service::AdminCreateTeamInput {
            name: body.name.clone(),
            description: body.description.clone(),
            admin_user_id: body.admin_user_id,
            admin_identifier: body.admin_identifier.clone(),
            policy_group_id: body.policy_group_id,
        },
    )
    .await?;
    let ctx = audit_service::AuditContext::from_request(&req, &claims);
    audit_service::log(
        &state,
        &ctx,
        audit_service::AuditAction::AdminCreateTeam,
        Some("team"),
        Some(team.id),
        Some(&team.name),
        admin_team_audit_details(&team),
    )
    .await;
    Ok(HttpResponse::Created().json(ApiResponse::ok(team)))
}

#[api_docs_macros::path(
    get,
    path = "/api/v1/admin/teams/{id}",
    tag = "admin",
    operation_id = "admin_get_team",
    params(("id" = i64, Path, description = "Team ID")),
    responses(
        (status = 200, description = "Team details", body = inline(ApiResponse<crate::services::team_service::AdminTeamInfo>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Team not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn get_team(state: web::Data<AppState>, path: web::Path<i64>) -> Result<HttpResponse> {
    let team = team_service::get_admin_team(&state, *path).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(team)))
}

#[api_docs_macros::path(
    patch,
    path = "/api/v1/admin/teams/{id}",
    tag = "admin",
    operation_id = "admin_update_team",
    params(("id" = i64, Path, description = "Team ID")),
    request_body = AdminPatchTeamReq,
    responses(
        (status = 200, description = "Team updated", body = inline(ApiResponse<crate::services::team_service::AdminTeamInfo>)),
        (status = 400, description = "Validation error"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Team not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn update_team(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    req: HttpRequest,
    path: web::Path<i64>,
    body: web::Json<AdminPatchTeamReq>,
) -> Result<HttpResponse> {
    let team = team_service::update_admin_team(
        &state,
        *path,
        team_service::AdminUpdateTeamInput {
            name: body.name.clone(),
            description: body.description.clone(),
            policy_group_id: body.policy_group_id,
        },
    )
    .await?;
    let ctx = audit_service::AuditContext::from_request(&req, &claims);
    audit_service::log(
        &state,
        &ctx,
        audit_service::AuditAction::AdminUpdateTeam,
        Some("team"),
        Some(team.id),
        Some(&team.name),
        admin_team_audit_details(&team),
    )
    .await;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(team)))
}

#[api_docs_macros::path(
    delete,
    path = "/api/v1/admin/teams/{id}",
    tag = "admin",
    operation_id = "admin_delete_team",
    params(("id" = i64, Path, description = "Team ID")),
    responses(
        (status = 200, description = "Team archived"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Team not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn delete_team(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    req: HttpRequest,
    path: web::Path<i64>,
) -> Result<HttpResponse> {
    let team = team_service::get_admin_team(&state, *path).await?;
    team_service::archive_admin_team(&state, *path).await?;
    let ctx = audit_service::AuditContext::from_request(&req, &claims);
    audit_service::log(
        &state,
        &ctx,
        audit_service::AuditAction::AdminArchiveTeam,
        Some("team"),
        Some(team.id),
        Some(&team.name),
        audit_service::details(audit_service::TeamAuditDetails {
            description: &team.description,
            member_count: team.member_count,
            storage_quota: team.storage_quota,
            policy_group_id: team.policy_group_id,
            archived_at: Some(chrono::Utc::now()),
            actor_role: None,
        }),
    )
    .await;
    Ok(HttpResponse::Ok().json(ApiResponse::<()>::ok_empty()))
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/admin/teams/{id}/restore",
    tag = "admin",
    operation_id = "admin_restore_team",
    params(("id" = i64, Path, description = "Team ID")),
    responses(
        (status = 200, description = "Team restored", body = inline(ApiResponse<crate::services::team_service::AdminTeamInfo>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Team not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn restore_team(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    req: HttpRequest,
    path: web::Path<i64>,
) -> Result<HttpResponse> {
    let team = team_service::restore_admin_team(&state, *path).await?;
    let ctx = audit_service::AuditContext::from_request(&req, &claims);
    audit_service::log(
        &state,
        &ctx,
        audit_service::AuditAction::AdminRestoreTeam,
        Some("team"),
        Some(team.id),
        Some(&team.name),
        admin_team_audit_details(&team),
    )
    .await;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(team)))
}
