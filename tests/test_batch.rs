#[macro_use]
mod common;

use actix_web::test;
use serde_json::Value;

fn upload_named_file(name: &str, content: &str, mime: &str, boundary: &str) -> String {
    format!(
        "--{boundary}\r\n\
         Content-Disposition: form-data; name=\"file\"; filename=\"{name}\"\r\n\
         Content-Type: {mime}\r\n\r\n\
         {content}\r\n\
         --{boundary}--\r\n"
    )
}

#[actix_web::test]
async fn test_batch_delete_files() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let boundary = "----TestBoundary123";

    // Upload 3 files
    let mut file_ids = Vec::new();
    for name in ["file1.txt", "file2.txt", "file3.txt"] {
        let payload =
            upload_named_file(name, &format!("content of {name}"), "text/plain", boundary);
        let req = test::TestRequest::post()
            .uri("/api/v1/files/upload")
            .insert_header(("Cookie", format!("aster_access={}", token)))
            .insert_header((
                "Content-Type",
                format!("multipart/form-data; boundary={boundary}"),
            ))
            .set_payload(payload)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 201);
        let body: Value = test::read_body_json(resp).await;
        file_ids.push(body["data"]["id"].as_i64().unwrap());
    }

    // Batch delete first two files
    let req = test::TestRequest::post()
        .uri("/api/v1/batch/delete")
        .insert_header(("Cookie", format!("aster_access={}", token)))
        .set_json(serde_json::json!({
            "file_ids": [file_ids[0], file_ids[1]],
            "folder_ids": []
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["code"], 0);
    assert_eq!(body["data"]["succeeded"], 2);
    assert_eq!(body["data"]["failed"], 0);

    // Third file should still be accessible
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/files/{}", file_ids[2]))
        .insert_header(("Cookie", format!("aster_access={}", token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

#[actix_web::test]
async fn test_batch_delete_mixed() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let boundary = "----TestBoundary123";

    // Upload a file
    let payload = upload_named_file("mixed1.txt", "content1", "text/plain", boundary);
    let req = test::TestRequest::post()
        .uri("/api/v1/files/upload")
        .insert_header(("Cookie", format!("aster_access={}", token)))
        .insert_header((
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        ))
        .set_payload(payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let file_id = body["data"]["id"].as_i64().unwrap();

    // Create a folder
    let req = test::TestRequest::post()
        .uri("/api/v1/folders")
        .insert_header(("Cookie", format!("aster_access={}", token)))
        .set_json(serde_json::json!({ "name": "MixedFolder", "parent_id": null }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let folder_id = body["data"]["id"].as_i64().unwrap();

    // Batch delete one file + one folder
    let req = test::TestRequest::post()
        .uri("/api/v1/batch/delete")
        .insert_header(("Cookie", format!("aster_access={}", token)))
        .set_json(serde_json::json!({
            "file_ids": [file_id],
            "folder_ids": [folder_id]
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["code"], 0);
    assert_eq!(body["data"]["succeeded"], 2);
    assert_eq!(body["data"]["failed"], 0);
}

#[actix_web::test]
async fn test_batch_move_files() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let req = test::TestRequest::post()
        .uri("/api/v1/folders")
        .insert_header(("Cookie", format!("aster_access={}", token)))
        .set_json(serde_json::json!({ "name": "Source", "parent_id": null }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let source_id = body["data"]["id"].as_i64().unwrap();

    let boundary = "----TestBoundary123";

    // Upload 2 files in source folder
    let mut file_ids = Vec::new();
    for name in ["move1.txt", "move2.txt"] {
        let payload =
            upload_named_file(name, &format!("content of {name}"), "text/plain", boundary);
        let req = test::TestRequest::post()
            .uri(&format!("/api/v1/files/upload?folder_id={source_id}"))
            .insert_header(("Cookie", format!("aster_access={}", token)))
            .insert_header((
                "Content-Type",
                format!("multipart/form-data; boundary={boundary}"),
            ))
            .set_payload(payload)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 201);
        let body: Value = test::read_body_json(resp).await;
        file_ids.push(body["data"]["id"].as_i64().unwrap());
    }

    // Create target folder
    let req = test::TestRequest::post()
        .uri("/api/v1/folders")
        .insert_header(("Cookie", format!("aster_access={}", token)))
        .set_json(serde_json::json!({ "name": "Target", "parent_id": null }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let target_id = body["data"]["id"].as_i64().unwrap();

    // Batch move both files into target folder
    let req = test::TestRequest::post()
        .uri("/api/v1/batch/move")
        .insert_header(("Cookie", format!("aster_access={}", token)))
        .set_json(serde_json::json!({
            "file_ids": file_ids,
            "folder_ids": [],
            "target_folder_id": target_id
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["code"], 0);
    assert_eq!(body["data"]["succeeded"], 2);

    // Verify files are now in target folder
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/folders/{target_id}"))
        .insert_header(("Cookie", format!("aster_access={}", token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["files"].as_array().unwrap().len(), 2);

    // Source folder should have no files now
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/folders/{source_id}"))
        .insert_header(("Cookie", format!("aster_access={}", token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["files"].as_array().unwrap().len(), 0);

    // Batch move both files back to root (null = root)
    let req = test::TestRequest::post()
        .uri("/api/v1/batch/move")
        .insert_header(("Cookie", format!("aster_access={}", token)))
        .set_json(serde_json::json!({
            "file_ids": file_ids,
            "folder_ids": [],
            "target_folder_id": null
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["code"], 0);
    assert_eq!(body["data"]["succeeded"], 2);

    // Root should have the files again
    let req = test::TestRequest::get()
        .uri("/api/v1/folders")
        .insert_header(("Cookie", format!("aster_access={}", token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["files"].as_array().unwrap().len(), 2);

    // Target folder should be empty again
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/folders/{target_id}"))
        .insert_header(("Cookie", format!("aster_access={}", token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["files"].as_array().unwrap().len(), 0);
}

#[actix_web::test]
async fn test_batch_copy_files() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let req = test::TestRequest::post()
        .uri("/api/v1/folders")
        .insert_header(("Cookie", format!("aster_access={}", token)))
        .set_json(serde_json::json!({ "name": "Source", "parent_id": null }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let source_id = body["data"]["id"].as_i64().unwrap();

    let boundary = "----TestBoundary123";

    // Upload 2 files in source folder
    let mut file_ids = Vec::new();
    for name in ["copy1.txt", "copy2.txt"] {
        let payload =
            upload_named_file(name, &format!("content of {name}"), "text/plain", boundary);
        let req = test::TestRequest::post()
            .uri(&format!("/api/v1/files/upload?folder_id={source_id}"))
            .insert_header(("Cookie", format!("aster_access={}", token)))
            .insert_header((
                "Content-Type",
                format!("multipart/form-data; boundary={boundary}"),
            ))
            .set_payload(payload)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 201);
        let body: Value = test::read_body_json(resp).await;
        file_ids.push(body["data"]["id"].as_i64().unwrap());
    }

    // Batch copy both files to root (null = root)
    let req = test::TestRequest::post()
        .uri("/api/v1/batch/copy")
        .insert_header(("Cookie", format!("aster_access={}", token)))
        .set_json(serde_json::json!({
            "file_ids": file_ids,
            "folder_ids": [],
            "target_folder_id": null
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["code"], 0);
    assert_eq!(body["data"]["succeeded"], 2);

    // Verify copies exist in root
    let req = test::TestRequest::get()
        .uri("/api/v1/folders")
        .insert_header(("Cookie", format!("aster_access={}", token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    let root_files = body["data"]["files"].as_array().unwrap();
    assert_eq!(root_files.len(), 2);

    // Originals should still be in source folder
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/folders/{source_id}"))
        .insert_header(("Cookie", format!("aster_access={}", token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["files"].as_array().unwrap().len(), 2);
}

#[actix_web::test]
async fn test_batch_limit_allows_1000_items() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let req = test::TestRequest::post()
        .uri("/api/v1/batch/delete")
        .insert_header(("Cookie", format!("aster_access={}", token)))
        .set_json(serde_json::json!({
            "file_ids": (1..=1000).collect::<Vec<i64>>(),
            "folder_ids": []
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

#[actix_web::test]
async fn test_batch_limit_rejects_over_1000_items() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let req = test::TestRequest::post()
        .uri("/api/v1/batch/delete")
        .insert_header(("Cookie", format!("aster_access={}", token)))
        .set_json(serde_json::json!({
            "file_ids": (1..=1001).collect::<Vec<i64>>(),
            "folder_ids": []
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["msg"], "batch size cannot exceed 1000 items");
}

#[actix_web::test]
async fn test_batch_empty_request() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    // Send batch delete with empty arrays — validation should reject
    let req = test::TestRequest::post()
        .uri("/api/v1/batch/delete")
        .insert_header(("Cookie", format!("aster_access={}", token)))
        .set_json(serde_json::json!({
            "file_ids": [],
            "folder_ids": []
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);
    let body: Value = test::read_body_json(resp).await;
    assert_ne!(body["code"], 0);
}
