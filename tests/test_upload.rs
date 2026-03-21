//! 上传集成测试（分片 + presigned）

#[macro_use]
mod common;

use actix_web::test;
use serde_json::Value;

#[actix_web::test]
async fn test_chunked_upload_flow() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    // 1. 初始化分片上传（10KB 文件，chunk_size=5MB → 直传模式）
    let req = test::TestRequest::post()
        .uri("/api/v1/files/upload/init")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "filename": "chunked.txt",
            "total_size": 10240
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    // 小文件可能返回 direct 模式
    let mode = body["data"]["mode"].as_str().unwrap();
    assert!(
        mode == "direct" || mode == "chunked",
        "mode should be direct or chunked, got {mode}"
    );

    if mode == "chunked" {
        let upload_id = body["data"]["upload_id"].as_str().unwrap().to_string();
        let total_chunks = body["data"]["total_chunks"].as_i64().unwrap();

        // 2. 上传分片
        for i in 0..total_chunks {
            let chunk_data = vec![b'A'; 5120]; // 5KB per chunk
            let req = test::TestRequest::put()
                .uri(&format!("/api/v1/files/upload/{upload_id}/{i}"))
                .insert_header(("Cookie", format!("aster_access={token}")))
                .insert_header(("Content-Type", "application/octet-stream"))
                .set_payload(chunk_data)
                .to_request();
            let resp = test::call_service(&app, req).await;
            assert_eq!(resp.status(), 200, "chunk {i} upload failed");
        }

        // 3. 查看进度
        let req = test::TestRequest::get()
            .uri(&format!("/api/v1/files/upload/{upload_id}"))
            .insert_header(("Cookie", format!("aster_access={token}")))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        // 4. 完成上传
        let req = test::TestRequest::post()
            .uri(&format!("/api/v1/files/upload/{upload_id}/complete"))
            .insert_header(("Cookie", format!("aster_access={token}")))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 201);
        let body: Value = test::read_body_json(resp).await;
        assert_eq!(body["data"]["name"], "chunked.txt");
    }
}

#[actix_web::test]
async fn test_chunked_upload_cancel() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    // 初始化大文件上传（强制 chunked 模式）
    let req = test::TestRequest::post()
        .uri("/api/v1/files/upload/init")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "filename": "big.bin",
            "total_size": 10_485_760  // 10MB → 超过 chunk_size(5MB) → chunked
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;

    if let Some(upload_id) = body["data"]["upload_id"].as_str() {
        // 取消上传
        let req = test::TestRequest::delete()
            .uri(&format!("/api/v1/files/upload/{upload_id}"))
            .insert_header(("Cookie", format!("aster_access={token}")))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        // 再查进度应该 404
        let req = test::TestRequest::get()
            .uri(&format!("/api/v1/files/upload/{upload_id}"))
            .insert_header(("Cookie", format!("aster_access={token}")))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status() == 404 || resp.status() == 410);
    }
}

/// 测试 init_upload：Local 策略下不返回 presigned
#[actix_web::test]
async fn test_init_upload_local_never_presigned() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let req = test::TestRequest::post()
        .uri("/api/v1/files/upload/init")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "filename": "test.bin",
            "total_size": 1024
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let mode = body["data"]["mode"].as_str().unwrap();
    assert_ne!(
        mode, "presigned",
        "local storage should never use presigned"
    );
    assert!(body["data"]["presigned_url"].is_null());
}

/// 并发上传同一分片不会导致 received_count 多算（TOCTOU 修复验证）
#[actix_web::test]
async fn test_concurrent_chunk_upload_idempotent() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    // 初始化大文件上传（强制 chunked）
    let req = test::TestRequest::post()
        .uri("/api/v1/files/upload/init")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "filename": "concurrent.bin",
            "total_size": 10_485_760
        }))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let mode = body["data"]["mode"].as_str().unwrap();

    if mode == "chunked" {
        let upload_id = body["data"]["upload_id"].as_str().unwrap().to_string();

        // 上传 chunk 0（小于默认 payload 限制）
        let chunk_data = vec![b'X'; 1024];
        let req = test::TestRequest::put()
            .uri(&format!("/api/v1/files/upload/{upload_id}/0"))
            .insert_header(("Cookie", format!("aster_access={token}")))
            .insert_header(("Content-Type", "application/octet-stream"))
            .set_payload(chunk_data.clone())
            .to_request();
        let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
        let body: Value = test::read_body_json(resp).await;
        let count_after_first = body["data"]["received_count"].as_i64().unwrap();

        // 重复上传同一 chunk 0（模拟并发/重试）
        let req = test::TestRequest::put()
            .uri(&format!("/api/v1/files/upload/{upload_id}/0"))
            .insert_header(("Cookie", format!("aster_access={token}")))
            .insert_header(("Content-Type", "application/octet-stream"))
            .set_payload(chunk_data.clone())
            .to_request();
        let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
        let body: Value = test::read_body_json(resp).await;
        let count_after_second = body["data"]["received_count"].as_i64().unwrap();

        // 第三次重复
        let req = test::TestRequest::put()
            .uri(&format!("/api/v1/files/upload/{upload_id}/0"))
            .insert_header(("Cookie", format!("aster_access={token}")))
            .insert_header(("Content-Type", "application/octet-stream"))
            .set_payload(chunk_data)
            .to_request();
        let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
        let body: Value = test::read_body_json(resp).await;
        let count_after_third = body["data"]["received_count"].as_i64().unwrap();

        // received_count 应该都是 1（幂等，不多算）
        assert_eq!(
            count_after_first, 1,
            "first upload should set count to 1"
        );
        assert_eq!(
            count_after_second, count_after_first,
            "duplicate chunk should not increment count: got {count_after_second}"
        );
        assert_eq!(
            count_after_third, count_after_first,
            "third duplicate should not increment count: got {count_after_third}"
        );
    }
}

/// S3 presigned upload 端到端测试（需要 testcontainers + rustfs）
#[tokio::test]
async fn test_presigned_upload_s3_e2e() {
    use aster_drive::services::{auth_service, upload_service};
    use testcontainers::{GenericImage, ImageExt, runners::AsyncRunner};

    // 启动 rustfs 容器
    let container = GenericImage::new("rustfs/rustfs", "latest")
        .with_exposed_port(testcontainers::core::IntoContainerPort::tcp(9000))
        .with_env_var("RUSTFS_ACCESS_KEY", "rustfsadmin")
        .with_env_var("RUSTFS_SECRET_KEY", "rustfsadmin123")
        .start()
        .await
        .expect("failed to start rustfs container");

    let port = container.get_host_port_ipv4(9000).await.unwrap();
    let endpoint = format!("http://127.0.0.1:{port}");
    let bucket = "test-presigned";

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // 创建 bucket
    {
        let credentials = aws_credential_types::Credentials::new(
            "rustfsadmin",
            "rustfsadmin123",
            None,
            None,
            "test",
        );
        let config = aws_sdk_s3::Config::builder()
            .behavior_version(aws_sdk_s3::config::BehaviorVersion::latest())
            .region(aws_sdk_s3::config::Region::new("us-east-1"))
            .credentials_provider(credentials)
            .endpoint_url(&endpoint)
            .force_path_style(true)
            .build();
        let client = aws_sdk_s3::Client::from_conf(config);
        let _ = client.create_bucket().bucket(bucket).send().await;
    }

    // 创建 state（内存 SQLite）
    let state = common::setup().await;

    // 创建 S3 策略 + presigned_upload: true
    use chrono::Utc;
    use sea_orm::Set;
    let now = Utc::now();
    let s3_policy = aster_drive::db::repository::policy_repo::create(
        &state.db,
        aster_drive::entities::storage_policy::ActiveModel {
            name: Set("Test S3 Presigned".to_string()),
            driver_type: Set(aster_drive::types::DriverType::S3),
            endpoint: Set(endpoint),
            bucket: Set(bucket.to_string()),
            access_key: Set("rustfsadmin".to_string()),
            secret_key: Set("rustfsadmin123".to_string()),
            base_path: Set("uploads".to_string()),
            max_file_size: Set(0),
            allowed_types: Set("[]".to_string()),
            options: Set(r#"{"presigned_upload":true}"#.to_string()),
            is_default: Set(false),
            chunk_size: Set(5_242_880),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    // 注册用户 + 分配 S3 策略为默认
    let user = auth_service::register(&state, "s3user", "s3@test.com", "pass123")
        .await
        .unwrap();
    use aster_drive::db::repository::policy_repo;
    let _ = policy_repo::create_user_policy(
        &state.db,
        aster_drive::entities::user_storage_policy::ActiveModel {
            user_id: Set(user.id),
            policy_id: Set(s3_policy.id),
            is_default: Set(true),
            quota_bytes: Set(0),
            created_at: Set(now),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    // 1. init_upload → 应返回 presigned 模式
    let data = b"hello presigned world!";
    let init = upload_service::init_upload(&state, user.id, "hello.txt", data.len() as i64, None)
        .await
        .unwrap();
    assert_eq!(init.mode, aster_drive::types::UploadMode::Presigned);
    assert!(init.presigned_url.is_some());
    assert!(init.upload_id.is_some());

    let presigned_url = init.presigned_url.unwrap();
    let upload_id = init.upload_id.unwrap();

    // 2. PUT 到 presigned URL（模拟客户端直传）
    let client = reqwest::Client::new();
    let resp = client
        .put(&presigned_url)
        .header("Content-Type", "application/octet-stream")
        .body(data.to_vec())
        .send()
        .await
        .unwrap();
    assert!(
        resp.status().is_success(),
        "S3 presigned PUT failed: {}",
        resp.status()
    );

    // 3. complete → 服务端 hash + dedup + 建记录
    let file = upload_service::complete_upload(&state, &upload_id, user.id)
        .await
        .unwrap();
    assert_eq!(file.name, "hello.txt");

    // 4. 验证文件可通过 driver 读取
    let policy = policy_repo::find_by_id(&state.db, s3_policy.id)
        .await
        .unwrap();
    let driver = state.driver_registry.get_driver(&policy).unwrap();
    let blob = aster_drive::db::repository::file_repo::find_blob_by_id(&state.db, file.blob_id)
        .await
        .unwrap();
    let got = driver.get(&blob.storage_path).await.unwrap();
    assert_eq!(got, data);

    // 5. 上传相同内容 → 应该 dedup（ref_count 增加，不新建 blob）
    let init2 = upload_service::init_upload(&state, user.id, "hello2.txt", data.len() as i64, None)
        .await
        .unwrap();
    let url2 = init2.presigned_url.unwrap();
    let id2 = init2.upload_id.unwrap();
    client
        .put(&url2)
        .header("Content-Type", "application/octet-stream")
        .body(data.to_vec())
        .send()
        .await
        .unwrap();
    // 同名文件会冲突，用不同文件名
    let file2 = upload_service::complete_upload(&state, &id2, user.id)
        .await
        .unwrap();
    assert_eq!(
        file2.blob_id, file.blob_id,
        "same content should dedup to same blob"
    );

    let blob_after =
        aster_drive::db::repository::file_repo::find_blob_by_id(&state.db, file.blob_id)
            .await
            .unwrap();
    assert_eq!(blob_after.ref_count, 2);
}
