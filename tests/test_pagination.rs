#[macro_use]
mod common;

use actix_web::test;
use serde_json::Value;

/// Helper macro: create a folder in root or parent, return its ID
macro_rules! create_folder {
    ($app:expr, $token:expr, $name:expr) => {{
        let req = test::TestRequest::post()
            .uri("/api/v1/folders")
            .insert_header(("Cookie", format!("aster_access={}", $token)))
            .set_json(serde_json::json!({ "name": $name }))
            .to_request();
        let resp: actix_web::dev::ServiceResponse = test::call_service(&$app, req).await;
        assert_eq!(resp.status(), 201);
        let body: Value = test::read_body_json(resp).await;
        body["data"]["id"].as_i64().unwrap()
    }};
    ($app:expr, $token:expr, $name:expr, $parent_id:expr) => {{
        let req = test::TestRequest::post()
            .uri("/api/v1/folders")
            .insert_header(("Cookie", format!("aster_access={}", $token)))
            .set_json(serde_json::json!({ "name": $name, "parent_id": $parent_id }))
            .to_request();
        let resp: actix_web::dev::ServiceResponse = test::call_service(&$app, req).await;
        assert_eq!(resp.status(), 201);
        let body: Value = test::read_body_json(resp).await;
        body["data"]["id"].as_i64().unwrap()
    }};
}

#[actix_web::test]
async fn test_folder_list_pagination_defaults() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    // Create 3 folders and 5 files
    for i in 0..3 {
        create_folder!(app, token, format!("folder-{i:03}"));
    }
    for _ in 0..5 {
        upload_test_file!(app, token);
    }

    // Default request returns totals
    let req = test::TestRequest::get()
        .uri("/api/v1/folders")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["folders"].as_array().unwrap().len(), 3);
    assert_eq!(body["data"]["files"].as_array().unwrap().len(), 5);
    assert_eq!(body["data"]["folders_total"], 3);
    assert_eq!(body["data"]["files_total"], 5);
}

#[actix_web::test]
async fn test_folder_list_pagination_limit_offset() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    for i in 0..5 {
        create_folder!(app, token, format!("folder-{i:03}"));
    }
    for _ in 0..8 {
        upload_test_file!(app, token);
    }

    // Page 1: folder_limit=2, file_limit=3
    let req = test::TestRequest::get()
        .uri("/api/v1/folders?folder_limit=2&folder_offset=0&file_limit=3&file_offset=0")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["folders"].as_array().unwrap().len(), 2);
    assert_eq!(body["data"]["files"].as_array().unwrap().len(), 3);
    assert_eq!(body["data"]["folders_total"], 5);
    assert_eq!(body["data"]["files_total"], 8);

    // Page tail: offset near end
    let req = test::TestRequest::get()
        .uri("/api/v1/folders?folder_limit=2&folder_offset=4&file_limit=3&file_offset=6")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["folders"].as_array().unwrap().len(), 1); // 5th folder
    assert_eq!(body["data"]["files"].as_array().unwrap().len(), 2); // 7th + 8th file
    assert_eq!(body["data"]["folders_total"], 5);
    assert_eq!(body["data"]["files_total"], 8);
}

#[actix_web::test]
async fn test_folder_list_file_limit_zero_skips_files() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    for i in 0..3 {
        create_folder!(app, token, format!("folder-{i:03}"));
    }
    for _ in 0..5 {
        upload_test_file!(app, token);
    }

    // file_limit=0 should return no files but still show files_total
    let req = test::TestRequest::get()
        .uri("/api/v1/folders?file_limit=0")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["folders"].as_array().unwrap().len(), 3);
    assert_eq!(body["data"]["files"].as_array().unwrap().len(), 0);
    assert_eq!(body["data"]["folders_total"], 3);
    assert_eq!(body["data"]["files_total"], 5);
}

#[actix_web::test]
async fn test_subfolder_pagination() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let parent_id = create_folder!(app, token, "parent");

    // Create 4 subfolders
    for i in 0..4 {
        create_folder!(app, token, format!("sub-{i}"), parent_id);
    }

    // Upload 6 files to parent
    for _ in 0..6 {
        upload_test_file_to_folder!(app, token, parent_id);
    }

    // Paginated list
    let req = test::TestRequest::get()
        .uri(&format!(
            "/api/v1/folders/{parent_id}?folder_limit=2&file_limit=3"
        ))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["folders"].as_array().unwrap().len(), 2);
    assert_eq!(body["data"]["files"].as_array().unwrap().len(), 3);
    assert_eq!(body["data"]["folders_total"], 4);
    assert_eq!(body["data"]["files_total"], 6);
}

#[actix_web::test]
async fn test_trash_pagination() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    // Create and delete 4 folders
    for i in 0..4 {
        let id = create_folder!(app, token, format!("trash-folder-{i}"));
        let req = test::TestRequest::delete()
            .uri(&format!("/api/v1/folders/{id}"))
            .insert_header(("Cookie", format!("aster_access={token}")))
            .to_request();
        let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    // Create and delete 5 files
    for _ in 0..5 {
        let id = upload_test_file!(app, token);
        let req = test::TestRequest::delete()
            .uri(&format!("/api/v1/files/{id}"))
            .insert_header(("Cookie", format!("aster_access={token}")))
            .to_request();
        let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    // Default trash list with totals
    let req = test::TestRequest::get()
        .uri("/api/v1/trash")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["folders_total"], 4);
    assert_eq!(body["data"]["files_total"], 5);

    // Paginated trash
    let req = test::TestRequest::get()
        .uri("/api/v1/trash?folder_limit=2&file_limit=3")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["folders"].as_array().unwrap().len(), 2);
    assert_eq!(body["data"]["files"].as_array().unwrap().len(), 3);
    assert_eq!(body["data"]["folders_total"], 4);
    assert_eq!(body["data"]["files_total"], 5);
}
