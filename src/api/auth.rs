use actix_web::{web, HttpResponse};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};

use crate::{
    api::dto::{AuthResponse, ErrorResponse, LoginRequest, RegisterRequest, UserResponse},
    auth::{hash_password, verify_password},
    models::{user, User},
};

use super::AppState;

/// Register a new user
pub async fn register(
    state: web::Data<AppState>,
    payload: web::Json<RegisterRequest>,
) -> HttpResponse {
    // Check if user exists
    let existing_user = match User::find()
        .filter(user::Column::Username.eq(&payload.username))
        .one(&state.db)
        .await
    {
        Ok(user) => user,
        Err(e) => {
            return HttpResponse::InternalServerError().json(ErrorResponse {
                error: e.to_string(),
            });
        }
    };

    if existing_user.is_some() {
        return HttpResponse::Conflict().json(ErrorResponse {
            error: "Username already exists".to_string(),
        });
    }

    // Hash password
    let password_hash = match hash_password(&payload.password) {
        Ok(hash) => hash,
        Err(e) => {
            return HttpResponse::InternalServerError().json(ErrorResponse {
                error: e.to_string(),
            });
        }
    };

    // Create user
    let user = user::ActiveModel {
        username: Set(payload.username.clone()),
        email: Set(payload.email.clone()),
        password_hash: Set(password_hash),
        is_active: Set(true),
        created_at: Set(chrono::Utc::now().into()),
        updated_at: Set(chrono::Utc::now().into()),
        ..Default::default()
    };

    let user = match user.insert(&state.db).await {
        Ok(user) => user,
        Err(e) => {
            return HttpResponse::InternalServerError().json(ErrorResponse {
                error: e.to_string(),
            });
        }
    };

    // Generate token
    let token = match state.jwt_manager.create_token(user.id, user.username.clone()) {
        Ok(token) => token,
        Err(e) => {
            return HttpResponse::InternalServerError().json(ErrorResponse {
                error: e.to_string(),
            });
        }
    };

    HttpResponse::Created().json(AuthResponse {
        token,
        user: UserResponse {
            id: user.id,
            username: user.username,
            email: user.email,
            is_active: user.is_active,
        },
    })
}

/// Login user
pub async fn login(
    state: web::Data<AppState>,
    payload: web::Json<LoginRequest>,
) -> HttpResponse {
    // Find user
    let user = match User::find()
        .filter(user::Column::Username.eq(&payload.username))
        .one(&state.db)
        .await
    {
        Ok(Some(user)) => user,
        Ok(None) => {
            return HttpResponse::Unauthorized().json(ErrorResponse {
                error: "Invalid credentials".to_string(),
            });
        }
        Err(e) => {
            return HttpResponse::InternalServerError().json(ErrorResponse {
                error: e.to_string(),
            });
        }
    };

    // Verify password
    let valid = match verify_password(&payload.password, &user.password_hash) {
        Ok(valid) => valid,
        Err(e) => {
            return HttpResponse::InternalServerError().json(ErrorResponse {
                error: e.to_string(),
            });
        }
    };

    if !valid {
        return HttpResponse::Unauthorized().json(ErrorResponse {
            error: "Invalid credentials".to_string(),
        });
    }

    // Generate token
    let token = match state.jwt_manager.create_token(user.id, user.username.clone()) {
        Ok(token) => token,
        Err(e) => {
            return HttpResponse::InternalServerError().json(ErrorResponse {
                error: e.to_string(),
            });
        }
    };

    HttpResponse::Ok().json(AuthResponse {
        token,
        user: UserResponse {
            id: user.id,
            username: user.username,
            email: user.email,
            is_active: user.is_active,
        },
    })
}
