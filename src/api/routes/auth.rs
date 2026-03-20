use crate::api::response::{ApiResponse, RefreshResponse, TokenResponse};
use crate::errors::Result;
use crate::runtime::AppState;
use crate::services::auth_service;
use actix_governor::{Governor, GovernorConfigBuilder};
use actix_web::{HttpResponse, web};
use serde::Deserialize;
use utoipa::ToSchema;

pub fn routes() -> actix_web::Scope {
    let login_limiter = GovernorConfigBuilder::default()
        .seconds_per_request(1)
        .burst_size(5)
        .finish()
        .unwrap();

    let register_limiter = GovernorConfigBuilder::default()
        .seconds_per_request(1)
        .burst_size(3)
        .finish()
        .unwrap();

    web::scope("/auth")
        .service(
            web::resource("/register")
                .wrap(Governor::new(&register_limiter))
                .route(web::post().to(register)),
        )
        .service(
            web::resource("/login")
                .wrap(Governor::new(&login_limiter))
                .route(web::post().to(login)),
        )
        .route("/refresh", web::post().to(refresh))
}

#[derive(Deserialize, ToSchema)]
pub struct RegisterReq {
    pub username: String,
    pub email: String,
    pub password: String,
}

#[derive(Deserialize, ToSchema)]
pub struct LoginReq {
    pub username: String,
    pub password: String,
}

#[derive(Deserialize, ToSchema)]
pub struct RefreshReq {
    pub refresh_token: String,
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/register",
    tag = "auth",
    operation_id = "register",
    request_body = RegisterReq,
    responses(
        (status = 201, description = "Registration successful", body = inline(ApiResponse<crate::entities::user::Model>)),
        (status = 400, description = "Validation error"),
    ),
)]
pub async fn register(
    state: web::Data<AppState>,
    body: web::Json<RegisterReq>,
) -> Result<HttpResponse> {
    let user = auth_service::register(
        &state.db,
        &body.username,
        &body.email,
        &body.password,
        &state.config.auth.jwt_secret,
    )
    .await?;
    Ok(HttpResponse::Created().json(ApiResponse::ok(user)))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/login",
    tag = "auth",
    operation_id = "login",
    request_body = LoginReq,
    responses(
        (status = 200, description = "Login successful", body = inline(ApiResponse<TokenResponse>)),
        (status = 401, description = "Invalid credentials"),
    ),
)]
pub async fn login(state: web::Data<AppState>, body: web::Json<LoginReq>) -> Result<HttpResponse> {
    let tokens = auth_service::login(
        &state.db,
        &body.username,
        &body.password,
        &state.config.auth,
    )
    .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(TokenResponse {
        access_token: tokens.0,
        refresh_token: tokens.1,
    })))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/refresh",
    tag = "auth",
    operation_id = "refresh",
    request_body = RefreshReq,
    responses(
        (status = 200, description = "Token refreshed", body = inline(ApiResponse<RefreshResponse>)),
        (status = 401, description = "Invalid refresh token"),
    ),
)]
pub async fn refresh(
    state: web::Data<AppState>,
    body: web::Json<RefreshReq>,
) -> Result<HttpResponse> {
    let access = auth_service::refresh_token(&body.refresh_token, &state.config.auth)?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(RefreshResponse {
        access_token: access,
    })))
}
