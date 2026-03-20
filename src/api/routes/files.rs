use crate::api::middleware::auth::JwtAuth;
use crate::api::response::ApiResponse;
use crate::errors::Result;
use crate::runtime::AppState;
use crate::services::{auth_service::Claims, file_service};
use actix_web::{HttpResponse, web};
use serde::Deserialize;

pub fn routes() -> impl actix_web::dev::HttpServiceFactory {
    web::scope("/files")
        .wrap(JwtAuth)
        .route("/upload", web::post().to(upload))
        .route("/{id}", web::get().to(get_file))
        .route("/{id}/download", web::get().to(download))
        .route("/{id}", web::delete().to(delete_file))
        .route("/{id}", web::patch().to(patch_file))
}

#[derive(Deserialize)]
struct FileQuery {
    folder_id: Option<i64>,
}

async fn upload(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    query: web::Query<FileQuery>,
    mut payload: actix_multipart::Multipart,
) -> Result<HttpResponse> {
    let file = file_service::upload(
        &state.db,
        &state.driver_registry,
        claims.user_id,
        &mut payload,
        query.folder_id,
    )
    .await?;
    Ok(HttpResponse::Created().json(ApiResponse::ok(file)))
}

async fn get_file(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<i64>,
) -> Result<HttpResponse> {
    let file = file_service::get_info(&state.db, *path, claims.user_id).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(file)))
}

async fn download(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<i64>,
) -> Result<HttpResponse> {
    let response =
        file_service::download(&state.db, &state.driver_registry, *path, claims.user_id).await?;
    Ok(response)
}

async fn delete_file(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<i64>,
) -> Result<HttpResponse> {
    file_service::delete(&state.db, &state.driver_registry, *path, claims.user_id).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::<()>::ok_empty()))
}

#[derive(Deserialize)]
struct PatchFileReq {
    name: Option<String>,
    folder_id: Option<i64>,
}

async fn patch_file(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<i64>,
    body: web::Json<PatchFileReq>,
) -> Result<HttpResponse> {
    let file = file_service::update(
        &state.db,
        *path,
        claims.user_id,
        body.name.clone(),
        body.folder_id,
    )
    .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(file)))
}
