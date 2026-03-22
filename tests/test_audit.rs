#[macro_use]
mod common;

use actix_web::test;
use serde_json::Value;

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
            "username": "user2",
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
