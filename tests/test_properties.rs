#[macro_use]
mod common;

use actix_web::test;
use serde_json::Value;

#[actix_web::test]
async fn test_entity_properties() {
    let state = common::setup().await;
    let app = create_test_app!(state);

    let (token, _) = register_and_login!(app);
    let file_id = upload_test_file!(app, token);

    // 设置属性
    let req = test::TestRequest::put()
        .uri(&format!("/api/v1/properties/file/{file_id}"))
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .set_json(serde_json::json!({
            "namespace": "aster:",
            "name": "color",
            "value": "red"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["name"], "color");
    assert_eq!(body["data"]["value"], "red");

    // 列出属性
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/properties/file/{file_id}"))
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"].as_array().unwrap().len(), 1);

    // 删除属性
    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/properties/file/{file_id}/aster:/color"))
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // 列出为空
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/properties/file/{file_id}"))
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"].as_array().unwrap().len(), 0);

    // DAV: 命名空间被拒绝
    let req = test::TestRequest::put()
        .uri(&format!("/api/v1/properties/file/{file_id}"))
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .set_json(serde_json::json!({
            "namespace": "DAV:",
            "name": "getcontenttype",
            "value": "text/plain"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status() == 403 || resp.status() == 423);
}
