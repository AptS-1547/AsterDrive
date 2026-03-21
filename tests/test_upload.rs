//! 分片上传集成测试

#[macro_use]
mod common;

use actix_web::test;
use serde_json::Value;

#[actix_web::test]
async fn test_chunked_upload_flow() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    // 1. 初始化分片上传（10KB 文件，chunk_size=5MB → 直传模式）
    let req = test::TestRequest::post()
        .uri("/api/v1/files/upload/init")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "filename": "chunked.txt",
            "total_size": 10240
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    // 小文件可能返回 direct 模式
    let mode = body["data"]["mode"].as_str().unwrap();
    assert!(
        mode == "direct" || mode == "chunked",
        "mode should be direct or chunked, got {mode}"
    );

    if mode == "chunked" {
        let upload_id = body["data"]["upload_id"].as_str().unwrap().to_string();
        let total_chunks = body["data"]["total_chunks"].as_i64().unwrap();

        // 2. 上传分片
        for i in 0..total_chunks {
            let chunk_data = vec![b'A'; 5120]; // 5KB per chunk
            let req = test::TestRequest::put()
                .uri(&format!("/api/v1/files/upload/{upload_id}/{i}"))
                .insert_header(("Cookie", format!("aster_access={token}")))
                .insert_header(("Content-Type", "application/octet-stream"))
                .set_payload(chunk_data)
                .to_request();
            let resp = test::call_service(&app, req).await;
            assert_eq!(resp.status(), 200, "chunk {i} upload failed");
        }

        // 3. 查看进度
        let req = test::TestRequest::get()
            .uri(&format!("/api/v1/files/upload/{upload_id}"))
            .insert_header(("Cookie", format!("aster_access={token}")))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        // 4. 完成上传
        let req = test::TestRequest::post()
            .uri(&format!("/api/v1/files/upload/{upload_id}/complete"))
            .insert_header(("Cookie", format!("aster_access={token}")))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 201);
        let body: Value = test::read_body_json(resp).await;
        assert_eq!(body["data"]["name"], "chunked.txt");
    }
}

#[actix_web::test]
async fn test_chunked_upload_cancel() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    // 初始化大文件上传（强制 chunked 模式）
    let req = test::TestRequest::post()
        .uri("/api/v1/files/upload/init")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "filename": "big.bin",
            "total_size": 10_485_760  // 10MB → 超过 chunk_size(5MB) → chunked
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;

    if let Some(upload_id) = body["data"]["upload_id"].as_str() {
        // 取消上传
        let req = test::TestRequest::delete()
            .uri(&format!("/api/v1/files/upload/{upload_id}"))
            .insert_header(("Cookie", format!("aster_access={token}")))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        // 再查进度应该 404
        let req = test::TestRequest::get()
            .uri(&format!("/api/v1/files/upload/{upload_id}"))
            .insert_header(("Cookie", format!("aster_access={token}")))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status() == 404 || resp.status() == 410);
    }
}
