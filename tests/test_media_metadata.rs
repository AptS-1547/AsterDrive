//! 集成测试：`media_metadata`。

#[macro_use]
mod common;

use actix_web::test;
use aster_drive::db::repository::{
    background_task_repo, config_repo, file_repo, media_metadata_repo,
};
use aster_drive::entities::{file, file_blob};
use aster_drive::types::{
    BackgroundTaskKind, BackgroundTaskStatus, FileCategory, SystemConfigSource,
    SystemConfigValueType,
};
use base64::Engine;
use sea_orm::{ActiveModelTrait, Set};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

fn tiny_png() -> Vec<u8> {
    let mut buf = std::io::Cursor::new(Vec::new());
    let encoder = image::codecs::png::PngEncoder::new(&mut buf);
    image::ImageEncoder::write_image(encoder, &[255, 0, 0], 1, 1, image::ExtendedColorType::Rgb8)
        .unwrap();
    buf.into_inner()
}

fn tiny_mp4() -> Vec<u8> {
    base64::engine::general_purpose::STANDARD
        .decode("AAAAIGZ0eXBpc29tAAACAGlzb21pc28yYXZjMW1wNDEAAAN1bW9vdgAAAGxtdmhkAAAAAAAAAAAAAAAAAAAD6AAAAMgAAQAAAQAAAAAAAAAAAAAAAAEAAAAAAAAAAAAAAAAAAAABAAAAAAAAAAAAAAAAAABAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAgAAAp90cmFrAAAAXHRraGQAAAADAAAAAAAAAAAAAAABAAAAAAAAAMgAAAAAAAAAAAAAAAAAAAAAAAEAAAAAAAAAAAAAAAAAAAABAAAAAAAAAAAAAAAAAABAAAAAABAAAAAQAAAAAAAkZWR0cwAAABxlbHN0AAAAAAAAAAEAAADIAAAEAAABAAAAAAIXbWRpYQAAACBtZGhkAAAAAAAAAAAAAAAAAAAyAAAACgBVxAAAAAAALWhkbHIAAAAAAAAAAHZpZGUAAAAAAAAAAAAAAABWaWRlb0hhbmRsZXIAAAABwm1pbmYAAAAUdm1oZAAAAAEAAAAAAAAAAAAAACRkaW5mAAAAHGRyZWYAAAAAAAAAAQAAAAx1cmwgAAAAAQAAAYJzdGJsAAAAvnN0c2QAAAAAAAAAAQAAAK5hdmMxAAAAAAAAAAEAAAAAAAAAAAAAAAAAAAAAABAAEABIAAAASAAAAAAAAAABFUxhdmM2Mi4yOC4xMDAgbGlieDI2NAAAAAAAAAAAAAAAGP//AAAANGF2Y0MBZAAK/+EAF2dkAAqs2V7ARAAAAwAEAAADAMg8SJZYAQAGaOvjyyLA/fj4AAAAABBwYXNwAAAAAQAAAAEAAAAUYnRydAAAAAAAAHcQAAAAAAAAABhzdHRzAAAAAAAAAAEAAAAFAAACAAAAABRzdHNzAAAAAAAAAAEAAAABAAAAOGN0dHMAAAAAAAAABQAAAAEAAAQAAAAAAQAACgAAAAABAAAEAAAAAAEAAAAAAAAAAQAAAgAAAAAcc3RzYwAAAAAAAAABAAAAAQAAAAUAAAABAAAAKHN0c3oAAAAAAAAAAAAAAAUAAALKAAAADAAAAAwAAAAMAAAADAAAABRzdGNvAAAAAAAAAAEAAAOlAAAAYnVkdGEAAABabWV0YQAAAAAAAAAhaGRscgAAAAAAAAAAbWRpcmFwcGwAAAAAAAAAAAAAAAAtaWxzdAAAACWpdG9vAAAAHWRhdGEAAAABAAAAAExhdmY2Mi4xMi4xMDAAAAAIZnJlZQAAAwJtZGF0AAACrgYF//+q3EXpvebZSLeWLNgg2SPu73gyNjQgLSBjb3JlIDE2NSByMzIyMiBiMzU2MDVhIC0gSC4yNjQvTVBFRy00IEFWQyBjb2RlYyAtIENvcHlsZWZ0IDIwMDMtMjAyNSAtIGh0dHA6Ly93d3cudmlkZW9sYW4ub3JnL3gyNjQuaHRtbCAtIG9wdGlvbnM6IGNhYmFjPTEgcmVmPTMgZGVibG9jaz0xOjA6MCBhbmFseXNlPTB4MzoweDExMyBtZT1oZXggc3VibWU9NyBwc3k9MSBwc3lfcmQ9MS4wMDowLjAwIG1peGVkX3JlZj0xIG1lX3JhbmdlPTE2IGNocm9tYV9tZT0xIHRyZWxsaXM9MSA4eDhkY3Q9MSBjcW09MCBkZWFkem9uZT0yMSwxMSBmYXN0X3Bza2lwPTEgY2hyb21hX3FwX29mZnNldD0tMiB0aHJlYWRzPTEgbG9va2FoZWFkX3RocmVhZHM9MSBzbGljZWRfdGhyZWFkcz0wIG5yPTAgZGVjaW1hdGU9MSBpbnRlcmxhY2VkPTAgYmx1cmF5X2NvbXBhdD0wIGNvbnN0cmFpbmVkX2ludHJhPTAgYmZyYW1lcz0zIGJfcHlyYW1pZD0yIGJfYWRhcHQ9MSBiX2JpYXM9MCBkaXJlY3Q9MSB3ZWlnaHRiPTEgb3Blbl9nb3A9MCB3ZWlnaHRwPTIga2V5aW50PTI1MCBrZXlpbnRfbWluPTI1IHNjZW5lY3V0PTQwIGludHJhX3JlZnJlc2g9MCByY19sb29rYWhlYWQ9NDAgcmM9Y3JmIG1idHJlZT0xIGNyZj0yMy4wIHFjb21wPTAuNjAgcXBtaW49MCBxcG1heD02OSBxcHN0ZXA9NCBpcF9yYXRpbz0xLjQwIGFxPTE6MS4wMACAAAAAFGWIhAAz//7fMvgUzcWJzsyAXJ6XAAAACEGaJGxCv/7AAAAACEGeQniF/8GBAAAACAGeYXRCv8SAAAAACAGeY2pCv8SB")
        .expect("embedded tiny mp4 fixture should decode")
}

#[cfg(unix)]
fn write_fake_ffprobe_metadata_command() -> std::path::PathBuf {
    let dir =
        std::env::temp_dir().join(format!("aster-drive-ffprobe-meta-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("fake-ffprobe");
    std::fs::write(
        &path,
        "#!/bin/sh\ncat <<'JSON'\n{\"streams\":[{\"codec_type\":\"video\",\"codec_name\":\"h264\",\"width\":32,\"height\":18,\"duration\":\"2.500000\",\"avg_frame_rate\":\"25/1\"}],\"format\":{\"format_name\":\"mov,mp4,m4a,3gp,3g2,mj2\",\"duration\":\"2.500000\"}}\nJSON\n",
    )
    .unwrap();
    let mut permissions = std::fs::metadata(&path).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&path, permissions).unwrap();
    path
}

async fn upload_file_bytes(
    app: &impl actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
    token: &str,
    filename: &str,
    content_type: &str,
    bytes: &[u8],
) -> i64 {
    let boundary = "----MediaMetadataBoundary";
    let mut payload = Vec::new();
    payload.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    payload.extend_from_slice(
        format!("Content-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\n")
            .as_bytes(),
    );
    payload.extend_from_slice(format!("Content-Type: {content_type}\r\n\r\n").as_bytes());
    payload.extend_from_slice(bytes);
    payload.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

    let req = test::TestRequest::post()
        .uri("/api/v1/files/upload")
        .insert_header(("Cookie", common::access_cookie_header(token)))
        .insert_header(common::csrf_header_for(token))
        .insert_header((
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        ))
        .set_payload(payload)
        .to_request();
    let resp = test::call_service(app, req).await;
    assert_eq!(resp.status(), 201, "upload should return 201");
    let body: Value = test::read_body_json(resp).await;
    body["data"]["id"].as_i64().unwrap()
}

async fn set_system_config(state: &aster_drive::runtime::PrimaryAppState, key: &str, value: &str) {
    config_repo::upsert(&state.db, key, value, 1).await.unwrap();
    let mut model = config_repo::find_by_key(&state.db, key)
        .await
        .unwrap()
        .unwrap();
    model.source = SystemConfigSource::System;
    model.value_type = if value == "true" || value == "false" {
        SystemConfigValueType::Boolean
    } else {
        SystemConfigValueType::String
    };
    state.runtime_config.apply(model);
}

async fn set_media_processing_registry(
    state: &aster_drive::runtime::PrimaryAppState,
    value: serde_json::Value,
) {
    set_system_config(
        state,
        "media_processing_registry_json",
        &serde_json::to_string_pretty(&value).unwrap(),
    )
    .await;
}

async fn insert_synthetic_media_file(
    state: &aster_drive::runtime::PrimaryAppState,
    name: &str,
    mime_type: &str,
    category: FileCategory,
    bytes: &[u8],
) -> i64 {
    let policy = aster_drive::db::repository::policy_repo::find_default(&state.db)
        .await
        .unwrap()
        .expect("default policy should exist");
    let driver = state.driver_registry.get_driver(&policy).unwrap();
    let hash = hex::encode(Sha256::digest(bytes));
    let storage_path = aster_drive::utils::storage_path_from_blob_key(&hash);
    driver.put(&storage_path, bytes).await.unwrap();
    let now = chrono::Utc::now();
    let blob = file_blob::ActiveModel {
        hash: Set(hash),
        size: Set(bytes.len() as i64),
        policy_id: Set(policy.id),
        storage_path: Set(storage_path),
        ref_count: Set(1),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    }
    .insert(&state.db)
    .await
    .unwrap();
    file::ActiveModel {
        name: Set(name.to_string()),
        folder_id: Set(None),
        team_id: Set(None),
        blob_id: Set(blob.id),
        size: Set(bytes.len() as i64),
        owner_user_id: Set(Some(1)),
        created_by_user_id: Set(Some(1)),
        created_by_username: Set("testuser".to_string()),
        mime_type: Set(mime_type.to_string()),
        extension: Set(String::new()),
        compound_extension: Set(None),
        file_category: Set(category),
        created_at: Set(now),
        updated_at: Set(now),
        deleted_at: Set(None),
        is_locked: Set(false),
        ..Default::default()
    }
    .insert(&state.db)
    .await
    .unwrap()
    .id
}

#[actix_web::test]
async fn file_media_metadata_extracts_image_and_reuses_blob_cache() {
    let state = common::setup().await;
    let app = create_test_app!(state.clone());
    let (token, _) = register_and_login!(app);
    let file_id = upload_file_bytes(&app, &token, "cover.png", "image/png", &tiny_png()).await;

    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/files/{file_id}/media-metadata"))
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 202);
    assert_eq!(
        resp.headers()
            .get("Retry-After")
            .and_then(|value| value.to_str().ok()),
        Some("2")
    );

    let task = background_task_repo::list_recent(&state.db, 16)
        .await
        .unwrap()
        .into_iter()
        .find(|task| task.kind == BackgroundTaskKind::MediaMetadataExtract)
        .expect("media metadata task should be queued");
    assert_eq!(task.status, BackgroundTaskStatus::Pending);
    let payload: Value = serde_json::from_str(task.payload_json.as_ref()).unwrap();
    assert_eq!(payload["kind"], "image");

    let stats = aster_drive::services::task_service::drain(&state)
        .await
        .expect("task drain should succeed");
    assert_eq!(stats.succeeded, 1);

    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/files/{file_id}/media-metadata"))
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["kind"], "image");
    assert_eq!(body["data"]["status"], "ready");
    assert_eq!(body["data"]["metadata"]["kind"], "image");
    assert_eq!(body["data"]["metadata"]["width"], 1);
    assert_eq!(body["data"]["metadata"]["height"], 1);

    assert_eq!(
        background_task_repo::list_recent(&state.db, 16)
            .await
            .unwrap()
            .into_iter()
            .filter(|task| task.kind == BackgroundTaskKind::MediaMetadataExtract)
            .count(),
        1
    );
}

#[actix_web::test]
async fn file_media_metadata_returns_unsupported_for_video_when_ffprobe_processor_is_disabled() {
    let state = common::setup().await;
    let app = create_test_app!(state.clone());
    let (token, _) = register_and_login!(app);
    let file_id = insert_synthetic_media_file(
        &state,
        "clip.mp4",
        "video/mp4",
        FileCategory::Video,
        &tiny_mp4(),
    )
    .await;

    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/files/{file_id}/media-metadata"))
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["kind"], "video");
    assert_eq!(body["data"]["status"], "unsupported");
    assert_eq!(body["data"]["parser"], "unsupported");
    assert!(body["data"]["metadata"].is_null());

    let file = file_repo::find_by_id(&state.db, file_id).await.unwrap();
    let record = media_metadata_repo::find_by_blob_id(&state.db, file.blob_id)
        .await
        .unwrap();
    assert!(record.is_none());
    assert_eq!(
        background_task_repo::list_recent(&state.db, 16)
            .await
            .unwrap()
            .into_iter()
            .filter(|task| task.kind == BackgroundTaskKind::MediaMetadataExtract)
            .count(),
        0
    );
}

#[actix_web::test]
async fn share_media_metadata_uses_same_pending_response_shape() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);
    let file_id = upload_file_bytes(&app, &token, "shared.png", "image/png", &tiny_png()).await;

    let req = test::TestRequest::post()
        .uri("/api/v1/shares")
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .set_json(json!({
            "target": {
                "type": "file",
                "id": file_id
            }
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let share_token = body["data"]["token"].as_str().unwrap();

    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/s/{share_token}/media-metadata"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 202);
    assert_eq!(
        resp.headers()
            .get("Retry-After")
            .and_then(|value| value.to_str().ok()),
        Some("2")
    );
}

#[actix_web::test]
async fn media_metadata_disabled_returns_unsupported_without_task() {
    let state = common::setup().await;
    let app = create_test_app!(state.clone());
    let (token, _) = register_and_login!(app);
    let req = test::TestRequest::put()
        .uri("/api/v1/admin/config/media_metadata_enabled")
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .set_json(json!({ "value": "false" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let file_id = upload_file_bytes(&app, &token, "disabled.png", "image/png", &tiny_png()).await;
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/files/{file_id}/media-metadata"))
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["kind"], "image");
    assert_eq!(body["data"]["status"], "unsupported");
    assert_eq!(body["data"]["parser"], "disabled");

    assert_eq!(
        background_task_repo::list_recent(&state.db, 16)
            .await
            .unwrap()
            .into_iter()
            .filter(|task| task.kind == BackgroundTaskKind::MediaMetadataExtract)
            .count(),
        0
    );
}

#[actix_web::test]
async fn media_metadata_processor_disabled_returns_unsupported_without_task() {
    let state = common::setup().await;
    set_media_processing_registry(
        &state,
        json!({
            "version": 2,
            "processors": [
                {
                    "kind": "images",
                    "enabled": false,
                    "uses": ["thumbnail:image", "metadata:image"]
                },
                {
                    "kind": "lofty",
                    "enabled": true,
                    "uses": ["thumbnail:audio", "metadata:audio"]
                }
            ]
        }),
    )
    .await;
    let app = create_test_app!(state.clone());
    let (token, _) = register_and_login!(app);
    let file_id =
        upload_file_bytes(&app, &token, "disabled-kind.png", "image/png", &tiny_png()).await;

    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/files/{file_id}/media-metadata"))
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["kind"], "image");
    assert_eq!(body["data"]["status"], "unsupported");
    assert_eq!(body["data"]["parser"], "unsupported");

    assert_eq!(
        background_task_repo::list_recent(&state.db, 16)
            .await
            .unwrap()
            .into_iter()
            .filter(|task| task.kind == BackgroundTaskKind::MediaMetadataExtract)
            .count(),
        0
    );
}

#[cfg(unix)]
#[actix_web::test]
async fn video_media_metadata_uses_configured_ffprobe_command() {
    let fake_ffprobe = write_fake_ffprobe_metadata_command();
    let state = common::setup().await;
    set_media_processing_registry(
        &state,
        json!({
            "version": 2,
            "processors": [
                {
                    "kind": "ffprobe_cli",
                    "enabled": true,
                    "uses": ["metadata:video"],
                    "extensions": ["mp4"],
                    "config": {
                        "command": fake_ffprobe.to_string_lossy()
                    }
                },
                {
                    "kind": "images",
                    "enabled": true,
                    "uses": ["thumbnail:image", "metadata:image"]
                },
                {
                    "kind": "lofty",
                    "enabled": true,
                    "uses": ["thumbnail:audio", "metadata:audio"]
                }
            ]
        }),
    )
    .await;
    let app = create_test_app!(state.clone());
    let (token, _) = register_and_login!(app);
    let file_id = insert_synthetic_media_file(
        &state,
        "configured-clip.mp4",
        "video/mp4",
        FileCategory::Video,
        &tiny_mp4(),
    )
    .await;

    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/files/{file_id}/media-metadata"))
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 202);

    let stats = aster_drive::services::task_service::drain(&state)
        .await
        .expect("task drain should succeed");
    assert_eq!(stats.succeeded, 1);

    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/files/{file_id}/media-metadata"))
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["status"], "ready");
    assert_eq!(body["data"]["parser"], "ffprobe");
    assert_eq!(body["data"]["metadata"]["width"], 32);
    assert_eq!(body["data"]["metadata"]["height"], 18);
    assert_eq!(body["data"]["metadata"]["duration_ms"], 2500);
    assert_eq!(body["data"]["metadata"]["frame_rate"], "25/1");

    let _ = std::fs::remove_dir_all(
        fake_ffprobe
            .parent()
            .expect("fake ffprobe script should have a parent directory"),
    );
}
