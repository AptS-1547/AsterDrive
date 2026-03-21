#[macro_use]
mod common;

use actix_web::test;
use serde_json::Value;

#[actix_web::test]
async fn test_folders_crud() {
    let state = common::setup().await;
    let app = create_test_app!(state);

    let (token, _) = register_and_login!(app);

    // 列出根目录（应为空）
    let req = test::TestRequest::get()
        .uri("/api/v1/folders")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["folders"], Value::Array(vec![]));
    assert_eq!(body["data"]["files"], Value::Array(vec![]));

    // 创建文件夹
    let req = test::TestRequest::post()
        .uri("/api/v1/folders")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({ "name": "Documents" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let folder_id = body["data"]["id"].as_i64().unwrap();
    assert_eq!(body["data"]["name"], "Documents");

    // 列出根目录（应有 1 个文件夹）
    let req = test::TestRequest::get()
        .uri("/api/v1/folders")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["folders"].as_array().unwrap().len(), 1);

    // 重命名文件夹
    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/folders/{folder_id}"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({ "name": "My Docs" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["name"], "My Docs");

    // 删除文件夹
    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/folders/{folder_id}"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

#[actix_web::test]
async fn test_folder_lock_unlock() {
    let state = common::setup().await;
    let app = create_test_app!(state);

    let (token, _) = register_and_login!(app);

    // 创建文件夹
    let req = test::TestRequest::post()
        .uri("/api/v1/folders")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({ "name": "Locked Folder" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let folder_id = body["data"]["id"].as_i64().unwrap();

    // 锁定
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/folders/{folder_id}/lock"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({ "locked": true }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // 删除失败
    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/folders/{folder_id}"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status() == 403 || resp.status() == 423);

    // 重命名失败
    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/folders/{folder_id}"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({ "name": "Nope" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status() == 403 || resp.status() == 423);

    // 解锁 → 删除成功
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/folders/{folder_id}/lock"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({ "locked": false }))
        .to_request();
    test::call_service(&app, req).await;

    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/folders/{folder_id}"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

#[actix_web::test]
async fn test_folder_copy() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    // 创建源文件夹 + 里面放个文件
    let req = test::TestRequest::post()
        .uri("/api/v1/folders")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({ "name": "Source" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let src_id = body["data"]["id"].as_i64().unwrap();

    let boundary = "----TestBoundary123";
    let payload = "------TestBoundary123\r\n\
         Content-Disposition: form-data; name=\"file\"; filename=\"inside.txt\"\r\n\
         Content-Type: text/plain\r\n\r\n\
         folder content\r\n\
         ------TestBoundary123--\r\n";
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/files/upload?folder_id={src_id}"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .insert_header((
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        ))
        .set_payload(payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);

    // 复制文件夹（同级 → "Source (1)"）
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/folders/{src_id}/copy"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({ "parent_id": null }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["name"], "Source (1)");
    let copy_id = body["data"]["id"].as_i64().unwrap();

    // 副本文件夹里应该有文件
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/folders/{copy_id}"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["files"].as_array().unwrap().len(), 1);
    assert_eq!(body["data"]["files"][0]["name"], "inside.txt");
}
