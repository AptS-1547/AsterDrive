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
    assert_eq!(body["data"]["items"].as_array().unwrap().len(), 1);
    assert_eq!(body["data"]["total"], 1);

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
            "max_file_size": 104857600,
            "chunk_size": 8388608,
            "options": serde_json::json!({
                "presigned_upload": true
            }).to_string()
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["name"], "Test S3");
    assert_eq!(body["data"]["chunk_size"], 8_388_608);
    assert_eq!(body["data"]["options"], r#"{"presigned_upload":true}"#);
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
    assert_eq!(body["data"]["items"].as_array().unwrap().len(), 1);
    assert_eq!(body["data"]["total"], 1);
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
    let policy_id = body["data"]["items"][0]["id"].as_i64().unwrap();

    // 获取用户 ID
    let req = test::TestRequest::get()
        .uri("/api/v1/admin/users")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    let user_id = body["data"]["items"][0]["id"].as_i64().unwrap();

    // 分配一个非默认策略给用户
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/admin/users/{user_id}/policies"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "policy_id": policy_id,
            "is_default": false,
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
    let policies = body["data"]["items"].as_array().unwrap();
    assert_eq!(policies.len(), 2, "should have 2 policies (auto + manual)");

    // 删除手动分配的策略（保留自动分配的）
    let manual = policies
        .iter()
        .find(|p| p["quota_bytes"] == 1073741824)
        .unwrap();
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
    let policies = body["data"]["items"].as_array().unwrap();
    let default_count = policies.iter().filter(|p| p["is_default"] == true).count();
    assert_eq!(
        default_count, 1,
        "should have exactly 1 default policy, got {default_count}"
    );
}

#[actix_web::test]
async fn test_patch_policy_promotes_existing_policy_to_default() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let req = test::TestRequest::post()
        .uri("/api/v1/admin/policies")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "name": "Patch To Default",
            "driver_type": "local",
            "base_path": "/tmp/test-patch-default",
            "max_file_size": 0,
            "is_default": false
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let policy_id = body["data"]["id"].as_i64().unwrap();

    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/admin/policies/{policy_id}"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({ "is_default": true }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["is_default"], true);

    let req = test::TestRequest::get()
        .uri("/api/v1/admin/policies")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    let policies = body["data"]["items"].as_array().unwrap();
    let default_ids: Vec<i64> = policies
        .iter()
        .filter(|policy| policy["is_default"] == true)
        .map(|policy| policy["id"].as_i64().unwrap())
        .collect();

    assert_eq!(default_ids, vec![policy_id]);
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
    let policy_id = body["data"]["items"][0]["id"].as_i64().unwrap();

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

#[actix_web::test]
async fn test_cannot_delete_builtin_system_policy_even_after_switching_default() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let req = test::TestRequest::post()
        .uri("/api/v1/admin/policies")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "name": "Replacement Default",
            "driver_type": "local",
            "base_path": format!("/tmp/test-replacement-default-{}", uuid::Uuid::new_v4()),
            "max_file_size": 0,
            "is_default": true
        }))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);

    let req = test::TestRequest::delete()
        .uri("/api/v1/admin/policies/1")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        400,
        "should reject deleting built-in policy #1, got {}",
        resp.status()
    );

    let req = test::TestRequest::get()
        .uri("/api/v1/admin/policies")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    let policies = body["data"]["items"].as_array().unwrap();
    assert!(
        policies.iter().any(|policy| policy["id"] == 1),
        "built-in policy #1 should still exist after failed delete"
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
    let policy_id = body["data"]["items"][0]["id"].as_i64().unwrap();

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

// ── 不能删除用户默认策略分配 ────────────────────────────────

#[actix_web::test]
async fn test_cannot_delete_default_user_policy() {
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
    let policy_id = body["data"]["items"][0]["id"].as_i64().unwrap();

    let req = test::TestRequest::get()
        .uri("/api/v1/admin/users")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    let user_id = body["data"]["items"][0]["id"].as_i64().unwrap();

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
    let default_assignment_id = body["data"]["id"].as_i64().unwrap();

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

    // 删除 default 策略分配 → 应被拒绝
    let req = test::TestRequest::delete()
        .uri(&format!(
            "/api/v1/admin/users/{user_id}/policies/{default_assignment_id}"
        ))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);

    // 默认分配仍存在，且默认标记未变化
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/admin/users/{user_id}/policies"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    let policies = body["data"]["items"].as_array().unwrap();
    let default_assignment = policies
        .iter()
        .find(|p| p["id"] == default_assignment_id)
        .unwrap();
    assert_eq!(policies.len(), 3);
    assert_eq!(default_assignment["is_default"], true);
}

#[actix_web::test]
async fn test_can_delete_non_default_user_policy_with_multiple_assignments() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let req = test::TestRequest::get()
        .uri("/api/v1/admin/users")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    let user_id = body["data"]["items"][0]["id"].as_i64().unwrap();

    let req = test::TestRequest::post()
        .uri("/api/v1/admin/policies")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "name": "Third Policy",
            "driver_type": "local",
            "base_path": "/tmp/test-third",
            "max_file_size": 0,
            "is_default": false
        }))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let policy_id = body["data"]["id"].as_i64().unwrap();

    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/admin/users/{user_id}/policies"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "policy_id": policy_id,
            "is_default": false,
            "quota_bytes": 0
        }))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let assignment_id = body["data"]["id"].as_i64().unwrap();

    let req = test::TestRequest::delete()
        .uri(&format!(
            "/api/v1/admin/users/{user_id}/policies/{assignment_id}"
        ))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/admin/users/{user_id}/policies"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    let policies = body["data"]["items"].as_array().unwrap();
    assert_eq!(policies.len(), 1);
    assert_eq!(policies[0]["is_default"], true);
}

#[actix_web::test]
async fn test_patch_user_policy_switches_default_assignment() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let req = test::TestRequest::get()
        .uri("/api/v1/admin/users")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    let user_id = body["data"]["items"][0]["id"].as_i64().unwrap();

    let req = test::TestRequest::post()
        .uri("/api/v1/admin/policies")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "name": "Patch User Default",
            "driver_type": "local",
            "base_path": "/tmp/test-user-default-switch",
            "max_file_size": 0,
            "is_default": false
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let policy_id = body["data"]["id"].as_i64().unwrap();

    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/admin/users/{user_id}/policies"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "policy_id": policy_id,
            "is_default": false,
            "quota_bytes": 1234
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let assignment_id = body["data"]["id"].as_i64().unwrap();

    let req = test::TestRequest::patch()
        .uri(&format!(
            "/api/v1/admin/users/{user_id}/policies/{assignment_id}"
        ))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "is_default": true,
            "quota_bytes": 4321
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["is_default"], true);
    assert_eq!(body["data"]["quota_bytes"], 4321);

    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/admin/users/{user_id}/policies"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    let policies = body["data"]["items"].as_array().unwrap();
    let default_ids: Vec<i64> = policies
        .iter()
        .filter(|policy| policy["is_default"] == true)
        .map(|policy| policy["id"].as_i64().unwrap())
        .collect();

    assert_eq!(default_ids, vec![assignment_id]);
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
    let policy_id = body["data"]["items"][0]["id"].as_i64().unwrap();

    let req = test::TestRequest::get()
        .uri("/api/v1/admin/users")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    let user_id = body["data"]["items"][0]["id"].as_i64().unwrap();

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
    let user_id = body["data"]["items"][0]["id"].as_i64().unwrap();

    // 注册时已自动分配 1 个策略，直接获取它
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/admin/users/{user_id}/policies"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    let policies = body["data"]["items"].as_array().unwrap();
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

#[actix_web::test]
async fn test_policy_delete_clears_folder_policy_reference() {
    use aster_drive::db::repository::folder_repo;

    let state = common::setup().await;
    let db = state.db.clone();
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let req = test::TestRequest::post()
        .uri("/api/v1/admin/policies")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "name": "Folder Override Policy",
            "driver_type": "local",
            "base_path": "/tmp/test-folder-override-policy",
            "max_file_size": 0,
            "is_default": false
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let policy_id = body["data"]["id"].as_i64().unwrap();

    let req = test::TestRequest::post()
        .uri("/api/v1/folders")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({ "name": "override-folder" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let folder_id = body["data"]["id"].as_i64().unwrap();

    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/folders/{folder_id}"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({ "policy_id": policy_id }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let folder = folder_repo::find_by_id(&db, folder_id).await.unwrap();
    assert_eq!(folder.policy_id, Some(policy_id));

    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/admin/policies/{policy_id}"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let folder = folder_repo::find_by_id(&db, folder_id).await.unwrap();
    assert_eq!(folder.policy_id, None);
}

#[actix_web::test]
async fn test_folder_patch_can_clear_policy_with_null() {
    use aster_drive::db::repository::folder_repo;

    let state = common::setup().await;
    let db = state.db.clone();
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let req = test::TestRequest::post()
        .uri("/api/v1/admin/policies")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "name": "Nullable Folder Override Policy",
            "driver_type": "local",
            "base_path": "/tmp/test-nullable-folder-override-policy",
            "max_file_size": 0,
            "is_default": false
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let policy_id = body["data"]["id"].as_i64().unwrap();

    let req = test::TestRequest::post()
        .uri("/api/v1/folders")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({ "name": "nullable-override-folder" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let folder_id = body["data"]["id"].as_i64().unwrap();

    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/folders/{folder_id}"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({ "policy_id": policy_id }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let folder = folder_repo::find_by_id(&db, folder_id).await.unwrap();
    assert_eq!(folder.policy_id, Some(policy_id));

    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/folders/{folder_id}"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({ "policy_id": null }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert!(body["data"]["policy_id"].is_null());

    let folder = folder_repo::find_by_id(&db, folder_id).await.unwrap();
    assert_eq!(folder.policy_id, None);
}

#[actix_web::test]
async fn test_policy_connection_endpoints_for_local_driver() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let stored_base_path = format!("/tmp/test-policy-connection-{}", uuid::Uuid::new_v4());
    let req = test::TestRequest::post()
        .uri("/api/v1/admin/policies")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "name": "Connection Test Policy",
            "driver_type": "local",
            "base_path": stored_base_path,
            "max_file_size": 0,
            "is_default": false
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let policy_id = body["data"]["id"].as_i64().unwrap();

    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/admin/policies/{policy_id}/test"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    assert!(!std::path::Path::new(&format!("{stored_base_path}/_aster_connection_test")).exists());

    let temp_base_path = format!("/tmp/test-policy-params-{}", uuid::Uuid::new_v4());
    let req = test::TestRequest::post()
        .uri("/api/v1/admin/policies/test")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "driver_type": "local",
            "base_path": temp_base_path
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    assert!(!std::path::Path::new(&format!("{temp_base_path}/_aster_connection_test")).exists());
}
