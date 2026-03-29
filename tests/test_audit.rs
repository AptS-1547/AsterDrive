#[macro_use]
mod common;

use actix_web::test;
use serde_json::Value;

macro_rules! fetch_audit_items {
    ($app:expr, $token:expr) => {{
        let req = test::TestRequest::get()
            .uri("/api/v1/admin/audit-logs")
            .insert_header(("Cookie", format!("aster_access={}", $token)))
            .to_request();
        let resp = test::call_service(&$app, req).await;
        assert_eq!(resp.status(), 200);
        let body: Value = test::read_body_json(resp).await;
        body["data"]["items"]
            .as_array()
            .expect("audit log response should contain items")
            .clone()
    }};
}

fn assert_action_present<'a>(items: &'a [Value], action: &str) -> &'a Value {
    items
        .iter()
        .find(|item| item["action"] == action)
        .unwrap_or_else(|| {
            panic!(
                "audit log should contain {action}, got {:?}",
                items
                    .iter()
                    .map(|item| item["action"].as_str().unwrap_or("<non-string>"))
                    .collect::<Vec<_>>()
            )
        })
}

#[actix_web::test]
async fn test_audit_log_recorded_on_upload() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    // Upload a file — this triggers a "file_upload" audit log entry
    let _file_id = upload_test_file!(app, token);

    // Admin queries audit logs
    let req = test::TestRequest::get()
        .uri("/api/v1/admin/audit-logs")
        .insert_header(("Cookie", format!("aster_access={}", token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["code"], 0);

    let items = body["data"]["items"].as_array().unwrap();
    let has_upload = items.iter().any(|item| item["action"] == "file_upload");
    assert!(
        has_upload,
        "audit log should contain a file_upload entry, got: {:?}",
        items.iter().map(|i| &i["action"]).collect::<Vec<_>>()
    );
}

#[actix_web::test]
async fn test_audit_log_recorded_on_admin_create_user() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let req = test::TestRequest::post()
        .uri("/api/v1/admin/users")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "username": "audituser",
            "email": "audituser@example.com",
            "password": "password123"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);

    let req = test::TestRequest::get()
        .uri("/api/v1/admin/audit-logs")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    let items = body["data"]["items"].as_array().unwrap();
    let entry = items
        .iter()
        .find(|item| item["action"] == "admin_create_user");
    assert!(
        entry.is_some(),
        "audit log should contain admin_create_user"
    );
    let entry = entry.unwrap();
    assert_eq!(entry["entity_type"], "user");
    assert_eq!(entry["entity_name"], "audituser");
}

#[actix_web::test]
async fn test_audit_log_pagination_fields_and_offset() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    for _ in 0..3 {
        let _file_id = upload_test_file!(app, token);
    }

    let req = test::TestRequest::get()
        .uri("/api/v1/admin/audit-logs?limit=1&offset=1")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;

    assert_eq!(body["data"]["limit"], 1);
    assert_eq!(body["data"]["offset"], 1);
    assert!(body["data"]["total"].as_u64().unwrap() >= 3);
    assert_eq!(body["data"]["items"].as_array().unwrap().len(), 1);
}

#[actix_web::test]
async fn test_audit_log_limit_is_clamped() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let _file_id = upload_test_file!(app, token);

    let req = test::TestRequest::get()
        .uri("/api/v1/admin/audit-logs?limit=9999")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;

    assert_eq!(body["data"]["limit"], 200);
    assert_eq!(body["data"]["offset"], 0);
}

#[actix_web::test]
async fn test_audit_log_admin_only() {
    let state = common::setup().await;
    let app = create_test_app!(state);

    // First registered user is admin
    let (_admin_token, _) = register_and_login!(app);

    // Register a second non-admin user
    let req = test::TestRequest::post()
        .uri("/api/v1/auth/register")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "username": "user2",
            "email": "user2@example.com",
            "password": "password123"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);

    // Login as the non-admin user
    let req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "identifier": "user2",
            "password": "password123"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let token2 = common::extract_cookie(&resp, "aster_access").unwrap();

    // Non-admin tries to access audit logs — should get 403
    let req = test::TestRequest::get()
        .uri("/api/v1/admin/audit-logs")
        .insert_header(("Cookie", format!("aster_access={}", token2)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 403);
}

#[actix_web::test]
async fn test_audit_log_recorded_on_setup_register_and_login_after_refactor() {
    let state = common::setup().await;
    let app = create_test_app!(state);

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/setup")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "username": "setupadmin",
            "email": "setupadmin@example.com",
            "password": "password123"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "identifier": "setupadmin",
            "password": "password123"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let token = common::extract_cookie(&resp, "aster_access").unwrap();

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/register")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "username": "member1",
            "email": "member1@example.com",
            "password": "password123"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);

    let items = fetch_audit_items!(app, token);

    let setup_entry = assert_action_present(&items, "system_setup");
    assert_eq!(setup_entry["entity_name"], "setupadmin");

    let login_entry = assert_action_present(&items, "user_login");
    assert_eq!(login_entry["entity_name"], "setupadmin");

    let register_entry = assert_action_present(&items, "user_register");
    assert_eq!(register_entry["entity_name"], "member1");
}

#[actix_web::test]
async fn test_audit_log_recorded_on_file_and_folder_patch_variants_after_refactor() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let req = test::TestRequest::post()
        .uri("/api/v1/folders")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({ "name": "Source Folder" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let source_folder_id = body["data"]["id"].as_i64().unwrap();

    let req = test::TestRequest::post()
        .uri("/api/v1/folders")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({ "name": "Target Folder" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let target_folder_id = body["data"]["id"].as_i64().unwrap();

    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/folders/{source_folder_id}"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({ "name": "Renamed Folder" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/folders/{source_folder_id}"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({ "parent_id": target_folder_id }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let file_id = upload_test_file_named!(app, token, "audit-file.txt");

    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/files/{file_id}"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({ "name": "renamed-file.txt" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/files/{file_id}"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({ "folder_id": target_folder_id }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let items = fetch_audit_items!(app, token);

    assert_eq!(
        assert_action_present(&items, "folder_rename")["entity_type"],
        "folder"
    );
    assert_eq!(
        assert_action_present(&items, "folder_move")["entity_type"],
        "folder"
    );
    assert_eq!(
        assert_action_present(&items, "file_rename")["entity_type"],
        "file"
    );
    assert_eq!(
        assert_action_present(&items, "file_move")["entity_type"],
        "file"
    );
}

#[actix_web::test]
async fn test_audit_log_recorded_on_batch_actions_after_refactor() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let req = test::TestRequest::post()
        .uri("/api/v1/folders")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({ "name": "Batch Target" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let target_folder_id = body["data"]["id"].as_i64().unwrap();

    let file_to_copy = upload_test_file_named!(app, token, "copy-me.txt");
    let file_to_move = upload_test_file_named!(app, token, "move-me.txt");
    let file_to_delete = upload_test_file_named!(app, token, "delete-me.txt");

    let req = test::TestRequest::post()
        .uri("/api/v1/batch/copy")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "file_ids": [file_to_copy],
            "folder_ids": [],
            "target_folder_id": target_folder_id
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::post()
        .uri("/api/v1/batch/move")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "file_ids": [file_to_move],
            "folder_ids": [],
            "target_folder_id": target_folder_id
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::post()
        .uri("/api/v1/batch/delete")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "file_ids": [file_to_delete],
            "folder_ids": []
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let items = fetch_audit_items!(app, token);
    assert_action_present(&items, "batch_copy");
    assert_action_present(&items, "batch_move");
    assert_action_present(&items, "batch_delete");
}

#[actix_web::test]
async fn test_audit_log_recorded_on_share_config_and_admin_user_actions_after_refactor() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let file_id = upload_test_file!(app, token);

    let req = test::TestRequest::post()
        .uri("/api/v1/shares")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({ "file_id": file_id }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let share_id = body["data"]["id"].as_i64().unwrap();

    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/shares/{share_id}"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "expires_at": "2026-04-02T12:00:00Z",
            "max_downloads": 3
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/shares/{share_id}"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::put()
        .uri("/api/v1/admin/config/max_versions_per_file")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({ "value": "25" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::post()
        .uri("/api/v1/admin/users")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "username": "managed-user",
            "email": "managed-user@example.com",
            "password": "password123"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let managed_user_id = body["data"]["id"].as_i64().unwrap();

    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/admin/users/{managed_user_id}"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "status": "disabled",
            "storage_quota": 1024
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let items = fetch_audit_items!(app, token);

    assert_action_present(&items, "share_create");
    assert_action_present(&items, "share_update");
    assert_action_present(&items, "share_delete");

    let config_entry = assert_action_present(&items, "config_update");
    assert_eq!(config_entry["entity_name"], "max_versions_per_file");

    let create_entry = assert_action_present(&items, "admin_create_user");
    assert_eq!(create_entry["entity_type"], "user");
    assert_eq!(create_entry["entity_name"], "managed-user");

    let update_entry = assert_action_present(&items, "admin_update_user");
    assert_eq!(update_entry["entity_type"], "user");
    assert_eq!(update_entry["entity_name"], "managed-user");
}
