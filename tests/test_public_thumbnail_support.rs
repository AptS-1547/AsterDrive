//! 集成测试：`public_thumbnail_support`。

#[macro_use]
mod common;

use actix_web::test;
use serde_json::{Value, json};

#[actix_web::test]
async fn test_public_thumbnail_support_returns_default_builtin_extensions() {
    let state = common::setup().await;
    let app = create_test_app!(state);

    let req = test::TestRequest::get()
        .uri("/api/v1/public/thumbnail-support")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["version"], 1);
    assert!(body["data"].get("mime_types").is_none());

    let extensions = body["data"]["extensions"]
        .as_array()
        .expect("extensions should be an array");
    assert!(extensions.iter().any(|value| value == "png"));
    assert!(extensions.iter().any(|value| value == "jpg"));
    assert!(extensions.iter().any(|value| value == "tiff"));
}

#[actix_web::test]
async fn test_public_thumbnail_support_merges_builtin_and_enabled_vips_extensions() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let req = test::TestRequest::put()
        .uri("/api/v1/admin/config/media_processing_registry_json")
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .set_json(json!({
            "value": json!({
                "version": 1,
                "processors": [
                    {
                        "kind": "vips_cli",
                        "enabled": true,
                        "extensions": ["HEIC", ".avif"],
                        "config": {
                            "command": "/bin/sh"
                        }
                    },
                    {
                        "kind": "images",
                        "enabled": true
                    }
                ]
            })
            .to_string()
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::get()
        .uri("/api/v1/public/thumbnail-support")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: Value = test::read_body_json(resp).await;
    let extensions = body["data"]["extensions"]
        .as_array()
        .expect("extensions should be an array");
    assert!(extensions.iter().any(|value| value == "png"));
    assert!(extensions.iter().any(|value| value == "heic"));
    assert!(extensions.iter().any(|value| value == "avif"));
}
