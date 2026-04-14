//! Background task integration tests

#[macro_use]
mod common;

use actix_web::{App, test, web};
use serde_json::Value;
use std::io::{Cursor, Read};

macro_rules! register_user {
    ($app:expr, $db:expr, $mail_sender:expr, $username:expr, $email:expr, $password:expr) => {{
        let req = test::TestRequest::post()
            .uri("/api/v1/auth/register")
            .peer_addr("127.0.0.1:12345".parse().unwrap())
            .set_json(serde_json::json!({
                "username": $username,
                "email": $email,
                "password": $password
            }))
            .to_request();
        let resp = test::call_service(&$app, req).await;
        assert_eq!(resp.status(), 201);
        let _body: Value = test::read_body_json(resp).await;
        let _ = confirm_latest_contact_verification!($app, $db, $mail_sender);
    }};
}

macro_rules! login_user {
    ($app:expr, $identifier:expr, $password:expr) => {{
        let req = test::TestRequest::post()
            .uri("/api/v1/auth/login")
            .peer_addr("127.0.0.1:12345".parse().unwrap())
            .set_json(serde_json::json!({
                "identifier": $identifier,
                "password": $password
            }))
            .to_request();
        let resp = test::call_service(&$app, req).await;
        assert_eq!(resp.status(), 200);
        common::extract_cookie(&resp, "aster_access").unwrap()
    }};
}

macro_rules! multipart_request {
    ($uri:expr, $token:expr, $filename:expr, $content:expr $(,)?) => {{
        let boundary = "----TaskBoundary123";
        let payload = format!(
            "------TaskBoundary123\r\n\
             Content-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\n\
             Content-Type: text/plain\r\n\r\n\
             {content}\r\n\
             ------TaskBoundary123--\r\n",
            filename = $filename,
            content = $content,
        );

        test::TestRequest::post()
            .uri($uri)
            .insert_header(("Cookie", common::access_cookie_header(&$token)))
            .insert_header(common::csrf_header_for(&$token))
            .insert_header((
                "Content-Type",
                format!("multipart/form-data; boundary={boundary}"),
            ))
            .set_payload(payload)
            .to_request()
    }};
}

fn zip_entry_names(bytes: &[u8]) -> Vec<String> {
    let mut archive =
        zip::ZipArchive::new(Cursor::new(bytes.to_vec())).expect("zip archive should be readable");
    let mut names = Vec::with_capacity(archive.len());
    for index in 0..archive.len() {
        names.push(
            archive
                .by_index(index)
                .expect("zip entry should exist")
                .name()
                .to_string(),
        );
    }
    names.sort();
    names
}

fn read_zip_entry_text(bytes: &[u8], name: &str) -> String {
    let mut archive =
        zip::ZipArchive::new(Cursor::new(bytes.to_vec())).expect("zip archive should be readable");
    let mut entry = archive.by_name(name).expect("zip entry should exist");
    let mut content = String::new();
    entry
        .read_to_string(&mut content)
        .expect("zip entry should be readable as utf-8 text");
    content
}

fn read_archive_download_path(body: &Value) -> String {
    body["data"]["token"]
        .as_str()
        .expect("ticket token should exist");
    body["data"]["download_path"]
        .as_str()
        .expect("download path should exist")
        .to_string()
}

async fn assert_response_status(
    resp: actix_web::dev::ServiceResponse,
    expected: actix_web::http::StatusCode,
) -> actix_web::dev::ServiceResponse {
    let status = resp.status();
    if status != expected {
        let body = test::read_body(resp).await;
        panic!(
            "expected status {}, got {} with body: {}",
            expected,
            status,
            String::from_utf8_lossy(&body)
        );
    }
    resp
}

#[actix_web::test]
async fn test_personal_archive_stream_preserves_empty_folders() {
    let state = common::setup().await;
    let db = state.db.clone();
    let mail_sender = state.mail_sender.clone();
    let state = web::Data::new(state);
    let app = test::init_service(
        App::new()
            .app_data(web::PayloadConfig::new(10 * 1024 * 1024))
            .app_data(web::JsonConfig::default().limit(1024 * 1024))
            .app_data(web::Data::clone(&state))
            .configure(move |cfg| aster_drive::api::configure(cfg, &db)),
    )
    .await;

    register_user!(
        app,
        state.db.clone(),
        mail_sender,
        "taskowner",
        "taskowner@example.com",
        "password123"
    );
    let token = login_user!(app, "taskowner", "password123");

    let req = test::TestRequest::post()
        .uri("/api/v1/folders")
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .set_json(serde_json::json!({ "name": "bundle", "parent_id": null }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let bundle_id = body["data"]["id"].as_i64().unwrap();

    let req = test::TestRequest::post()
        .uri("/api/v1/folders")
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .set_json(serde_json::json!({ "name": "docs", "parent_id": bundle_id }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let docs_id = body["data"]["id"].as_i64().unwrap();

    let req = test::TestRequest::post()
        .uri("/api/v1/folders")
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .set_json(serde_json::json!({ "name": "empty", "parent_id": bundle_id }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);

    let req = multipart_request!(
        &format!("/api/v1/files/upload?folder_id={docs_id}"),
        &token,
        "note.txt",
        "hello from archive task",
    );
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);

    let req = test::TestRequest::post()
        .uri("/api/v1/batch/archive-download")
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .set_json(serde_json::json!({
            "file_ids": [],
            "folder_ids": [bundle_id],
            "archive_name": "bundle-export"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    let download_path = read_archive_download_path(&body);

    let req = test::TestRequest::get()
        .uri(&download_path)
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .to_request();
    let resp = assert_response_status(
        test::call_service(&app, req).await,
        actix_web::http::StatusCode::OK,
    )
    .await;
    assert_eq!(
        resp.headers()
            .get("Content-Type")
            .and_then(|value| value.to_str().ok()),
        Some("application/zip")
    );
    let zip_bytes = test::read_body(resp).await;
    let names = zip_entry_names(&zip_bytes);
    assert_eq!(
        names,
        vec![
            "bundle/",
            "bundle/docs/",
            "bundle/docs/note.txt",
            "bundle/empty/",
        ]
    );
    assert_eq!(
        read_zip_entry_text(&zip_bytes, "bundle/docs/note.txt"),
        "hello from archive task"
    );
}

#[actix_web::test]
async fn test_team_archive_stream_is_scoped_to_team_routes() {
    let state = common::setup().await;
    let db = state.db.clone();
    let mail_sender = state.mail_sender.clone();
    let state = web::Data::new(state);
    let app = test::init_service(
        App::new()
            .app_data(web::PayloadConfig::new(10 * 1024 * 1024))
            .app_data(web::JsonConfig::default().limit(1024 * 1024))
            .app_data(web::Data::clone(&state))
            .configure(move |cfg| aster_drive::api::configure(cfg, &db)),
    )
    .await;

    register_user!(
        app,
        state.db.clone(),
        mail_sender,
        "teamowner",
        "teamowner@example.com",
        "password123"
    );
    let token = login_user!(app, "teamowner", "password123");

    let req = test::TestRequest::post()
        .uri("/api/v1/teams")
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .set_json(serde_json::json!({ "name": "Ops Team" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let team_id = body["data"]["id"].as_i64().unwrap();

    let req = multipart_request!(
        &format!("/api/v1/teams/{team_id}/files/upload"),
        &token,
        "team.txt",
        "team archive payload",
    );
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let file_id = body["data"]["id"].as_i64().unwrap();

    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/teams/{team_id}/batch/archive-download"))
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .set_json(serde_json::json!({
            "file_ids": [file_id],
            "folder_ids": [],
            "archive_name": "ops-export"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    let download_path = read_archive_download_path(&body);

    let req = test::TestRequest::get()
        .uri(&download_path)
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .to_request();
    let resp = assert_response_status(
        test::call_service(&app, req).await,
        actix_web::http::StatusCode::OK,
    )
    .await;
    let zip_bytes = test::read_body(resp).await;
    assert_eq!(zip_entry_names(&zip_bytes), vec!["team.txt"]);
    assert_eq!(
        read_zip_entry_text(&zip_bytes, "team.txt"),
        "team archive payload"
    );

    let req = test::TestRequest::post()
        .uri("/api/v1/batch/archive-download")
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .set_json(serde_json::json!({
            "file_ids": [file_id],
            "folder_ids": [],
            "archive_name": "should-fail"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 403);
}
