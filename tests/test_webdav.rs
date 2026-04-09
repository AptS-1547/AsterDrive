#[macro_use]
mod common;

use actix_web::test;
use base64::Engine;

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
async fn test_webdav_runtime_toggle_takes_effect_immediately() {
    use actix_web::{App, web};
    use serde_json::Value;

    let state = common::setup().await;
    let db1 = state.db.clone();
    let db2 = state.db.clone();
    let webdav_config = aster_drive::config::WebDavConfig::default();
    let app = test::init_service(
        App::new()
            .app_data(web::PayloadConfig::new(10 * 1024 * 1024))
            .app_data(web::JsonConfig::default().limit(1024 * 1024))
            .app_data(web::Data::new(state))
            .configure(move |cfg| {
                aster_drive::webdav::configure(cfg, &webdav_config, &db2);
                aster_drive::api::configure(cfg, &db1);
            }),
    )
    .await;

    let (token, _) = register_and_login!(app);
    let auth = format!("Bearer {token}");

    let req = test::TestRequest::with_uri("/webdav/")
        .method(actix_web::http::Method::from_bytes(b"PROPFIND").unwrap())
        .insert_header(("Authorization", auth.clone()))
        .insert_header(("Depth", "0"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 207);

    let req = test::TestRequest::put()
        .uri("/api/v1/admin/config/webdav_enabled")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({ "value": "false" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["key"], "webdav_enabled");
    assert_eq!(body["data"]["value"], "false");

    let req = test::TestRequest::with_uri("/webdav/")
        .method(actix_web::http::Method::from_bytes(b"PROPFIND").unwrap())
        .insert_header(("Authorization", auth.clone()))
        .insert_header(("Depth", "0"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 503);

    let req = test::TestRequest::put()
        .uri("/api/v1/admin/config/webdav_enabled")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({ "value": "true" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::with_uri("/webdav/")
        .method(actix_web::http::Method::from_bytes(b"PROPFIND").unwrap())
        .insert_header(("Authorization", auth))
        .insert_header(("Depth", "0"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 207);
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
async fn test_webdav_copy_folder_recursively() {
    let app = setup_with_webdav!();
    let (token, _) = register_and_login!(app);
    let auth = format!("Bearer {token}");

    let req = test::TestRequest::with_uri("/webdav/srcdir/")
        .method(actix_web::http::Method::from_bytes(b"MKCOL").unwrap())
        .insert_header(("Authorization", auth.clone()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);

    let req = test::TestRequest::with_uri("/webdav/srcdir/sub/")
        .method(actix_web::http::Method::from_bytes(b"MKCOL").unwrap())
        .insert_header(("Authorization", auth.clone()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);

    let req = test::TestRequest::put()
        .uri("/webdav/srcdir/sub/nested.txt")
        .insert_header(("Authorization", auth.clone()))
        .set_payload("recursive copy content")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status() == 201 || resp.status() == 204);

    let req = test::TestRequest::with_uri("/webdav/srcdir/")
        .method(actix_web::http::Method::from_bytes(b"COPY").unwrap())
        .insert_header(("Authorization", auth.clone()))
        .insert_header(("Destination", "/webdav/copied-dir/"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status() == 201 || resp.status() == 204,
        "COPY folder should return 201/204, got {}",
        resp.status()
    );

    let req = test::TestRequest::get()
        .uri("/webdav/copied-dir/sub/nested.txt")
        .insert_header(("Authorization", auth.clone()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body = test::read_body(resp).await;
    assert_eq!(String::from_utf8_lossy(&body), "recursive copy content");
}

#[actix_web::test]
async fn test_webdav_move_overwrites_existing_destination() {
    let app = setup_with_webdav!();
    let (token, _) = register_and_login!(app);
    let auth = format!("Bearer {token}");

    for (path, content) in [
        ("/webdav/source-overwrite.txt", "fresh content"),
        ("/webdav/existing-target.txt", "stale content"),
    ] {
        let req = test::TestRequest::put()
            .uri(path)
            .insert_header(("Authorization", auth.clone()))
            .set_payload(content)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status() == 201 || resp.status() == 204);
    }

    let req = test::TestRequest::with_uri("/webdav/source-overwrite.txt")
        .method(actix_web::http::Method::from_bytes(b"MOVE").unwrap())
        .insert_header(("Authorization", auth.clone()))
        .insert_header(("Destination", "/webdav/existing-target.txt"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status() == 201 || resp.status() == 204,
        "MOVE overwrite should return 201/204, got {}",
        resp.status()
    );

    let req = test::TestRequest::get()
        .uri("/webdav/source-overwrite.txt")
        .insert_header(("Authorization", auth.clone()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 404);

    let req = test::TestRequest::get()
        .uri("/webdav/existing-target.txt")
        .insert_header(("Authorization", auth.clone()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body = test::read_body(resp).await;
    assert_eq!(String::from_utf8_lossy(&body), "fresh content");
}

#[actix_web::test]
async fn test_webdav_propfind_hides_hidden_artifacts() {
    let app = setup_with_webdav!();
    let (token, _) = register_and_login!(app);
    let auth = format!("Bearer {token}");

    for path in [
        "/webdav/._hidden",
        "/webdav/.DS_Store",
        "/webdav/visible.txt",
    ] {
        let req = test::TestRequest::put()
            .uri(path)
            .insert_header(("Authorization", auth.clone()))
            .set_payload("artifact")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status() == 201 || resp.status() == 204);
    }

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
        xml.contains("visible.txt"),
        "visible file should be listed: {xml}"
    );
    assert!(
        !xml.contains("._hidden"),
        "._hidden should be filtered out: {xml}"
    );
    assert!(
        !xml.contains(".DS_Store"),
        ".DS_Store should be filtered out: {xml}"
    );
}

#[actix_web::test]
async fn test_webdav_copy_overwrites_existing_destination() {
    let app = setup_with_webdav!();
    let (token, _) = register_and_login!(app);
    let auth = format!("Bearer {token}");

    for (path, content) in [
        ("/webdav/source-copy.txt", "copy fresh"),
        ("/webdav/existing-copy-target.txt", "copy stale"),
    ] {
        let req = test::TestRequest::put()
            .uri(path)
            .insert_header(("Authorization", auth.clone()))
            .set_payload(content)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status() == 201 || resp.status() == 204);
    }

    let req = test::TestRequest::with_uri("/webdav/source-copy.txt")
        .method(actix_web::http::Method::from_bytes(b"COPY").unwrap())
        .insert_header(("Authorization", auth.clone()))
        .insert_header(("Destination", "/webdav/existing-copy-target.txt"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status() == 201 || resp.status() == 204,
        "COPY overwrite should return 201/204, got {}",
        resp.status()
    );

    let req = test::TestRequest::get()
        .uri("/webdav/source-copy.txt")
        .insert_header(("Authorization", auth.clone()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::get()
        .uri("/webdav/existing-copy-target.txt")
        .insert_header(("Authorization", auth.clone()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body = test::read_body(resp).await;
    assert_eq!(String::from_utf8_lossy(&body), "copy fresh");
}

#[actix_web::test]
async fn test_webdav_custom_property_roundtrip() {
    let app = setup_with_webdav!();
    let (token, _) = register_and_login!(app);
    let auth = format!("Bearer {token}");

    let req = test::TestRequest::put()
        .uri("/webdav/props.txt")
        .insert_header(("Authorization", auth.clone()))
        .set_payload("props")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status() == 201 || resp.status() == 204);

    let set_body = r#"<?xml version="1.0" encoding="utf-8" ?>
<D:propertyupdate xmlns:D="DAV:" xmlns:A="urn:aster:">
  <D:set>
    <D:prop>
      <A:color>blue</A:color>
    </D:prop>
  </D:set>
</D:propertyupdate>"#;
    let req = test::TestRequest::with_uri("/webdav/props.txt")
        .method(actix_web::http::Method::from_bytes(b"PROPPATCH").unwrap())
        .insert_header(("Authorization", auth.clone()))
        .insert_header(("Content-Type", "application/xml"))
        .set_payload(set_body)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 207);

    let propfind_body = r#"<?xml version="1.0" encoding="utf-8" ?>
<D:propfind xmlns:D="DAV:" xmlns:A="urn:aster:">
  <D:prop>
    <A:color />
  </D:prop>
</D:propfind>"#;
    let req = test::TestRequest::with_uri("/webdav/props.txt")
        .method(actix_web::http::Method::from_bytes(b"PROPFIND").unwrap())
        .insert_header(("Authorization", auth.clone()))
        .insert_header(("Depth", "0"))
        .insert_header(("Content-Type", "application/xml"))
        .set_payload(propfind_body)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 207);
    let body = test::read_body(resp).await;
    let xml = String::from_utf8_lossy(&body);
    assert!(
        xml.contains("blue"),
        "custom property value should roundtrip: {xml}"
    );

    let remove_body = r#"<?xml version="1.0" encoding="utf-8" ?>
<D:propertyupdate xmlns:D="DAV:" xmlns:A="urn:aster:">
  <D:remove>
    <D:prop>
      <A:color />
    </D:prop>
  </D:remove>
</D:propertyupdate>"#;
    let req = test::TestRequest::with_uri("/webdav/props.txt")
        .method(actix_web::http::Method::from_bytes(b"PROPPATCH").unwrap())
        .insert_header(("Authorization", auth.clone()))
        .insert_header(("Content-Type", "application/xml"))
        .set_payload(remove_body)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 207);

    let req = test::TestRequest::with_uri("/webdav/props.txt")
        .method(actix_web::http::Method::from_bytes(b"PROPFIND").unwrap())
        .insert_header(("Authorization", auth.clone()))
        .insert_header(("Depth", "0"))
        .insert_header(("Content-Type", "application/xml"))
        .set_payload(propfind_body)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 207);
    let body = test::read_body(resp).await;
    let xml = String::from_utf8_lossy(&body);
    assert!(
        !xml.contains(">blue<"),
        "removed property should be absent: {xml}"
    );
}

#[actix_web::test]
async fn test_webdav_proppatch_rejects_dav_namespace_changes() {
    let app = setup_with_webdav!();
    let (token, _) = register_and_login!(app);
    let auth = format!("Bearer {token}");

    let req = test::TestRequest::put()
        .uri("/webdav/dav-props.txt")
        .insert_header(("Authorization", auth.clone()))
        .set_payload("dav")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status() == 201 || resp.status() == 204);

    let body = r#"<?xml version="1.0" encoding="utf-8" ?>
<D:propertyupdate xmlns:D="DAV:">
  <D:set>
    <D:prop>
      <D:displayname>blocked</D:displayname>
    </D:prop>
  </D:set>
</D:propertyupdate>"#;
    let req = test::TestRequest::with_uri("/webdav/dav-props.txt")
        .method(actix_web::http::Method::from_bytes(b"PROPPATCH").unwrap())
        .insert_header(("Authorization", auth.clone()))
        .insert_header(("Content-Type", "application/xml"))
        .set_payload(body)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 207);
    let body = test::read_body(resp).await;
    let xml = String::from_utf8_lossy(&body);
    assert!(
        xml.contains("403") || xml.contains("Forbidden"),
        "DAV namespace writes should be rejected: {xml}"
    );
}

#[actix_web::test]
async fn test_webdav_basic_auth_root_scope() {
    let app = setup_with_webdav!();
    let (token, _) = register_and_login!(app);

    let req = test::TestRequest::post()
        .uri("/api/v1/folders")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({ "name": "scope-root" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    let root_id = body["data"]["id"].as_i64().unwrap();

    let req = test::TestRequest::post()
        .uri("/api/v1/folders")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({ "name": "inside", "parent_id": root_id }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);

    let req = test::TestRequest::post()
        .uri("/api/v1/folders")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({ "name": "outside" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);

    let req = test::TestRequest::post()
        .uri("/api/v1/webdav-accounts")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "username": "basic-scope-user",
            "password": "basic-scope-pass",
            "root_folder_id": root_id,
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);

    let basic = format!(
        "Basic {}",
        base64::engine::general_purpose::STANDARD.encode("basic-scope-user:basic-scope-pass")
    );

    let req = test::TestRequest::with_uri("/webdav/")
        .method(actix_web::http::Method::from_bytes(b"PROPFIND").unwrap())
        .insert_header(("Authorization", basic.clone()))
        .insert_header(("Depth", "1"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 207);
    let body = test::read_body(resp).await;
    let xml = String::from_utf8_lossy(&body);
    assert!(xml.contains("inside"));
    assert!(!xml.contains("outside"));

    let req = test::TestRequest::get()
        .uri("/webdav/outside/")
        .insert_header(("Authorization", basic.clone()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_client_error());
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
    assert_eq!(
        resp.status(),
        200,
        "LOCK should return 200, got {}",
        resp.status()
    );

    // 提取 Lock-Token header
    let lock_token = resp
        .headers()
        .get("Lock-Token")
        .map(|v| v.to_str().unwrap_or("").to_string())
        .unwrap_or_default();
    assert!(
        !lock_token.is_empty(),
        "Lock-Token header should be present"
    );

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

#[actix_web::test]
async fn test_webdav_bearer_rejects_refresh_token() {
    let app = setup_with_webdav!();
    let (_access, refresh) = register_and_login!(app);

    let req = test::TestRequest::with_uri("/webdav/")
        .method(actix_web::http::Method::from_bytes(b"PROPFIND").unwrap())
        .insert_header(("Authorization", format!("Bearer {refresh}")))
        .insert_header(("Depth", "0"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 401);
}

#[actix_web::test]
async fn test_webdav_bearer_respects_session_revocation() {
    let (app, mail_sender) = setup_with_webdav_and_mail!();
    let (admin_token, _) = register_and_login!(app);

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/register")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "username": "webdavrevoke",
            "email": "webdavrevoke@example.com",
            "password": "password123"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: serde_json::Value = test::read_body_json(resp).await;
    let user_id = body["data"]["id"].as_i64().unwrap();

    let _ = confirm_latest_contact_verification!(app, mail_sender);

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "identifier": "webdavrevoke",
            "password": "password123"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let user_access = common::extract_cookie(&resp, "aster_access").unwrap();

    let req = test::TestRequest::with_uri("/webdav/")
        .method(actix_web::http::Method::from_bytes(b"PROPFIND").unwrap())
        .insert_header(("Authorization", format!("Bearer {user_access}")))
        .insert_header(("Depth", "0"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 207);

    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/admin/users/{user_id}/sessions/revoke"))
        .insert_header(("Cookie", format!("aster_access={admin_token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::with_uri("/webdav/")
        .method(actix_web::http::Method::from_bytes(b"PROPFIND").unwrap())
        .insert_header(("Authorization", format!("Bearer {user_access}")))
        .insert_header(("Depth", "0"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 401);
}
