//! 集成测试：`middleware`。

#[macro_use]
mod common;

use actix_web::{body::to_bytes, test};
use serde_json::Value;

#[actix_web::test]
async fn test_jwt_auth_missing_token_returns_api_error() {
    let state = common::setup().await;
    let app = create_test_app!(state);

    let req = test::TestRequest::get().uri("/api/v1/folders").to_request();
    let err = test::try_call_service(&app, req).await.unwrap_err();
    let resp = err.error_response();
    assert_eq!(resp.status(), 401);

    let body = to_bytes(resp.into_body()).await.unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body["code"], 2000);
    assert_eq!(body["msg"], "missing token");
    assert!(body["data"].is_null());
}

#[actix_web::test]
async fn test_jwt_auth_invalid_token_returns_api_error() {
    let state = common::setup().await;
    let app = create_test_app!(state);

    let req = test::TestRequest::get()
        .uri("/api/v1/folders")
        .insert_header(("Authorization", "Bearer fake.token.here"))
        .to_request();
    let err = test::try_call_service(&app, req).await.unwrap_err();
    let resp = err.error_response();
    assert_eq!(resp.status(), 401);

    let body = to_bytes(resp.into_body()).await.unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body["code"], 2002);
    assert_eq!(body["msg"], "invalid token");
    assert!(body["data"].is_null());
}
