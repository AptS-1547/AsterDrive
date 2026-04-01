//! 存储策略管理测试

#[macro_use]
mod common;

use actix_web::test;
use serde_json::Value;

#[actix_web::test]
async fn test_user_default_policy_switch_updates_snapshot_immediately() {
    use aster_drive::services::{auth_service, file_service, policy_service};
    use aster_drive::types::DriverType;

    let state = common::setup().await;
    let user = auth_service::register(
        &state,
        "policysnapsw",
        "policy-snapshot-switch@example.com",
        "password123",
    )
    .await
    .unwrap();

    let initial_policy = file_service::resolve_policy(&state, user.id, None)
        .await
        .unwrap();

    let alternate_base_path = format!("/tmp/asterdrive-policy-switch-{}", uuid::Uuid::new_v4());
    std::fs::create_dir_all(&alternate_base_path).unwrap();
    let alternate_policy = policy_service::create(
        &state,
        "Alternate Local",
        DriverType::Local,
        "",
        "",
        "",
        "",
        &alternate_base_path,
        0,
        None,
        false,
        None,
    )
    .await
    .unwrap();

    assert_ne!(alternate_policy.id, initial_policy.id);

    policy_service::assign_user_policy(&state, user.id, alternate_policy.id, true, 0)
        .await
        .unwrap();

    assert_eq!(
        state.policy_snapshot.resolve_default_policy_id(user.id),
        Some(alternate_policy.id)
    );

    let resolved_after_switch = file_service::resolve_policy(&state, user.id, None)
        .await
        .unwrap();
    assert_eq!(resolved_after_switch.id, alternate_policy.id);
}

#[actix_web::test]
async fn test_seed_policy_groups_backfills_missing_users_to_default_group() {
    use aster_drive::db::repository::{policy_group_repo, user_repo};
    use aster_drive::services::{auth_service, policy_service};
    use sea_orm::{ActiveModelTrait, Set};

    let state = common::setup().await;
    let user = auth_service::register(
        &state,
        "policybackfill",
        "policy-backfill@example.com",
        "password123",
    )
    .await
    .unwrap();
    let default_group = policy_group_repo::find_default_group(&state.db)
        .await
        .unwrap()
        .expect("default group should exist");

    let mut user_active: aster_drive::entities::user::ActiveModel = user.into();
    user_active.policy_group_id = Set(None);
    user_active.update(&state.db).await.unwrap();

    policy_service::ensure_policy_groups_seeded(&state.db)
        .await
        .unwrap();

    let updated = user_repo::find_by_email(&state.db, "policy-backfill@example.com")
        .await
        .unwrap()
        .expect("user should exist");
    assert_eq!(updated.policy_group_id, Some(default_group.id));
}

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

    let req = test::TestRequest::post()
        .uri("/api/v1/admin/policy-groups")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "name": "Dedicated User Group",
            "description": "Single binding target",
            "is_enabled": true,
            "is_default": false,
            "items": [
                {
                    "policy_id": policy_id,
                    "priority": 1,
                    "min_file_size": 0,
                    "max_file_size": 0
                }
            ]
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let group_id = body["data"]["id"].as_i64().unwrap();

    // 获取用户 ID
    let req = test::TestRequest::get()
        .uri("/api/v1/admin/users")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    let user_id = body["data"]["items"][0]["id"].as_i64().unwrap();

    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/admin/users/{user_id}"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "policy_group_id": group_id
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["policy_group_id"], group_id);

    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/admin/users/{user_id}"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["policy_group_id"], group_id);

    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/admin/users/{user_id}"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "policy_group_id": serde_json::Value::Null
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);
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

#[actix_web::test]
async fn test_cannot_disable_default_policy_group() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let req = test::TestRequest::get()
        .uri("/api/v1/admin/policy-groups")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    let group_id = body["data"]["items"][0]["id"].as_i64().unwrap();

    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/admin/policy-groups/{group_id}"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({ "is_enabled": false }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(
        body["msg"],
        "cannot disable the default storage policy group; set another group as default first"
    );
}

#[actix_web::test]
async fn test_cannot_disable_assigned_policy_group() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let req = test::TestRequest::get()
        .uri("/api/v1/admin/policies")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    let policy_id = body["data"]["items"][0]["id"].as_i64().unwrap();

    let req = test::TestRequest::post()
        .uri("/api/v1/admin/policy-groups")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "name": "Assigned Group",
            "description": "Used by one user",
            "is_enabled": true,
            "is_default": false,
            "items": [
                {
                    "policy_id": policy_id,
                    "priority": 1,
                    "min_file_size": 0,
                    "max_file_size": 0
                }
            ]
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let group_id = body["data"]["id"].as_i64().unwrap();

    let req = test::TestRequest::get()
        .uri("/api/v1/admin/users")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    let user_id = body["data"]["items"][0]["id"].as_i64().unwrap();

    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/admin/users/{user_id}"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "policy_group_id": group_id
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/admin/policy-groups/{group_id}"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({ "is_enabled": false }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(
        body["msg"],
        "cannot disable policy group: 1 user assignment(s) still reference it"
    );
}

#[actix_web::test]
async fn test_cannot_set_disabled_user_policy_group_as_default() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let req = test::TestRequest::get()
        .uri("/api/v1/admin/policies")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    let policy_id = body["data"]["items"][0]["id"].as_i64().unwrap();

    let req = test::TestRequest::post()
        .uri("/api/v1/admin/policy-groups")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "name": "Legacy Disabled Group",
            "description": "Disabled after assignment",
            "is_enabled": true,
            "is_default": false,
            "items": [
                {
                    "policy_id": policy_id,
                    "priority": 1,
                    "min_file_size": 0,
                    "max_file_size": 0
                }
            ]
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let group_id = body["data"]["id"].as_i64().unwrap();

    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/admin/policy-groups/{group_id}"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({ "is_enabled": false }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::get()
        .uri("/api/v1/admin/users")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    let user_id = body["data"]["items"][0]["id"].as_i64().unwrap();

    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/admin/users/{user_id}"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({ "policy_group_id": group_id }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["msg"], "cannot assign a disabled storage policy group");
}

#[actix_web::test]
async fn test_migrate_policy_group_users_moves_assignments_and_preserves_default() {
    use aster_drive::services::auth_service;

    let state = common::setup().await;
    let admin_user = auth_service::register(
        &state,
        "pgmigrate-admin",
        "pgmigrate-admin@example.com",
        "password123",
    )
    .await
    .unwrap();
    let user_with_source_only = auth_service::register(
        &state,
        "pgmigrate1",
        "pgmigrate1@example.com",
        "password123",
    )
    .await
    .unwrap();
    let user_with_existing_target = auth_service::register(
        &state,
        "pgmigrate2",
        "pgmigrate2@example.com",
        "password123",
    )
    .await
    .unwrap();
    let app = create_test_app!(state);
    let req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "identifier": admin_user.username,
            "password": "password123"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let token = common::extract_cookie(&resp, "aster_access").unwrap();

    let req = test::TestRequest::get()
        .uri("/api/v1/admin/policies")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    let policy_id = body["data"]["items"][0]["id"].as_i64().unwrap();

    let req = test::TestRequest::post()
        .uri("/api/v1/admin/policy-groups")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "name": "Source Group",
            "description": "Users will be migrated away",
            "is_enabled": true,
            "is_default": false,
            "items": [
                {
                    "policy_id": policy_id,
                    "priority": 1,
                    "min_file_size": 0,
                    "max_file_size": 0
                }
            ]
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let source_group_id = body["data"]["id"].as_i64().unwrap();

    let req = test::TestRequest::post()
        .uri("/api/v1/admin/policy-groups")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "name": "Target Group",
            "description": "Users land here after migration",
            "is_enabled": true,
            "is_default": false,
            "items": [
                {
                    "policy_id": policy_id,
                    "priority": 1,
                    "min_file_size": 0,
                    "max_file_size": 0
                }
            ]
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let target_group_id = body["data"]["id"].as_i64().unwrap();

    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/admin/users/{}", user_with_source_only.id))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "policy_group_id": source_group_id
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::patch()
        .uri(&format!(
            "/api/v1/admin/users/{}",
            user_with_existing_target.id
        ))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "policy_group_id": source_group_id
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::post()
        .uri(&format!(
            "/api/v1/admin/policy-groups/{source_group_id}/migrate-users"
        ))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "target_group_id": target_group_id
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["source_group_id"], source_group_id);
    assert_eq!(body["data"]["target_group_id"], target_group_id);
    assert_eq!(body["data"]["affected_users"], 2);
    assert_eq!(body["data"]["migrated_assignments"], 2);

    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/admin/users/{}", user_with_source_only.id))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["policy_group_id"], target_group_id);

    let req = test::TestRequest::get()
        .uri(&format!(
            "/api/v1/admin/users/{}",
            user_with_existing_target.id
        ))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["policy_group_id"], target_group_id);
}

#[actix_web::test]
async fn test_cannot_migrate_policy_group_users_to_disabled_group() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let req = test::TestRequest::get()
        .uri("/api/v1/admin/policies")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    let policy_id = body["data"]["items"][0]["id"].as_i64().unwrap();

    let req = test::TestRequest::post()
        .uri("/api/v1/admin/policy-groups")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "name": "Migration Source",
            "description": "source",
            "is_enabled": true,
            "is_default": false,
            "items": [
                {
                    "policy_id": policy_id,
                    "priority": 1,
                    "min_file_size": 0,
                    "max_file_size": 0
                }
            ]
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let source_group_id = body["data"]["id"].as_i64().unwrap();

    let req = test::TestRequest::post()
        .uri("/api/v1/admin/policy-groups")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "name": "Disabled Target",
            "description": "target",
            "is_enabled": false,
            "is_default": false,
            "items": [
                {
                    "policy_id": policy_id,
                    "priority": 1,
                    "min_file_size": 0,
                    "max_file_size": 0
                }
            ]
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let target_group_id = body["data"]["id"].as_i64().unwrap();

    let req = test::TestRequest::post()
        .uri(&format!(
            "/api/v1/admin/policy-groups/{source_group_id}/migrate-users"
        ))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "target_group_id": target_group_id
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(
        body["msg"],
        "cannot migrate users to a disabled storage policy group"
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

// ── 用户绑定策略组的运行时校验 ─────────────────────────────

#[actix_web::test]
async fn test_resolve_policy_fails_without_user_policy_group() {
    use aster_drive::db::repository::user_repo;
    use aster_drive::services::{auth_service, file_service};
    use sea_orm::{ActiveModelTrait, Set};

    let state = common::setup().await;
    let user = auth_service::register(
        &state,
        "nogroup-user",
        "nogroup-user@example.com",
        "password123",
    )
    .await
    .unwrap();

    let model = user_repo::find_by_id(&state.db, user.id).await.unwrap();
    let mut active: aster_drive::entities::user::ActiveModel = model.into();
    active.policy_group_id = Set(None);
    active.updated_at = Set(chrono::Utc::now());
    active.update(&state.db).await.unwrap();
    state.policy_snapshot.reload(&state.db).await.unwrap();

    let err = file_service::resolve_policy(&state, user.id, None)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "E030");
    assert!(err.message().contains("no storage policy group assigned"));
}

#[actix_web::test]
async fn test_resolve_policy_fails_for_disabled_assigned_policy_group() {
    use aster_drive::db::repository::{policy_group_repo, user_repo};
    use aster_drive::services::{auth_service, file_service};
    use sea_orm::{ActiveModelTrait, Set};

    let state = common::setup().await;
    let user = auth_service::register(
        &state,
        "disabledgrpusr",
        "disabled-group-user@example.com",
        "password123",
    )
    .await
    .unwrap();

    let default_policy = aster_drive::db::repository::policy_repo::find_default(&state.db)
        .await
        .unwrap()
        .unwrap();
    let now = chrono::Utc::now();
    let group = policy_group_repo::create_group(
        &state.db,
        aster_drive::entities::storage_policy_group::ActiveModel {
            name: Set("Disabled Assigned Group".to_string()),
            description: Set(String::new()),
            is_enabled: Set(true),
            is_default: Set(false),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    policy_group_repo::create_group_item(
        &state.db,
        aster_drive::entities::storage_policy_group_item::ActiveModel {
            group_id: Set(group.id),
            policy_id: Set(default_policy.id),
            priority: Set(1),
            min_file_size: Set(0),
            max_file_size: Set(0),
            created_at: Set(now),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let user_model = user_repo::find_by_id(&state.db, user.id).await.unwrap();
    let mut user_active: aster_drive::entities::user::ActiveModel = user_model.into();
    user_active.policy_group_id = Set(Some(group.id));
    user_active.updated_at = Set(chrono::Utc::now());
    user_active.update(&state.db).await.unwrap();

    let group_model = policy_group_repo::find_group_by_id(&state.db, group.id)
        .await
        .unwrap();
    let mut group_active: aster_drive::entities::storage_policy_group::ActiveModel =
        group_model.into();
    group_active.is_enabled = Set(false);
    group_active.updated_at = Set(chrono::Utc::now());
    group_active.update(&state.db).await.unwrap();

    state.policy_snapshot.reload(&state.db).await.unwrap();

    let err = file_service::resolve_policy(&state, user.id, None)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "E005");
    assert!(err.message().contains("is disabled"));
}

#[actix_web::test]
async fn test_resolve_policy_fails_when_policy_group_has_no_matching_rule() {
    use aster_drive::db::repository::{policy_group_repo, policy_repo, user_repo};
    use aster_drive::services::{auth_service, file_service, policy_service};
    use aster_drive::types::DriverType;
    use sea_orm::{ActiveModelTrait, Set};

    let state = common::setup().await;
    let user = auth_service::register(
        &state,
        "gappolicyuser",
        "gap-policy-user@example.com",
        "password123",
    )
    .await
    .unwrap();

    let default_policy = policy_repo::find_default(&state.db).await.unwrap().unwrap();
    let overflow_path = format!("/tmp/asterdrive-gap-policy-{}", uuid::Uuid::new_v4());
    std::fs::create_dir_all(&overflow_path).unwrap();
    let overflow_policy = policy_service::create(
        &state,
        "Gap Overflow Policy",
        DriverType::Local,
        "",
        "",
        "",
        "",
        &overflow_path,
        0,
        None,
        false,
        None,
    )
    .await
    .unwrap();

    let now = chrono::Utc::now();
    let group = policy_group_repo::create_group(
        &state.db,
        aster_drive::entities::storage_policy_group::ActiveModel {
            name: Set("Gap Rule Group".to_string()),
            description: Set(String::new()),
            is_enabled: Set(true),
            is_default: Set(false),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    for (priority, policy_id, min_file_size, max_file_size) in [
        (1, default_policy.id, 0, 10),
        (2, overflow_policy.id, 20, 0),
    ] {
        policy_group_repo::create_group_item(
            &state.db,
            aster_drive::entities::storage_policy_group_item::ActiveModel {
                group_id: Set(group.id),
                policy_id: Set(policy_id),
                priority: Set(priority),
                min_file_size: Set(min_file_size),
                max_file_size: Set(max_file_size),
                created_at: Set(now),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    }

    let user_model = user_repo::find_by_id(&state.db, user.id).await.unwrap();
    let mut user_active: aster_drive::entities::user::ActiveModel = user_model.into();
    user_active.policy_group_id = Set(Some(group.id));
    user_active.updated_at = Set(now);
    user_active.update(&state.db).await.unwrap();
    state.policy_snapshot.reload(&state.db).await.unwrap();

    let err = file_service::resolve_policy_for_size(&state, user.id, None, 15)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "E005");
    assert!(err.message().contains("no storage policy rule"));
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
