#[macro_use]
mod common;

use actix_web::test;
use serde_json::Value;

#[actix_web::test]
async fn test_trash_restore_purge() {
    let state = common::setup().await;
    let app = create_test_app!(state);

    let (token, _) = register_and_login!(app);
    let file_id = upload_test_file!(app, token);

    // 软删除
    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/files/{file_id}"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // 列出回收站
    let req = test::TestRequest::get()
        .uri("/api/v1/trash")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["files"].as_array().unwrap().len(), 1);

    // 恢复
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/trash/file/{file_id}/restore"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // 文件可访问
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/files/{file_id}"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // 再次软删除 → purge 永久删除
    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/files/{file_id}"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    test::call_service(&app, req).await;

    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/trash/file/{file_id}"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // 回收站为空
    let req = test::TestRequest::get()
        .uri("/api/v1/trash")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["files"].as_array().unwrap().len(), 0);
}

#[actix_web::test]
async fn test_trash_purge_all() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    // 上传两个文件
    let f1 = upload_test_file!(app, token);
    // 第二个用不同名字
    let boundary = "----TestBoundary123";
    let payload = "------TestBoundary123\r\n\
         Content-Disposition: form-data; name=\"file\"; filename=\"second.txt\"\r\n\
         Content-Type: text/plain\r\n\r\n\
         second\r\n\
         ------TestBoundary123--\r\n";
    let req = test::TestRequest::post()
        .uri("/api/v1/files/upload")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .insert_header((
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        ))
        .set_payload(payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let f2 = body["data"]["id"].as_i64().unwrap();

    // 软删除两个
    for fid in [f1, f2] {
        let req = test::TestRequest::delete()
            .uri(&format!("/api/v1/files/{fid}"))
            .insert_header(("Cookie", format!("aster_access={token}")))
            .to_request();
        test::call_service(&app, req).await;
    }

    // 回收站有 2 个
    let req = test::TestRequest::get()
        .uri("/api/v1/trash")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["files"].as_array().unwrap().len(), 2);

    // purge all
    let req = test::TestRequest::delete()
        .uri("/api/v1/trash")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // 回收站为空
    let req = test::TestRequest::get()
        .uri("/api/v1/trash")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["files"].as_array().unwrap().len(), 0);
}
