use axum::{
    extract::{Multipart, Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Extension, Json,
};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

use crate::{
    api::dto::{ErrorResponse, FileInfo, FileListResponse, FileUploadResponse},
    auth::Claims,
    models::{file, File},
};

use super::AppState;

/// Upload a file
#[utoipa::path(
    post,
    path = "/api/files/upload",
    request_body(content = inline(Object), description = "Multipart form data with file", content_type = "multipart/form-data"),
    responses(
        (status = 201, description = "File uploaded successfully", body = FileUploadResponse),
        (status = 400, description = "Bad request", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "files"
)]
pub async fn upload_file(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
    let mut uploaded_file = None;

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("Failed to read multipart: {}", e),
            }),
        )
    })? {
        let name = field.name().unwrap_or("").to_string();
        
        if name == "file" {
            let filename = field
                .file_name()
                .unwrap_or("unnamed")
                .to_string();
            let content_type = field
                .content_type()
                .unwrap_or("application/octet-stream")
                .to_string();
            
            let data = field.bytes().await.map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: format!("Failed to read file data: {}", e),
                    }),
                )
            })?;

            uploaded_file = Some((filename, content_type, data));
            break;
        }
    }

    let (original_filename, mime_type, data) = uploaded_file.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "No file provided".to_string(),
            }),
        )
    })?;

    let size = data.len() as i64;
    let file_id = Uuid::new_v4().to_string();
    let storage_path = format!("{}/{}/{}", claims.sub, file_id, original_filename);

    // Store file
    state
        .storage
        .store(&storage_path, data)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to store file: {}", e),
                }),
            )
        })?;

    // Save metadata to database
    let file_model = file::ActiveModel {
        user_id: Set(claims.sub),
        filename: Set(file_id.clone()),
        original_filename: Set(original_filename),
        mime_type: Set(mime_type.clone()),
        size: Set(size),
        storage_path: Set(storage_path),
        storage_backend: Set(state.storage_backend.clone()),
        checksum: Set(None),
        is_public: Set(false),
        created_at: Set(chrono::Utc::now().into()),
        updated_at: Set(chrono::Utc::now().into()),
        ..Default::default()
    };

    let file = file_model.insert(&state.db).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to save file metadata: {}", e),
            }),
        )
    })?;

    Ok((
        StatusCode::CREATED,
        Json(FileUploadResponse {
            id: file.id,
            filename: file.filename,
            size: file.size,
            mime_type,
        }),
    ))
}

/// List user's files
#[utoipa::path(
    get,
    path = "/api/files",
    responses(
        (status = 200, description = "List of files", body = FileListResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "files"
)]
pub async fn list_files(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
    let files = File::find()
        .filter(file::Column::UserId.eq(claims.sub))
        .all(&state.db)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
        })?;

    let file_list: Vec<FileInfo> = files
        .into_iter()
        .map(|f| FileInfo {
            id: f.id,
            filename: f.filename,
            original_filename: f.original_filename,
            size: f.size,
            mime_type: f.mime_type,
            is_public: f.is_public,
            created_at: f.created_at.to_string(),
        })
        .collect();

    let total = file_list.len();

    Ok(Json(FileListResponse {
        files: file_list,
        total,
    }))
}

/// Download a file
#[utoipa::path(
    get,
    path = "/api/files/{id}",
    params(
        ("id" = i32, Path, description = "File ID")
    ),
    responses(
        (status = 200, description = "File content", content_type = "application/octet-stream"),
        (status = 404, description = "File not found", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "files"
)]
pub async fn download_file(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<i32>,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    let file = File::find_by_id(id)
        .one(&state.db)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "File not found".to_string(),
                }),
            )
        })?;

    // Check ownership
    if file.user_id != claims.sub && !file.is_public {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                error: "Access denied".to_string(),
            }),
        ));
    }

    // Retrieve file from storage
    let data = state
        .storage
        .retrieve(&file.storage_path)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to retrieve file: {}", e),
                }),
            )
        })?;

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, file.mime_type)
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", file.original_filename),
        )
        .body(axum::body::Body::from(data))
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to build response: {}", e),
                }),
            )
        })
}

/// Delete a file
#[utoipa::path(
    delete,
    path = "/api/files/{id}",
    params(
        ("id" = i32, Path, description = "File ID")
    ),
    responses(
        (status = 204, description = "File deleted successfully"),
        (status = 404, description = "File not found", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "files"
)]
pub async fn delete_file(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<i32>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let file = File::find_by_id(id)
        .one(&state.db)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "File not found".to_string(),
                }),
            )
        })?;

    // Check ownership
    if file.user_id != claims.sub {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                error: "Access denied".to_string(),
            }),
        ));
    }

    // Delete from storage
    state
        .storage
        .delete(&file.storage_path)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to delete file: {}", e),
                }),
            )
        })?;

    // Delete from database
    File::delete_by_id(id)
        .exec(&state.db)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
        })?;

    Ok(StatusCode::NO_CONTENT)
}
