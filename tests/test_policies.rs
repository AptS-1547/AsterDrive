//! 存储策略管理测试

#[macro_use]
mod common;

use actix_web::test;
use serde_json::Value;

#[actix_web::test]
async fn test_policy_crud() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    // 列出策略（应有 1 个默认）
    let req = test::TestRequest::get()
        .uri("/api/v1/admin/policies")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"].as_array().unwrap().len(), 1);

    // 创建新策略
    let req = test::TestRequest::post()
        .uri("/api/v1/admin/policies")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "name": "Test S3",
            "driver_type": "s3",
            "endpoint": "http://localhost:9000",
            "bucket": "test-bucket",
            "access_key": "minioadmin",
            "secret_key": "minioadmin",
            "base_path": "",
            "max_file_size": 104857600
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["name"], "Test S3");
    let policy_id = body["data"]["id"].as_i64().unwrap();

    // 获取单个
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/admin/policies/{policy_id}"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // 更新策略
    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/admin/policies/{policy_id}"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({ "name": "Renamed S3" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["name"], "Renamed S3");

    // 删除策略
    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/admin/policies/{policy_id}"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // 只剩默认策略
    let req = test::TestRequest::get()
        .uri("/api/v1/admin/policies")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"].as_array().unwrap().len(), 1);
}

#[actix_web::test]
async fn test_user_policy_assignment() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    // 获取默认策略 ID
    let req = test::TestRequest::get()
        .uri("/api/v1/admin/policies")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    let policy_id = body["data"][0]["id"].as_i64().unwrap();

    // 获取用户 ID
    let req = test::TestRequest::get()
        .uri("/api/v1/admin/users")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    let user_id = body["data"][0]["id"].as_i64().unwrap();

    // 分配策略给用户
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/admin/users/{user_id}/policies"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "policy_id": policy_id,
            "is_default": true,
            "quota_bytes": 1073741824
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);

    // 列出用户策略
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/admin/users/{user_id}/policies"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    let policies = body["data"].as_array().unwrap();
    assert_eq!(policies.len(), 1);
    assert_eq!(policies[0]["policy_id"], policy_id);

    // 删除用户策略
    let usp_id = policies[0]["id"].as_i64().unwrap();
    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/admin/users/{user_id}/policies/{usp_id}"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}
