use crate::api::response::ApiResponse;
use crate::api::routes::team_scope;
use crate::errors::Result;
use crate::runtime::AppState;
#[cfg(all(debug_assertions, feature = "openapi"))]
use crate::services::search_service::SearchResults;
use crate::services::{auth_service::Claims, search_service, search_service::SearchParams};
use actix_web::{HttpResponse, web};

pub fn routes() -> impl actix_web::dev::HttpServiceFactory + use<> {
    web::scope("/{team_id}/search").route("", web::get().to(search))
}

#[api_docs_macros::path(
    get,
    path = "/api/v1/teams/{team_id}/search",
    tag = "teams",
    operation_id = "search_team_workspace",
    params(
        ("team_id" = i64, Path, description = "Team ID"),
        SearchParams
    ),
    responses(
        (status = 200, description = "Team workspace search results", body = inline(ApiResponse<SearchResults>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer" = [])),
)]
pub async fn search(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<i64>,
    query: web::Query<SearchParams>,
) -> Result<HttpResponse> {
    let query = query.into_inner();
    let results =
        search_service::search_in_scope(&state, team_scope(*path, claims.user_id), &query).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(results)))
}
