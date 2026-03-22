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

    let boundary = "----TestBoundary123";

    // Upload 2 files in root
    let mut file_ids = Vec::new();
    for name in ["move1.txt", "move2.txt"] {
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

    // Root should have no files (only the folder remains)
    let req = test::TestRequest::get()
        .uri("/api/v1/folders")
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

    let boundary = "----TestBoundary123";

    // Upload 2 files in root
    let mut file_ids = Vec::new();
    for name in ["copy1.txt", "copy2.txt"] {
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

    // Create destination folder
    let req = test::TestRequest::post()
        .uri("/api/v1/folders")
        .insert_header(("Cookie", format!("aster_access={}", token)))
        .set_json(serde_json::json!({ "name": "CopyDest", "parent_id": null }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let dest_id = body["data"]["id"].as_i64().unwrap();

    // Batch copy both files to destination folder
    let req = test::TestRequest::post()
        .uri("/api/v1/batch/copy")
        .insert_header(("Cookie", format!("aster_access={}", token)))
        .set_json(serde_json::json!({
            "file_ids": file_ids,
            "folder_ids": [],
            "target_folder_id": dest_id
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["code"], 0);
    assert_eq!(body["data"]["succeeded"], 2);

    // Verify copies exist in destination folder
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/folders/{dest_id}"))
        .insert_header(("Cookie", format!("aster_access={}", token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["files"].as_array().unwrap().len(), 2);

    // Originals should still be in root
    let req = test::TestRequest::get()
        .uri("/api/v1/folders")
        .insert_header(("Cookie", format!("aster_access={}", token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["files"].as_array().unwrap().len(), 2);
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
