#[macro_use]
mod common;

use actix_web::test;
use aster_drive::api::error_code::ErrorCode;
use aster_drive::db::repository::{background_task_repo, file_repo};
use aster_drive::runtime::AppState;
use aster_drive::types::{BackgroundTaskKind, BackgroundTaskStatus};
use serde_json::{Value, json};

/// 生成一个最小的 1x1 红色 PNG。
fn tiny_png() -> Vec<u8> {
    let mut buf = std::io::Cursor::new(Vec::new());
    let encoder = image::codecs::png::PngEncoder::new(&mut buf);
    image::ImageEncoder::write_image(encoder, &[255, 0, 0], 1, 1, image::ExtendedColorType::Rgb8)
        .unwrap();
    buf.into_inner()
}

macro_rules! upload_file_bytes {
    ($app:expr, $token:expr, $filename:expr, $content_type:expr, $bytes:expr) => {{
        let boundary = "----TestBound";
        let mut payload = Vec::new();
        payload.extend_from_slice(b"------TestBound\r\n");
        payload.extend_from_slice(
            format!(
                "Content-Disposition: form-data; name=\"file\"; filename=\"{}\"\r\n",
                $filename
            )
            .as_bytes(),
        );
        payload.extend_from_slice(format!("Content-Type: {}\r\n\r\n", $content_type).as_bytes());
        payload.extend_from_slice(&$bytes);
        payload.extend_from_slice(b"\r\n------TestBound--\r\n");

        let req = test::TestRequest::post()
            .uri("/api/v1/files/upload")
            .insert_header(("Cookie", common::access_cookie_header(&$token)))
            .insert_header(common::csrf_header_for(&$token))
            .insert_header((
                "Content-Type",
                format!("multipart/form-data; boundary={boundary}"),
            ))
            .set_payload(payload)
            .to_request();
        let resp: actix_web::dev::ServiceResponse = test::call_service(&$app, req).await;
        assert_eq!(resp.status(), 201, "upload should return 201");
        let body: Value = test::read_body_json(resp).await;
        body["data"]["id"].as_i64().unwrap()
    }};
}

macro_rules! request_thumbnail {
    ($app:expr, $token:expr, $file_id:expr) => {{
        let req = test::TestRequest::get()
            .uri(&format!("/api/v1/files/{}/thumbnail", $file_id))
            .insert_header(("Cookie", common::access_cookie_header(&$token)))
            .insert_header(common::csrf_header_for(&$token))
            .to_request();
        test::call_service(&$app, req).await
    }};
}

async fn thumbnail_task_display_name(state: &AppState, file_id: i64) -> String {
    let file = file_repo::find_by_id(&state.db, file_id).await.unwrap();
    format!("Generate thumbnail for blob #{}", file.blob_id)
}

async fn thumbnail_task_count(state: &AppState, file_id: i64) -> usize {
    let display_name = thumbnail_task_display_name(state, file_id).await;
    background_task_repo::list_recent(&state.db, 32)
        .await
        .unwrap()
        .into_iter()
        .filter(|task| {
            task.kind == BackgroundTaskKind::ThumbnailGenerate && task.display_name == display_name
        })
        .count()
}

async fn latest_thumbnail_task(
    state: &AppState,
    file_id: i64,
) -> aster_drive::entities::background_task::Model {
    let display_name = thumbnail_task_display_name(state, file_id).await;
    background_task_repo::find_latest_by_kind_and_display_name(
        &state.db,
        BackgroundTaskKind::ThumbnailGenerate,
        &display_name,
    )
    .await
    .unwrap()
    .expect("thumbnail task should exist")
}

#[actix_web::test]
async fn test_thumbnail_returns_202_when_not_ready() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let file_id = upload_file_bytes!(app, token, "test.png", "image/png", tiny_png());
    let resp = request_thumbnail!(app, token, file_id);

    assert_eq!(resp.status(), 202);
    assert_eq!(
        resp.headers()
            .get("Retry-After")
            .and_then(|value| value.to_str().ok()),
        Some("2")
    );
}

#[actix_web::test]
async fn test_thumbnail_returns_200_after_generation() {
    let state = common::setup().await;
    let app = create_test_app!(state.clone());
    let (token, _) = register_and_login!(app);

    let file_id = upload_file_bytes!(app, token, "test.png", "image/png", tiny_png());

    let first = request_thumbnail!(app, token, file_id);
    assert_eq!(first.status(), 202);

    aster_drive::services::task_service::drain(&state)
        .await
        .unwrap();

    let task = latest_thumbnail_task(&state, file_id).await;
    assert_eq!(task.status, BackgroundTaskStatus::Succeeded);

    let resp = request_thumbnail!(app, token, file_id);
    assert_eq!(resp.status(), 200);
    assert_eq!(
        resp.headers()
            .get("Content-Type")
            .and_then(|value| value.to_str().ok()),
        Some("image/webp")
    );

    let cache_control = resp
        .headers()
        .get("Cache-Control")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    assert!(cache_control.contains("private"));
    assert!(cache_control.contains("must-revalidate"));
    assert!(!cache_control.contains("public"));
    assert!(!cache_control.contains("immutable"));
}

#[actix_web::test]
async fn test_thumbnail_returns_304_for_matching_if_none_match() {
    let state = common::setup().await;
    let app = create_test_app!(state.clone());
    let (token, _) = register_and_login!(app);

    let file_id = upload_file_bytes!(app, token, "test.png", "image/png", tiny_png());

    let first = request_thumbnail!(app, token, file_id);
    assert_eq!(first.status(), 202);

    aster_drive::services::task_service::drain(&state)
        .await
        .unwrap();

    let resp = request_thumbnail!(app, token, file_id);
    assert_eq!(resp.status(), 200);
    let etag = resp
        .headers()
        .get("ETag")
        .and_then(|value| value.to_str().ok())
        .expect("thumbnail response should include ETag")
        .to_string();
    assert!(etag.contains("thumb-v2-"));

    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/files/{file_id}/thumbnail"))
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .insert_header(("If-None-Match", etag.as_str()))
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), 304);
    assert_eq!(
        resp.headers()
            .get("ETag")
            .and_then(|value| value.to_str().ok()),
        Some(etag.as_str())
    );
    assert_eq!(
        resp.headers()
            .get("Cache-Control")
            .and_then(|value| value.to_str().ok()),
        Some("private, max-age=0, must-revalidate")
    );
}

#[actix_web::test]
async fn test_thumbnail_non_image_returns_bad_request_without_task() {
    let state = common::setup().await;
    let app = create_test_app!(state.clone());
    let (token, _) = register_and_login!(app);

    let file_id = upload_test_file!(app, token);
    let resp = request_thumbnail!(app, token, file_id);

    assert_eq!(resp.status(), 400);
    let tasks = background_task_repo::list_recent(&state.db, 16)
        .await
        .unwrap();
    assert!(
        tasks
            .into_iter()
            .all(|task| task.kind != BackgroundTaskKind::ThumbnailGenerate)
    );
}

#[actix_web::test]
async fn test_thumbnail_dedup_same_blob() {
    let state = common::setup().await;
    let app = create_test_app!(state.clone());
    let (token, _) = register_and_login!(app);

    let file_id = upload_file_bytes!(app, token, "test.png", "image/png", tiny_png());

    for _ in 0..5 {
        let resp = request_thumbnail!(app, token, file_id);
        let status = resp.status().as_u16();
        assert!(
            status == 202 || status == 200,
            "thumbnail request should be pending or ready, got {status}"
        );
    }

    assert_eq!(thumbnail_task_count(&state, file_id).await, 1);

    aster_drive::services::task_service::drain(&state)
        .await
        .unwrap();

    let resp = request_thumbnail!(app, token, file_id);
    assert_eq!(resp.status(), 200);
    assert_eq!(thumbnail_task_count(&state, file_id).await, 1);
}

#[actix_web::test]
async fn test_thumbnail_failed_task_returns_error_without_requeue() {
    let state = common::setup().await;
    let app = create_test_app!(state.clone());
    let (token, _) = register_and_login!(app);

    let invalid_png = b"not-a-real-png".to_vec();
    let file_id = upload_file_bytes!(app, token, "broken.png", "image/png", invalid_png);

    let first = request_thumbnail!(app, token, file_id);
    assert_eq!(first.status(), 202);

    aster_drive::services::task_service::drain(&state)
        .await
        .unwrap();

    let task = latest_thumbnail_task(&state, file_id).await;
    assert_eq!(task.status, BackgroundTaskStatus::Failed);
    assert_eq!(task.attempt_count, 1);

    let count_before = thumbnail_task_count(&state, file_id).await;
    assert_eq!(count_before, 1);

    let resp = request_thumbnail!(app, token, file_id);
    assert_eq!(resp.status(), 500);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["code"], json!(ErrorCode::ThumbnailFailed as i32));

    for _ in 0..3 {
        let resp = request_thumbnail!(app, token, file_id);
        assert_eq!(resp.status(), 500);
    }

    let count_after = thumbnail_task_count(&state, file_id).await;
    assert_eq!(count_after, count_before);
}
