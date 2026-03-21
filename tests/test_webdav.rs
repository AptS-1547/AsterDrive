#[macro_use]
mod common;

use actix_web::test;

#[actix_web::test]
async fn test_webdav_propfind_root() {
    let app = setup_with_webdav!();

    let (token, _) = register_and_login!(app);

    // PROPFIND 根目录 (Depth: 0)
    let req = test::TestRequest::with_uri("/webdav/")
        .method(actix_web::http::Method::from_bytes(b"PROPFIND").unwrap())
        .insert_header(("Authorization", format!("Bearer {token}")))
        .insert_header(("Depth", "0"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 207, "PROPFIND root should return 207");
}

#[actix_web::test]
async fn test_webdav_mkcol_and_list() {
    let app = setup_with_webdav!();

    let (token, _) = register_and_login!(app);
    let auth = format!("Bearer {token}");

    // MKCOL 创建目录
    let req = test::TestRequest::with_uri("/webdav/testdir/")
        .method(actix_web::http::Method::from_bytes(b"MKCOL").unwrap())
        .insert_header(("Authorization", auth.clone()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201, "MKCOL should return 201");

    // PROPFIND 根目录 (Depth: 1) — 应包含 testdir
    let req = test::TestRequest::with_uri("/webdav/")
        .method(actix_web::http::Method::from_bytes(b"PROPFIND").unwrap())
        .insert_header(("Authorization", auth.clone()))
        .insert_header(("Depth", "1"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 207);
    let body = test::read_body(resp).await;
    let xml = String::from_utf8_lossy(&body);
    assert!(
        xml.contains("testdir"),
        "PROPFIND should list testdir: {xml}"
    );
}

#[actix_web::test]
async fn test_webdav_put_get_delete() {
    let app = setup_with_webdav!();

    let (token, _) = register_and_login!(app);
    let auth = format!("Bearer {token}");

    // PUT 上传文件
    let req = test::TestRequest::put()
        .uri("/webdav/hello.txt")
        .insert_header(("Authorization", auth.clone()))
        .insert_header(("Content-Type", "text/plain"))
        .set_payload("WebDAV test content")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status() == 201 || resp.status() == 204,
        "PUT should return 201 or 204, got {}",
        resp.status()
    );

    // GET 下载文件
    let req = test::TestRequest::get()
        .uri("/webdav/hello.txt")
        .insert_header(("Authorization", auth.clone()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200, "GET should return 200");
    let body = test::read_body(resp).await;
    assert!(
        String::from_utf8_lossy(&body).contains("WebDAV test content"),
        "GET content mismatch"
    );

    // DELETE 删除文件
    let req = test::TestRequest::delete()
        .uri("/webdav/hello.txt")
        .insert_header(("Authorization", auth.clone()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status() == 200 || resp.status() == 204,
        "DELETE should return 200 or 204, got {}",
        resp.status()
    );

    // GET 应该 404
    let req = test::TestRequest::get()
        .uri("/webdav/hello.txt")
        .insert_header(("Authorization", auth.clone()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 404);
}

#[actix_web::test]
async fn test_webdav_copy_move() {
    let app = setup_with_webdav!();

    let (token, _) = register_and_login!(app);
    let auth = format!("Bearer {token}");

    // PUT 创建源文件
    let req = test::TestRequest::put()
        .uri("/webdav/source.txt")
        .insert_header(("Authorization", auth.clone()))
        .set_payload("copy me")
        .to_request();
    test::call_service(&app, req).await;

    // COPY 复制文件
    let req = test::TestRequest::with_uri("/webdav/source.txt")
        .method(actix_web::http::Method::from_bytes(b"COPY").unwrap())
        .insert_header(("Authorization", auth.clone()))
        .insert_header(("Destination", "/webdav/copied.txt"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status() == 201 || resp.status() == 204,
        "COPY should return 201/204, got {}",
        resp.status()
    );

    // 验证副本存在
    let req = test::TestRequest::get()
        .uri("/webdav/copied.txt")
        .insert_header(("Authorization", auth.clone()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // MOVE 移动文件
    let req = test::TestRequest::with_uri("/webdav/source.txt")
        .method(actix_web::http::Method::from_bytes(b"MOVE").unwrap())
        .insert_header(("Authorization", auth.clone()))
        .insert_header(("Destination", "/webdav/moved.txt"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status() == 201 || resp.status() == 204,
        "MOVE should return 201/204, got {}",
        resp.status()
    );

    // 原文件不存在
    let req = test::TestRequest::get()
        .uri("/webdav/source.txt")
        .insert_header(("Authorization", auth.clone()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 404);

    // 新位置存在
    let req = test::TestRequest::get()
        .uri("/webdav/moved.txt")
        .insert_header(("Authorization", auth.clone()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

#[actix_web::test]
async fn test_webdav_options() {
    let app = setup_with_webdav!();

    let (token, _) = register_and_login!(app);

    // OPTIONS 应返回 DAV header
    let req = test::TestRequest::with_uri("/webdav/")
        .method(actix_web::http::Method::OPTIONS)
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let dav_header = resp
        .headers()
        .get("DAV")
        .map(|v| v.to_str().unwrap_or(""))
        .unwrap_or("");
    assert!(
        dav_header.contains("1"),
        "DAV header should contain '1', got: '{dav_header}'"
    );
}

#[actix_web::test]
async fn test_webdav_lock_unlock() {
    let app = setup_with_webdav!();
    let (token, _) = register_and_login!(app);
    let auth = format!("Bearer {token}");

    // PUT 创建文件
    let req = test::TestRequest::put()
        .uri("/webdav/lockme.txt")
        .insert_header(("Authorization", auth.clone()))
        .set_payload("lock test")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status() == 201 || resp.status() == 204);

    // LOCK 文件
    let lock_body = r#"<?xml version="1.0" encoding="utf-8" ?>
<D:lockinfo xmlns:D="DAV:">
  <D:lockscope><D:exclusive/></D:lockscope>
  <D:locktype><D:write/></D:locktype>
  <D:owner><D:href>testuser</D:href></D:owner>
</D:lockinfo>"#;

    let req = test::TestRequest::with_uri("/webdav/lockme.txt")
        .method(actix_web::http::Method::from_bytes(b"LOCK").unwrap())
        .insert_header(("Authorization", auth.clone()))
        .insert_header(("Content-Type", "application/xml"))
        .insert_header(("Timeout", "Second-3600"))
        .set_payload(lock_body)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200, "LOCK should return 200, got {}", resp.status());

    // 提取 Lock-Token header
    let lock_token = resp
        .headers()
        .get("Lock-Token")
        .map(|v| v.to_str().unwrap_or("").to_string())
        .unwrap_or_default();
    assert!(!lock_token.is_empty(), "Lock-Token header should be present");

    // 删除应该失败（被锁了，没提交 token）
    let req = test::TestRequest::delete()
        .uri("/webdav/lockme.txt")
        .insert_header(("Authorization", auth.clone()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status() == 423 || resp.status() == 403,
        "DELETE locked file should fail, got {}",
        resp.status()
    );

    // UNLOCK
    let req = test::TestRequest::with_uri("/webdav/lockme.txt")
        .method(actix_web::http::Method::from_bytes(b"UNLOCK").unwrap())
        .insert_header(("Authorization", auth.clone()))
        .insert_header(("Lock-Token", lock_token))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status() == 200 || resp.status() == 204,
        "UNLOCK should return 200/204, got {}",
        resp.status()
    );

    // 解锁后删除应该成功
    let req = test::TestRequest::delete()
        .uri("/webdav/lockme.txt")
        .insert_header(("Authorization", auth.clone()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status() == 200 || resp.status() == 204,
        "DELETE after unlock should succeed, got {}",
        resp.status()
    );
}

#[actix_web::test]
async fn test_webdav_unauthorized() {
    let app = setup_with_webdav!();

    // 无认证访问 WebDAV
    let req = test::TestRequest::with_uri("/webdav/")
        .method(actix_web::http::Method::from_bytes(b"PROPFIND").unwrap())
        .insert_header(("Depth", "0"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 401);
}
