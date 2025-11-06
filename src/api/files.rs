use actix_multipart::Multipart;
use actix_web::{web, HttpMessage, HttpRequest, HttpResponse};
use bytes::BytesMut;
use futures::StreamExt;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

use crate::{
    api::dto::{ErrorResponse, FileInfo, FileListResponse, FileUploadResponse},
    auth::Claims,
    models::{file, File},
};

use super::AppState;

/// Upload a file
pub async fn upload_file(
    state: web::Data<AppState>,
    req: HttpRequest,
    mut payload: Multipart,
) -> HttpResponse {
    // Extract claims from request extensions (set by middleware)
    let claims = match req.extensions().get::<Claims>() {
        Some(claims) => claims.clone(),
        None => {
            return HttpResponse::Unauthorized().json(ErrorResponse {
                error: "Unauthorized".to_string(),
            });
        }
    };

    let mut uploaded_file = None;

    // Process multipart stream
    while let Some(item) = payload.next().await {
        let mut field = match item {
            Ok(field) => field,
            Err(e) => {
                return HttpResponse::BadRequest().json(ErrorResponse {
                    error: format!("Failed to read multipart: {}", e),
                });
            }
        };

        // Get content disposition (actix-multipart 0.7+ returns Option)
        let content_disposition = match field.content_disposition() {
            Some(cd) => cd,
            None => continue,
        };

        let name = content_disposition.get_name().unwrap_or("");

        if name == "file" {
            let filename = content_disposition
                .get_filename()
                .unwrap_or("unnamed")
                .to_string();
            let content_type = field
                .content_type()
                .map(|ct| ct.to_string())
                .unwrap_or_else(|| "application/octet-stream".to_string());

            // Read all bytes from the field
            let mut bytes = BytesMut::new();
            while let Some(chunk) = field.next().await {
                let data = match chunk {
                    Ok(data) => data,
                    Err(e) => {
                        return HttpResponse::BadRequest().json(ErrorResponse {
                            error: format!("Failed to read file data: {}", e),
                        });
                    }
                };
                bytes.extend_from_slice(&data);
            }

            uploaded_file = Some((filename, content_type, bytes.freeze()));
            break;
        }
    }

    let (original_filename, mime_type, data) = match uploaded_file {
        Some(file) => file,
        None => {
            return HttpResponse::BadRequest().json(ErrorResponse {
                error: "No file provided".to_string(),
            });
        }
    };

    let size = data.len() as i64;
    let file_id = Uuid::new_v4().to_string();
    let storage_path = format!("{}/{}/{}", claims.sub, file_id, original_filename);

    // Store file
    if let Err(e) = state.storage.store(&storage_path, data).await {
        return HttpResponse::InternalServerError().json(ErrorResponse {
            error: format!("Failed to store file: {}", e),
        });
    }

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

    let file = match file_model.insert(&state.db).await {
        Ok(file) => file,
        Err(e) => {
            return HttpResponse::InternalServerError().json(ErrorResponse {
                error: format!("Failed to save file metadata: {}", e),
            });
        }
    };

    HttpResponse::Created().json(FileUploadResponse {
        id: file.id,
        filename: file.filename,
        size: file.size,
        mime_type,
    })
}

/// List user's files
pub async fn list_files(
    state: web::Data<AppState>,
    req: HttpRequest,
) -> HttpResponse {
    // Extract claims from request extensions
    let claims = match req.extensions().get::<Claims>() {
        Some(claims) => claims.clone(),
        None => {
            return HttpResponse::Unauthorized().json(ErrorResponse {
                error: "Unauthorized".to_string(),
            });
        }
    };

    let files = match File::find()
        .filter(file::Column::UserId.eq(claims.sub))
        .all(&state.db)
        .await
    {
        Ok(files) => files,
        Err(e) => {
            return HttpResponse::InternalServerError().json(ErrorResponse {
                error: e.to_string(),
            });
        }
    };

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

    HttpResponse::Ok().json(FileListResponse {
        files: file_list,
        total,
    })
}

/// Download a file
pub async fn download_file(
    state: web::Data<AppState>,
    req: HttpRequest,
    id: web::Path<i32>,
) -> HttpResponse {
    // Extract claims from request extensions
    let claims = match req.extensions().get::<Claims>() {
        Some(claims) => claims.clone(),
        None => {
            return HttpResponse::Unauthorized().json(ErrorResponse {
                error: "Unauthorized".to_string(),
            });
        }
    };

    let file = match File::find_by_id(*id).one(&state.db).await {
        Ok(Some(file)) => file,
        Ok(None) => {
            return HttpResponse::NotFound().json(ErrorResponse {
                error: "File not found".to_string(),
            });
        }
        Err(e) => {
            return HttpResponse::InternalServerError().json(ErrorResponse {
                error: e.to_string(),
            });
        }
    };

    // Check ownership
    if file.user_id != claims.sub && !file.is_public {
        return HttpResponse::Forbidden().json(ErrorResponse {
            error: "Access denied".to_string(),
        });
    }

    // Retrieve file from storage
    let data = match state.storage.retrieve(&file.storage_path).await {
        Ok(data) => data,
        Err(e) => {
            return HttpResponse::InternalServerError().json(ErrorResponse {
                error: format!("Failed to retrieve file: {}", e),
            });
        }
    };

    HttpResponse::Ok()
        .content_type(file.mime_type)
        .insert_header((
            "Content-Disposition",
            format!("attachment; filename=\"{}\"", file.original_filename),
        ))
        .body(data)
}

/// Delete a file
pub async fn delete_file(
    state: web::Data<AppState>,
    req: HttpRequest,
    id: web::Path<i32>,
) -> HttpResponse {
    // Extract claims from request extensions
    let claims = match req.extensions().get::<Claims>() {
        Some(claims) => claims.clone(),
        None => {
            return HttpResponse::Unauthorized().json(ErrorResponse {
                error: "Unauthorized".to_string(),
            });
        }
    };

    let file = match File::find_by_id(*id).one(&state.db).await {
        Ok(Some(file)) => file,
        Ok(None) => {
            return HttpResponse::NotFound().json(ErrorResponse {
                error: "File not found".to_string(),
            });
        }
        Err(e) => {
            return HttpResponse::InternalServerError().json(ErrorResponse {
                error: e.to_string(),
            });
        }
    };

    // Check ownership
    if file.user_id != claims.sub {
        return HttpResponse::Forbidden().json(ErrorResponse {
            error: "Access denied".to_string(),
        });
    }

    // Delete from storage
    if let Err(e) = state.storage.delete(&file.storage_path).await {
        return HttpResponse::InternalServerError().json(ErrorResponse {
            error: format!("Failed to delete file: {}", e),
        });
    }

    // Delete from database
    if let Err(e) = File::delete_by_id(*id).exec(&state.db).await {
        return HttpResponse::InternalServerError().json(ErrorResponse {
            error: e.to_string(),
        });
    }

    HttpResponse::NoContent().finish()
}
