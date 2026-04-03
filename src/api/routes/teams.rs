use crate::api::middleware::auth::JwtAuth;
use crate::api::middleware::rate_limit;
use crate::api::response::ApiResponse;
use crate::api::routes::{team_batch, team_search, team_shares, team_space, team_trash};
use crate::config::RateLimitConfig;
use crate::errors::Result;
use crate::runtime::AppState;
use crate::services::{auth_service::Claims, team_service};
use crate::types::TeamMemberRole;
use actix_governor::Governor;
use actix_web::middleware::Condition;
use actix_web::{HttpResponse, web};
use serde::Deserialize;
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::ToSchema;

pub fn routes(rl: &RateLimitConfig) -> impl actix_web::dev::HttpServiceFactory + use<> {
    let limiter = rate_limit::build_governor(&rl.api);

    web::scope("/teams")
        .wrap(JwtAuth)
        .wrap(Condition::new(rl.enabled, Governor::new(&limiter)))
        .route("", web::get().to(list_teams))
        .route("", web::post().to(create_team))
        .route("/{id}", web::get().to(get_team))
        .route("/{id}", web::patch().to(patch_team))
        .route("/{id}", web::delete().to(delete_team))
        .route("/{id}/members", web::get().to(list_members))
        .route("/{id}/members", web::post().to(add_member))
        .route(
            "/{id}/members/{member_user_id}",
            web::patch().to(patch_member),
        )
        .route(
            "/{id}/members/{member_user_id}",
            web::delete().to(delete_member),
        )
        .service(team_batch::routes())
        .service(team_search::routes())
        .service(team_shares::routes())
        .service(team_trash::routes())
        .service(team_space::routes(rl))
}

#[derive(Debug, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct CreateTeamReq {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct PatchTeamReq {
    pub name: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct AddTeamMemberReq {
    pub user_id: Option<i64>,
    pub identifier: Option<String>,
    pub role: Option<TeamMemberRole>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct PatchTeamMemberReq {
    pub role: TeamMemberRole,
}

#[api_docs_macros::path(
    get,
    path = "/api/v1/teams",
    tag = "teams",
    operation_id = "list_teams",
    responses(
        (status = 200, description = "Teams visible to the signed-in user", body = inline(ApiResponse<Vec<team_service::TeamInfo>>)),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer" = [])),
)]
pub async fn list_teams(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    let teams = team_service::list_teams(&state, claims.user_id).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(teams)))
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/teams",
    tag = "teams",
    operation_id = "create_team",
    request_body = CreateTeamReq,
    responses(
        (status = 201, description = "Team created", body = inline(ApiResponse<team_service::TeamInfo>)),
        (status = 400, description = "Validation error"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer" = [])),
)]
pub async fn create_team(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    body: web::Json<CreateTeamReq>,
) -> Result<HttpResponse> {
    let team = team_service::create_team(
        &state,
        claims.user_id,
        team_service::CreateTeamInput {
            name: body.name.clone(),
            description: body.description.clone(),
        },
    )
    .await?;
    Ok(HttpResponse::Created().json(ApiResponse::ok(team)))
}

#[api_docs_macros::path(
    get,
    path = "/api/v1/teams/{id}",
    tag = "teams",
    operation_id = "get_team",
    params(("id" = i64, Path, description = "Team ID")),
    responses(
        (status = 200, description = "Team details", body = inline(ApiResponse<team_service::TeamInfo>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn get_team(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<i64>,
) -> Result<HttpResponse> {
    let team = team_service::get_team(&state, *path, claims.user_id).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(team)))
}

#[api_docs_macros::path(
    patch,
    path = "/api/v1/teams/{id}",
    tag = "teams",
    operation_id = "patch_team",
    params(("id" = i64, Path, description = "Team ID")),
    request_body = PatchTeamReq,
    responses(
        (status = 200, description = "Team updated", body = inline(ApiResponse<team_service::TeamInfo>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn patch_team(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<i64>,
    body: web::Json<PatchTeamReq>,
) -> Result<HttpResponse> {
    let team = team_service::update_team(
        &state,
        *path,
        claims.user_id,
        team_service::UpdateTeamInput {
            name: body.name.clone(),
            description: body.description.clone(),
        },
    )
    .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(team)))
}

#[api_docs_macros::path(
    delete,
    path = "/api/v1/teams/{id}",
    tag = "teams",
    operation_id = "delete_team",
    params(("id" = i64, Path, description = "Team ID")),
    responses(
        (status = 200, description = "Team archived"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn delete_team(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<i64>,
) -> Result<HttpResponse> {
    team_service::archive_team(&state, *path, claims.user_id).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::<()>::ok_empty()))
}

#[api_docs_macros::path(
    get,
    path = "/api/v1/teams/{id}/members",
    tag = "teams",
    operation_id = "list_team_members",
    params(("id" = i64, Path, description = "Team ID")),
    responses(
        (status = 200, description = "Team members", body = inline(ApiResponse<Vec<team_service::TeamMemberInfo>>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn list_members(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<i64>,
) -> Result<HttpResponse> {
    let members = team_service::list_members(&state, *path, claims.user_id).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(members)))
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/teams/{id}/members",
    tag = "teams",
    operation_id = "add_team_member",
    params(("id" = i64, Path, description = "Team ID")),
    request_body = AddTeamMemberReq,
    responses(
        (status = 201, description = "Member added", body = inline(ApiResponse<team_service::TeamMemberInfo>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn add_member(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<i64>,
    body: web::Json<AddTeamMemberReq>,
) -> Result<HttpResponse> {
    let member = team_service::add_member(
        &state,
        *path,
        claims.user_id,
        team_service::AddTeamMemberInput {
            user_id: body.user_id,
            identifier: body.identifier.clone(),
            role: body.role.unwrap_or(TeamMemberRole::Member),
        },
    )
    .await?;
    Ok(HttpResponse::Created().json(ApiResponse::ok(member)))
}

#[api_docs_macros::path(
    patch,
    path = "/api/v1/teams/{id}/members/{member_user_id}",
    tag = "teams",
    operation_id = "patch_team_member",
    params(
        ("id" = i64, Path, description = "Team ID"),
        ("member_user_id" = i64, Path, description = "Member user ID")
    ),
    request_body = PatchTeamMemberReq,
    responses(
        (status = 200, description = "Member updated", body = inline(ApiResponse<team_service::TeamMemberInfo>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn patch_member(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<(i64, i64)>,
    body: web::Json<PatchTeamMemberReq>,
) -> Result<HttpResponse> {
    let (team_id, member_user_id) = path.into_inner();
    let member = team_service::update_member_role(
        &state,
        team_id,
        claims.user_id,
        member_user_id,
        body.role,
    )
    .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(member)))
}

#[api_docs_macros::path(
    delete,
    path = "/api/v1/teams/{id}/members/{member_user_id}",
    tag = "teams",
    operation_id = "delete_team_member",
    params(
        ("id" = i64, Path, description = "Team ID"),
        ("member_user_id" = i64, Path, description = "Member user ID")
    ),
    responses(
        (status = 200, description = "Member removed"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn delete_member(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<(i64, i64)>,
) -> Result<HttpResponse> {
    let (team_id, member_user_id) = path.into_inner();
    team_service::remove_member(&state, team_id, claims.user_id, member_user_id).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::<()>::ok_empty()))
}
