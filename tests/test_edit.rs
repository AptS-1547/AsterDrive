#[macro_use]
mod common;

use actix_web::body::to_bytes;
use actix_web::test;
use serde_json::Value;

// ── PUT /content 基本覆盖写入 ───────────────────────────────

#[actix_web::test]
async fn test_update_content_basic() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);
    let file_id = upload_test_file!(app, token);

    // PUT /content 覆盖
    let req = test::TestRequest::put()
        .uri(&format!("/api/v1/files/{file_id}/content"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .insert_header(("Content-Type", "application/octet-stream"))
        .set_payload("updated content")
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200, "PUT content should return 200");

    // 应有 ETag header
    let etag = resp
        .headers()
        .get("ETag")
        .map(|v| v.to_str().unwrap().to_string());
    assert!(etag.is_some(), "response should have ETag header");

    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["code"], 0);

    // 下载验证 ETag
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/files/{file_id}/download"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let dl_etag = resp
        .headers()
        .get("ETag")
        .map(|v| v.to_str().unwrap().to_string());
    assert!(dl_etag.is_some(), "download should have ETag");
    assert_eq!(etag, dl_etag, "ETag should match between PUT and GET");
}

// ── PUT /content 自动创建版本 ───────────────────────────────

#[actix_web::test]
async fn test_update_content_creates_version() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);
    let file_id = upload_test_file!(app, token);

    // 覆盖写入
    let req = test::TestRequest::put()
        .uri(&format!("/api/v1/files/{file_id}/content"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .insert_header(("Content-Type", "application/octet-stream"))
        .set_payload("v2")
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // 查版本列表
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/files/{file_id}/versions"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    let versions = body["data"].as_array().unwrap();
    assert!(
        !versions.is_empty(),
        "should have at least 1 history version"
    );
}

#[actix_web::test]
async fn test_restore_single_history_version_recovers_original_content() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);
    let file_id = upload_test_file!(app, token);

    let req = test::TestRequest::put()
        .uri(&format!("/api/v1/files/{file_id}/content"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .insert_header(("Content-Type", "application/octet-stream"))
        .set_payload("v2")
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/files/{file_id}/versions"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    let versions = body["data"].as_array().unwrap();
    assert_eq!(versions.len(), 1, "should have exactly one history version");
    let version_id = versions[0]["id"].as_i64().unwrap();

    let req = test::TestRequest::post()
        .uri(&format!(
            "/api/v1/files/{file_id}/versions/{version_id}/restore"
        ))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/files/{file_id}/download"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body = to_bytes(resp.into_body()).await.unwrap();
    assert_eq!(body.as_ref(), b"test content");

    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/files/{file_id}/versions"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    let versions = body["data"].as_array().unwrap();
    assert!(versions.is_empty(), "history should be empty after restoring v1");
}

// ── ETag 乐观锁：正确 ETag 通过 ────────────────────────────

#[actix_web::test]
async fn test_update_content_etag_match() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);
    let file_id = upload_test_file!(app, token);

    // 下载拿 ETag
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/files/{file_id}/download"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    let etag = resp
        .headers()
        .get("ETag")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();

    // 用正确 ETag 覆盖
    let req = test::TestRequest::put()
        .uri(&format!("/api/v1/files/{file_id}/content"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .insert_header(("Content-Type", "application/octet-stream"))
        .insert_header(("If-Match", etag.as_str()))
        .set_payload("updated with etag")
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200, "matching ETag should succeed");
}

// ── ETag 乐观锁：错误 ETag 返回 412 ────────────────────────

#[actix_web::test]
async fn test_update_content_etag_mismatch() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);
    let file_id = upload_test_file!(app, token);

    let req = test::TestRequest::put()
        .uri(&format!("/api/v1/files/{file_id}/content"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .insert_header(("Content-Type", "application/octet-stream"))
        .insert_header(("If-Match", "\"wrong-etag\""))
        .set_payload("should fail")
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        412,
        "mismatched ETag should return 412, got {}",
        resp.status()
    );
}

// ── 悲观锁：被其他用户锁定时返回 423 ───────────────────────

#[actix_web::test]
async fn test_update_content_locked_by_other() {
    let state = common::setup().await;
    let db = state.db.clone();
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);
    let file_id = upload_test_file!(app, token);

    // 用另一个 owner_id 直接插锁（模拟其他用户）
    {
        use sea_orm::{ActiveModelTrait, Set};
        let now = chrono::Utc::now();
        let lock = aster_drive::entities::resource_lock::ActiveModel {
            token: Set(format!("urn:uuid:{}", uuid::Uuid::new_v4())),
            entity_type: Set(aster_drive::types::EntityType::File),
            entity_id: Set(file_id),
            path: Set("/test.txt".to_string()),
            owner_id: Set(Some(99999)),
            owner_info: Set(None),
            timeout_at: Set(None),
            shared: Set(false),
            deep: Set(false),
            created_at: Set(now),
            ..Default::default()
        };
        lock.insert(&db).await.unwrap();
        aster_drive::services::lock_service::set_entity_locked(
            &db,
            aster_drive::types::EntityType::File,
            file_id,
            true,
        )
        .await
        .unwrap();
    }

    let req = test::TestRequest::put()
        .uri(&format!("/api/v1/files/{file_id}/content"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .insert_header(("Content-Type", "application/octet-stream"))
        .set_payload("should fail")
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        423,
        "locked file should return 423, got {}",
        resp.status()
    );
}

// ── 悲观锁：锁持有者可以写入 ───────────────────────────────

#[actix_web::test]
async fn test_update_content_lock_owner_can_write() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);
    let file_id = upload_test_file!(app, token);

    // 自己锁定
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/files/{file_id}/lock"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({"locked": true}))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // 锁持有者覆盖
    let req = test::TestRequest::put()
        .uri(&format!("/api/v1/files/{file_id}/content"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .insert_header(("Content-Type", "application/octet-stream"))
        .set_payload("updated by lock owner")
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        200,
        "lock owner should be able to write, got {}",
        resp.status()
    );
}
