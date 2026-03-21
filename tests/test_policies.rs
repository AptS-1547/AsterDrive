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

    // 列出用户策略（注册时自动分配 1 个 + 手动分配 1 个 = 2 个）
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/admin/users/{user_id}/policies"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    let policies = body["data"].as_array().unwrap();
    assert_eq!(policies.len(), 2, "should have 2 policies (auto + manual)");

    // 删除手动分配的策略（保留自动分配的）
    let manual = policies.iter().find(|p| p["quota_bytes"] == 1073741824).unwrap();
    let usp_id = manual["id"].as_i64().unwrap();
    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/admin/users/{user_id}/policies/{usp_id}"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

// ── 系统策略 default 唯一性 ─────────────────────────────────

#[actix_web::test]
async fn test_system_policy_default_uniqueness() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    // 创建第二个策略并设为 default
    let req = test::TestRequest::post()
        .uri("/api/v1/admin/policies")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "name": "New Default",
            "driver_type": "local",
            "base_path": "/tmp/test-new-default",
            "max_file_size": 0,
            "is_default": true
        }))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);

    // 列出所有策略，应只有一个 is_default=true
    let req = test::TestRequest::get()
        .uri("/api/v1/admin/policies")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    let policies = body["data"].as_array().unwrap();
    let default_count = policies.iter().filter(|p| p["is_default"] == true).count();
    assert_eq!(
        default_count, 1,
        "should have exactly 1 default policy, got {default_count}"
    );
}

// ── 不能删除唯一的默认系统策略 ──────────────────────────────

#[actix_web::test]
async fn test_cannot_delete_only_default_policy() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    // 获取默认策略 ID
    let req = test::TestRequest::get()
        .uri("/api/v1/admin/policies")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    let policy_id = body["data"][0]["id"].as_i64().unwrap();

    // 尝试删除唯一默认策略 → 应被拒绝
    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/admin/policies/{policy_id}"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        400,
        "should reject deleting only default policy, got {}",
        resp.status()
    );
}

// ── 不能取消唯一的默认系统策略 ──────────────────────────────

#[actix_web::test]
async fn test_cannot_unset_only_default_policy() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    // 获取默认策略 ID
    let req = test::TestRequest::get()
        .uri("/api/v1/admin/policies")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    let policy_id = body["data"][0]["id"].as_i64().unwrap();

    // 尝试取消 default → 应被拒绝
    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/admin/policies/{policy_id}"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({"is_default": false}))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        400,
        "should reject unsetting only default, got {}",
        resp.status()
    );
}

// ── 删除用户默认策略时自动转移 ──────────────────────────────

#[actix_web::test]
async fn test_user_policy_default_auto_promote() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    // 获取策略 ID 和用户 ID
    let req = test::TestRequest::get()
        .uri("/api/v1/admin/policies")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    let policy_id = body["data"][0]["id"].as_i64().unwrap();

    let req = test::TestRequest::get()
        .uri("/api/v1/admin/users")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    let user_id = body["data"][0]["id"].as_i64().unwrap();

    // 创建第二个策略（非默认）
    let req = test::TestRequest::post()
        .uri("/api/v1/admin/policies")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "name": "Second Policy",
            "driver_type": "local",
            "base_path": "/tmp/test-second",
            "max_file_size": 0,
            "is_default": false
        }))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let policy2_id = body["data"]["id"].as_i64().unwrap();

    // 分配两个策略给用户，第一个是 default
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/admin/users/{user_id}/policies"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "policy_id": policy_id,
            "is_default": true,
            "quota_bytes": 0
        }))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let usp1_id = body["data"]["id"].as_i64().unwrap();

    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/admin/users/{user_id}/policies"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "policy_id": policy2_id,
            "is_default": false,
            "quota_bytes": 0
        }))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);

    // 删除 default 策略分配
    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/admin/users/{user_id}/policies/{usp1_id}"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // 剩余的策略中应有一个 is_default=true
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/admin/users/{user_id}/policies"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    let policies = body["data"].as_array().unwrap();
    let has_default = policies.iter().any(|p| p["is_default"] == true);
    assert!(
        has_default,
        "should have at least one default policy after deleting the previous default"
    );
}

// ── 不能取消用户唯一默认策略 ────────────────────────────────

#[actix_web::test]
async fn test_cannot_unset_only_user_default_policy() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    // 获取策略 ID 和用户 ID
    let req = test::TestRequest::get()
        .uri("/api/v1/admin/policies")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    let policy_id = body["data"][0]["id"].as_i64().unwrap();

    let req = test::TestRequest::get()
        .uri("/api/v1/admin/users")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    let user_id = body["data"][0]["id"].as_i64().unwrap();

    // 分配唯一策略（default）
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/admin/users/{user_id}/policies"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "policy_id": policy_id,
            "is_default": true,
            "quota_bytes": 0
        }))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let usp_id = body["data"]["id"].as_i64().unwrap();

    // 尝试取消 default → 应被拒绝
    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/admin/users/{user_id}/policies/{usp_id}"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({"is_default": false}))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        400,
        "should reject unsetting only user default, got {}",
        resp.status()
    );
}

// ── 不能删除用户唯一的策略分配 ──────────────────────────────

#[actix_web::test]
async fn test_cannot_delete_last_user_policy() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let req = test::TestRequest::get()
        .uri("/api/v1/admin/users")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    let user_id = body["data"][0]["id"].as_i64().unwrap();

    // 注册时已自动分配 1 个策略，直接获取它
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/admin/users/{user_id}/policies"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    let policies = body["data"].as_array().unwrap();
    assert_eq!(policies.len(), 1, "user should have 1 auto-assigned policy");
    let usp_id = policies[0]["id"].as_i64().unwrap();

    // 尝试删除唯一策略 → 应被拒绝
    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/admin/users/{user_id}/policies/{usp_id}"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        400,
        "should reject deleting only user policy, got {}",
        resp.status()
    );
}
