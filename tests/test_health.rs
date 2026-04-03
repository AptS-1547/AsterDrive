#[macro_use]
mod common;

use actix_web::test;
use aster_drive::api::error_code::ErrorCode;
use serde_json::Value;

#[actix_web::test]
async fn test_health() {
    let state = common::setup().await;
    let app = create_test_app!(state);

    let req = test::TestRequest::get().uri("/health").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["status"], "ok");
}

#[actix_web::test]
async fn test_health_ready() {
    let state = common::setup().await;
    let app = create_test_app!(state);

    let req = test::TestRequest::get().uri("/health/ready").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["status"], "ready");
}

#[actix_web::test]
async fn test_health_ready_redacts_database_error() {
    let state = common::setup().await;
    let db = state.db.clone();
    let app = create_test_app!(state);

    db.close_by_ref().await.unwrap();

    let req = test::TestRequest::get().uri("/health/ready").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 503);

    let body: Value = test::read_body_json(resp).await;
    assert_eq!(
        body["code"],
        serde_json::json!(ErrorCode::DatabaseError as i32)
    );
    assert_eq!(body["msg"], "Database unavailable");
}
