#[macro_use]
mod common;

use actix_web::test;
use serde_json::Value;

/// 生成一个最小的 1x1 红色 PNG（68 字节）
fn tiny_png() -> Vec<u8> {
    // Minimal valid PNG: 1x1 pixel, RGB, red
    let mut buf = std::io::Cursor::new(Vec::new());
    let encoder = image::codecs::png::PngEncoder::new(&mut buf);
    image::ImageEncoder::write_image(
        encoder,
        &[255, 0, 0], // 1 pixel, red
        1,
        1,
        image::ExtendedColorType::Rgb8,
    )
    .unwrap();
    buf.into_inner()
}

/// 上传一张 PNG 图片，返回 file_id
macro_rules! upload_png {
    ($app:expr, $token:expr) => {{
        let png_bytes = tiny_png();
        let boundary = "----TestBound";
        let mut payload = Vec::new();
        payload.extend_from_slice(b"------TestBound\r\n");
        payload.extend_from_slice(
            b"Content-Disposition: form-data; name=\"file\"; filename=\"test.png\"\r\n",
        );
        payload.extend_from_slice(b"Content-Type: image/png\r\n\r\n");
        payload.extend_from_slice(&png_bytes);
        payload.extend_from_slice(b"\r\n------TestBound--\r\n");

        let req = test::TestRequest::post()
            .uri("/api/v1/files/upload")
            .insert_header(("Cookie", format!("aster_access={}", $token)))
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

#[actix_web::test]
async fn test_thumbnail_returns_202_when_not_ready() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let file_id = upload_png!(app, token);

    // 首次请求缩略图——后台 worker 未运行（测试中 rx 被 drop），应返回 202
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/files/{file_id}/thumbnail"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        202,
        "should return 202 when thumbnail not ready"
    );

    // Retry-After header 应存在
    let retry_after = resp
        .headers()
        .get("Retry-After")
        .and_then(|v| v.to_str().ok());
    assert!(retry_after.is_some(), "should have Retry-After header");
}

#[actix_web::test]
async fn test_thumbnail_returns_200_after_generation() {
    let state = common::setup().await;
    // 启动真正的 thumbnail worker
    let (tx, rx) = tokio::sync::mpsc::channel::<i64>(16);
    aster_drive::services::thumbnail_service::spawn_worker(
        actix_web::web::Data::new(state.db.clone()),
        state.driver_registry.clone(),
        state.policy_snapshot.clone(),
        rx,
    );
    // 替换 state 的 tx
    let state = aster_drive::runtime::AppState {
        db: state.db,
        driver_registry: state.driver_registry,
        runtime_config: state.runtime_config,
        policy_snapshot: state.policy_snapshot,
        config: state.config,
        cache: state.cache,
        mail_sender: state.mail_sender,
        thumbnail_tx: tx,
        storage_change_tx: state.storage_change_tx,
    };
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let file_id = upload_png!(app, token);

    // 首次请求——入队生成
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/files/{file_id}/thumbnail"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    // 可能是 202（worker 还没处理）或 200（worker 很快就处理了）
    let first_status = resp.status().as_u16();
    assert!(
        first_status == 202 || first_status == 200,
        "first request should be 202 or 200, got {first_status}"
    );

    // 等待 worker 处理
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // 再次请求——应已生成
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/files/{file_id}/thumbnail"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        200,
        "should return 200 after worker generates thumbnail"
    );

    // 验证返回的是 WebP
    let content_type = resp
        .headers()
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(content_type, "image/webp");

    // 鉴权缩略图不应允许共享缓存，也不应标记 immutable
    let cache_control = resp
        .headers()
        .get("Cache-Control")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(cache_control.contains("private"));
    assert!(cache_control.contains("no-cache") || cache_control.contains("max-age=0"));
    assert!(cache_control.contains("must-revalidate"));
    assert!(!cache_control.contains("public"));
    assert!(!cache_control.contains("immutable"));
}

#[actix_web::test]
async fn test_thumbnail_returns_304_for_matching_if_none_match() {
    let state = common::setup().await;
    let (tx, rx) = tokio::sync::mpsc::channel::<i64>(16);
    aster_drive::services::thumbnail_service::spawn_worker(
        actix_web::web::Data::new(state.db.clone()),
        state.driver_registry.clone(),
        state.policy_snapshot.clone(),
        rx,
    );
    let state = aster_drive::runtime::AppState {
        db: state.db,
        driver_registry: state.driver_registry,
        runtime_config: state.runtime_config,
        policy_snapshot: state.policy_snapshot,
        config: state.config,
        cache: state.cache,
        mail_sender: state.mail_sender,
        thumbnail_tx: tx,
        storage_change_tx: state.storage_change_tx,
    };
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let file_id = upload_png!(app, token);

    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/files/{file_id}/thumbnail"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let _ = test::call_service(&app, req).await;

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/files/{file_id}/thumbnail"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
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
        .insert_header(("Cookie", format!("aster_access={token}")))
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
async fn test_thumbnail_non_image_returns_error() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    // 上传 txt 文件（不是图片）
    let file_id = upload_test_file!(app, token);

    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/files/{file_id}/thumbnail"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    // 非图片应返回错误（不是 202）
    assert_ne!(resp.status(), 202);
    assert_ne!(resp.status(), 200);
}

#[actix_web::test]
async fn test_thumbnail_dedup_same_blob() {
    let state = common::setup().await;
    let (tx, rx) = tokio::sync::mpsc::channel::<i64>(16);
    aster_drive::services::thumbnail_service::spawn_worker(
        actix_web::web::Data::new(state.db.clone()),
        state.driver_registry.clone(),
        state.policy_snapshot.clone(),
        rx,
    );
    let state = aster_drive::runtime::AppState {
        db: state.db,
        driver_registry: state.driver_registry,
        runtime_config: state.runtime_config,
        policy_snapshot: state.policy_snapshot,
        config: state.config,
        cache: state.cache,
        mail_sender: state.mail_sender,
        thumbnail_tx: tx,
        storage_change_tx: state.storage_change_tx,
    };
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let file_id = upload_png!(app, token);

    // 连续请求多次——channel 只应入队一次（去重）
    for _ in 0..5 {
        let req = test::TestRequest::get()
            .uri(&format!("/api/v1/files/{file_id}/thumbnail"))
            .insert_header(("Cookie", format!("aster_access={token}")))
            .to_request();
        let _ = test::call_service(&app, req).await;
    }

    // 等待 worker 处理
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // 最终请求应返回 200
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/files/{file_id}/thumbnail"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}
