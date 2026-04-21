//! 远端存储集成测试。

#[macro_use]
mod common;

use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::time::Duration;

use actix_web::{App, HttpServer, test, web};
use aster_drive::api::error_code::ErrorCode;
use aster_drive::db::repository::{
    file_repo, managed_follower_repo, master_binding_repo, policy_repo, upload_session_repo,
    user_repo,
};
use aster_drive::entities::storage_policy;
use aster_drive::services::{
    auth_service, file_service, folder_service, managed_follower_service, master_binding_service,
    policy_service, thumbnail_service, upload_service,
};
use aster_drive::storage::remote_protocol::{
    RemoteStorageClient, RemoteStorageComposeRequest, sign_internal_request, sign_presigned_request,
};
use aster_drive::types::{
    DriverType, NullablePatch, RemoteUploadStrategy, StoragePolicyOptions,
    StoredStoragePolicyAllowedTypes, serialize_storage_policy_options,
};
use chrono::Utc;
use futures::TryStreamExt;
use sea_orm::{ActiveModelTrait, Set};

struct TestHttpServer {
    base_url: String,
    handle: actix_web::dev::ServerHandle,
    task: tokio::task::JoinHandle<std::io::Result<()>>,
}

impl TestHttpServer {
    async fn stop(self) {
        self.handle.stop(true).await;
        let _ = self.task.await;
    }
}

async fn spawn_internal_storage_server(
    state: aster_drive::runtime::FollowerAppState,
) -> TestHttpServer {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0))
        .expect("test internal storage listener should bind");
    let addr = listener
        .local_addr()
        .expect("test internal storage listener should expose local addr");
    let state_for_server = state.clone();
    let server = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(state_for_server.clone()))
            .service(
                web::scope("/api/v1").service(aster_drive::api::routes::internal_storage::routes()),
            )
    })
    .listen(listener)
    .expect("test internal storage server should listen")
    .run();
    let handle = server.handle();
    let task = tokio::spawn(server);

    TestHttpServer {
        base_url: format!("http://127.0.0.1:{}", addr.port()),
        handle,
        task,
    }
}

async fn create_remote_policy(
    state: &aster_drive::runtime::PrimaryAppState,
    remote_node_id: i64,
    name: &str,
    base_path: &str,
) -> storage_policy::Model {
    create_remote_policy_with_options(
        state,
        remote_node_id,
        name,
        base_path,
        StoragePolicyOptions::default(),
        5_242_880,
    )
    .await
}

async fn create_remote_policy_with_options(
    state: &aster_drive::runtime::PrimaryAppState,
    remote_node_id: i64,
    name: &str,
    base_path: &str,
    options: StoragePolicyOptions,
    chunk_size: i64,
) -> storage_policy::Model {
    let now = Utc::now();
    let policy = policy_repo::create(
        &state.db,
        storage_policy::ActiveModel {
            name: Set(name.to_string()),
            driver_type: Set(DriverType::Remote),
            endpoint: Set(String::new()),
            bucket: Set(String::new()),
            access_key: Set(String::new()),
            secret_key: Set(String::new()),
            base_path: Set(base_path.to_string()),
            remote_node_id: Set(Some(remote_node_id)),
            max_file_size: Set(0),
            allowed_types: Set(StoredStoragePolicyAllowedTypes::empty()),
            options: Set(serialize_storage_policy_options(&options)
                .expect("remote policy options should serialize")),
            is_default: Set(false),
            chunk_size: Set(chunk_size),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        },
    )
    .await
    .expect("remote policy should be created");

    state
        .policy_snapshot
        .reload(&state.db)
        .await
        .expect("policy snapshot should reload after creating remote policy");

    policy
}

async fn set_policy_max_file_size(
    state: &aster_drive::runtime::PrimaryAppState,
    policy: &storage_policy::Model,
    max_file_size: i64,
) {
    let mut active: storage_policy::ActiveModel = policy.clone().into();
    active.max_file_size = Set(max_file_size);
    active.updated_at = Set(Utc::now());
    active
        .update(&state.db)
        .await
        .expect("policy max_file_size should update");
    state
        .policy_snapshot
        .reload(&state.db)
        .await
        .expect("policy snapshot should reload after updating max_file_size");
    state.driver_registry.invalidate(policy.id);
}

async fn wait_for_remote_probe(
    state: &aster_drive::runtime::PrimaryAppState,
    node_id: i64,
) -> managed_follower_service::RemoteNodeInfo {
    for attempt in 0..20 {
        match managed_follower_service::test_connection(state, node_id).await {
            Ok(info) => return info,
            Err(error) if attempt < 19 => {
                tracing::debug!(attempt, node_id, "remote probe not ready yet: {error}");
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            Err(error) => panic!("remote probe should eventually succeed: {error}"),
        }
    }

    unreachable!("remote probe retry loop should return or panic")
}

async fn write_temp_upload_file(
    state: &aster_drive::runtime::PrimaryAppState,
    name: &str,
    data: &[u8],
) -> PathBuf {
    let path = Path::new(&state.config.server.temp_dir).join(name);
    tokio::fs::create_dir_all(&state.config.server.temp_dir)
        .await
        .expect("test temp dir should exist");
    tokio::fs::write(&path, data)
        .await
        .expect("test temp upload file should be written");
    path
}

fn build_test_png() -> Vec<u8> {
    let image = image::RgbaImage::from_pixel(4, 4, image::Rgba([255, 0, 0, 255]));
    let mut cursor = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut cursor, image::ImageFormat::Png)
        .expect("test png should encode");
    cursor.into_inner()
}

async fn collect_download_body(
    outcome: aster_drive::services::file_service::DownloadOutcome,
) -> Vec<u8> {
    match outcome {
        aster_drive::services::file_service::DownloadOutcome::Stream(streamed) => streamed
            .body
            .try_fold(Vec::new(), |mut acc, chunk| async move {
                acc.extend_from_slice(&chunk);
                Ok(acc)
            })
            .await
            .expect("download stream should succeed"),
        other => panic!("expected streaming remote download, got {other:?}"),
    }
}

async fn put_presigned_bytes(url: &str, data: Vec<u8>) -> reqwest::Response {
    reqwest::Client::new()
        .put(url)
        .body(data)
        .send()
        .await
        .expect("presigned upload request should succeed")
}

fn provider_object_path(
    ingress_base_path: &str,
    namespace: &str,
    remote_base_path: &str,
    storage_path: &str,
) -> PathBuf {
    let mut relative = PathBuf::from(namespace.trim_matches('/'));
    if !remote_base_path.trim_matches('/').is_empty() {
        relative.push(remote_base_path.trim_matches('/'));
    }
    relative.push(storage_path.trim_start_matches('/'));
    Path::new(ingress_base_path).join(relative)
}

fn path_and_query_from_url(url: &str) -> String {
    let parsed = reqwest::Url::parse(url).expect("test URL should parse");
    match parsed.query() {
        Some(query) => format!("{}?{query}", parsed.path()),
        None => parsed.path().to_string(),
    }
}

fn rewrite_path_query_param(path_and_query: &str, key: &str, value: Option<&str>) -> String {
    let mut parsed = reqwest::Url::parse(&format!("http://example.invalid{path_and_query}"))
        .expect("test path and query should parse");
    let existing_pairs: Vec<(String, String)> = parsed
        .query_pairs()
        .into_owned()
        .filter(|(existing_key, _)| existing_key != key)
        .collect();
    parsed.set_query(None);
    {
        let mut query = parsed.query_pairs_mut();
        for (existing_key, existing_value) in existing_pairs {
            query.append_pair(&existing_key, &existing_value);
        }
        if let Some(value) = value {
            query.append_pair(key, value);
        }
    }
    path_and_query_from_url(parsed.as_str())
}

struct BrowserPresignedCorsFixture {
    provider_state: aster_drive::runtime::PrimaryAppState,
    presigned_path: String,
}

async fn setup_browser_presigned_cors_fixture(
    namespace: &str,
    master_url: &str,
) -> BrowserPresignedCorsFixture {
    let provider_state = common::setup().await;
    let consumer_state = common::setup().await;
    let provider_ingress_policy = provider_state
        .policy_snapshot
        .system_default_policy()
        .expect("provider default ingress policy should exist");

    let consumer_node = managed_follower_service::create(
        &consumer_state,
        managed_follower_service::CreateRemoteNodeInput {
            name: format!("{namespace}-node"),
            base_url: "http://provider.example.com".to_string(),
            namespace: namespace.to_string(),
            is_enabled: true,
        },
    )
    .await
    .expect("consumer remote node should be created");
    let consumer_node_model =
        managed_follower_repo::find_by_id(&consumer_state.db, consumer_node.id)
            .await
            .expect("consumer remote node should be queryable");

    master_binding_service::upsert_from_enrollment(
        &provider_state.db,
        master_binding_service::UpsertMasterBindingInput {
            name: format!("{namespace}-binding"),
            master_url: master_url.to_string(),
            access_key: consumer_node_model.access_key.clone(),
            secret_key: consumer_node_model.secret_key.clone(),
            namespace: namespace.to_string(),
            ingress_policy_id: provider_ingress_policy.id,
            is_enabled: true,
        },
    )
    .await
    .expect("provider master binding should be created");
    provider_state
        .driver_registry
        .reload_master_bindings(&provider_state.db)
        .await
        .expect("provider binding registry should reload");

    let remote_policy = create_remote_policy_with_options(
        &consumer_state,
        consumer_node.id,
        &format!("Remote Presigned {namespace} Policy"),
        &format!("{namespace}-base"),
        StoragePolicyOptions {
            remote_upload_strategy: Some(RemoteUploadStrategy::Presigned),
            ..Default::default()
        },
        1024,
    )
    .await;

    let app = create_test_app!(consumer_state.clone());
    let _ = register_and_login!(app);
    let user = user_repo::find_by_username(&consumer_state.db, "testuser")
        .await
        .expect("test user lookup should succeed")
        .expect("test user should exist");
    let folder = folder_service::create(
        &consumer_state,
        user.id,
        &format!("{namespace}-folder"),
        None,
    )
    .await
    .expect("remote folder should be created");
    folder_service::update(
        &consumer_state,
        folder.id,
        user.id,
        None,
        NullablePatch::Absent,
        NullablePatch::Value(remote_policy.id),
    )
    .await
    .expect("remote policy should bind to folder");

    let init = upload_service::init_upload(
        &consumer_state,
        user.id,
        &format!("{namespace}.bin"),
        32,
        Some(folder.id),
        None,
    )
    .await
    .expect("remote presigned upload should initialize");

    BrowserPresignedCorsFixture {
        provider_state,
        presigned_path: path_and_query_from_url(
            &init
                .presigned_url
                .expect("presigned mode should return presigned_url"),
        ),
    }
}

#[actix_web::test]
async fn test_internal_storage_presigned_put_rejects_payload_exceeding_ingress_limit() {
    let provider_state = common::setup().await;
    let provider_ingress_policy = provider_state
        .policy_snapshot
        .system_default_policy()
        .expect("provider default ingress policy should exist");
    set_policy_max_file_size(&provider_state, &provider_ingress_policy, 8).await;

    let access_key = "limit-access-key";
    let secret_key = "limit-secret-key";
    master_binding_service::upsert_from_enrollment(
        &provider_state.db,
        master_binding_service::UpsertMasterBindingInput {
            name: "limit-binding".to_string(),
            master_url: "http://master.example.com".to_string(),
            access_key: access_key.to_string(),
            secret_key: secret_key.to_string(),
            namespace: "provider-limit-space".to_string(),
            ingress_policy_id: provider_ingress_policy.id,
            is_enabled: true,
        },
    )
    .await
    .expect("provider binding should be created");
    provider_state
        .driver_registry
        .reload_master_bindings(&provider_state.db)
        .await
        .expect("provider binding registry should reload");

    let follower_app = test::init_service(
        App::new()
            .app_data(web::Data::new(provider_state.follower_view()))
            .service(
                web::scope("/api/v1").service(aster_drive::api::routes::internal_storage::routes()),
            ),
    )
    .await;

    let path = "/api/v1/internal/storage/objects/too-large.bin";
    let expires_at = Utc::now().timestamp() + 300;
    let signature = sign_presigned_request(secret_key, "PUT", path, access_key, expires_at);
    let req = test::TestRequest::put()
        .uri(&format!(
            "{path}?aster_access_key={access_key}&aster_expires={expires_at}&aster_signature={signature}"
        ))
        .insert_header((
            actix_web::http::header::CONTENT_TYPE,
            "application/octet-stream",
        ))
        .insert_header((actix_web::http::header::CONTENT_LENGTH, "16"))
        .set_payload(vec![b'x'; 16])
        .to_request();
    let resp = test::call_service(&follower_app, req).await;

    assert_eq!(resp.status(), actix_web::http::StatusCode::BAD_REQUEST);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(
        body["code"],
        serde_json::json!(ErrorCode::FileTooLarge as i32)
    );
    assert_eq!(body["msg"], "object size 16 exceeds limit 8");
}

#[actix_web::test]
async fn test_internal_storage_compose_rejects_expected_size_exceeding_ingress_limit() {
    let provider_state = common::setup().await;
    let provider_ingress_policy = provider_state
        .policy_snapshot
        .system_default_policy()
        .expect("provider default ingress policy should exist");
    set_policy_max_file_size(&provider_state, &provider_ingress_policy, 8).await;

    let access_key = "compose-limit-access-key";
    let secret_key = "compose-limit-secret-key";
    master_binding_service::upsert_from_enrollment(
        &provider_state.db,
        master_binding_service::UpsertMasterBindingInput {
            name: "compose-limit-binding".to_string(),
            master_url: "http://master.example.com".to_string(),
            access_key: access_key.to_string(),
            secret_key: secret_key.to_string(),
            namespace: "provider-compose-limit-space".to_string(),
            ingress_policy_id: provider_ingress_policy.id,
            is_enabled: true,
        },
    )
    .await
    .expect("provider binding should be created");
    provider_state
        .driver_registry
        .reload_master_bindings(&provider_state.db)
        .await
        .expect("provider binding registry should reload");

    let follower_app = test::init_service(
        App::new()
            .app_data(web::Data::new(provider_state.follower_view()))
            .service(
                web::scope("/api/v1").service(aster_drive::api::routes::internal_storage::routes()),
            ),
    )
    .await;

    let body = serde_json::to_vec(&RemoteStorageComposeRequest {
        target_key: "assembled.bin".to_string(),
        part_keys: vec!["part-1".to_string(), "part-2".to_string()],
        expected_size: 16,
    })
    .expect("compose request body should serialize");
    let path = "/api/v1/internal/storage/compose";
    let timestamp = Utc::now().timestamp();
    let nonce = "compose-limit-test";
    let signature = sign_internal_request(
        secret_key,
        "POST",
        path,
        timestamp,
        nonce,
        Some(u64::try_from(body.len()).expect("compose body length should fit u64")),
    );
    let req = test::TestRequest::post()
        .uri(path)
        .insert_header((actix_web::http::header::CONTENT_TYPE, "application/json"))
        .insert_header((
            actix_web::http::header::CONTENT_LENGTH,
            body.len().to_string(),
        ))
        .insert_header(("x-aster-access-key", access_key))
        .insert_header(("x-aster-timestamp", timestamp.to_string()))
        .insert_header(("x-aster-nonce", nonce))
        .insert_header(("x-aster-signature", signature))
        .set_payload(body)
        .to_request();
    let resp = test::call_service(&follower_app, req).await;

    assert_eq!(resp.status(), actix_web::http::StatusCode::BAD_REQUEST);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(
        body["code"],
        serde_json::json!(ErrorCode::FileTooLarge as i32)
    );
    assert_eq!(body["msg"], "composed object size 16 exceeds limit 8");
}

#[actix_web::test]
async fn test_remote_node_connection_failure_returns_error_and_persists_last_error() {
    let state = common::setup().await;
    let node = managed_follower_service::create(
        &state,
        managed_follower_service::CreateRemoteNodeInput {
            name: "broken-remote".to_string(),
            base_url: "http://127.0.0.1:9".to_string(),
            namespace: "broken-space".to_string(),
            is_enabled: true,
        },
    )
    .await
    .expect("broken remote node should be created");

    let error = managed_follower_service::test_connection(&state, node.id)
        .await
        .expect_err("connection test should surface probe failures");
    assert_eq!(error.code(), "E005");

    let stored = managed_follower_repo::find_by_id(&state.db, node.id)
        .await
        .expect("remote node should still exist after failed probe");
    assert!(!stored.last_error.is_empty());
}

#[actix_web::test]
async fn test_remote_storage_end_to_end_via_internal_api() {
    let provider_state = common::setup().await;
    let consumer_state = common::setup().await;
    let provider_server = spawn_internal_storage_server(provider_state.follower_view()).await;

    let provider_ingress_policy = provider_state
        .policy_snapshot
        .system_default_policy()
        .expect("provider default ingress policy should exist");

    let consumer_node = managed_follower_service::create(
        &consumer_state,
        managed_follower_service::CreateRemoteNodeInput {
            name: "provider-target".to_string(),
            base_url: provider_server.base_url.clone(),
            namespace: "provider-space".to_string(),
            is_enabled: true,
        },
    )
    .await
    .expect("consumer remote node should be created");
    let consumer_node_model =
        managed_follower_repo::find_by_id(&consumer_state.db, consumer_node.id)
            .await
            .expect("consumer remote node should be queryable");

    let (provider_binding, _) = master_binding_service::upsert_from_enrollment(
        &provider_state.db,
        master_binding_service::UpsertMasterBindingInput {
            name: "consumer-access".to_string(),
            master_url: "http://master.example.com".to_string(),
            access_key: consumer_node_model.access_key.clone(),
            secret_key: consumer_node_model.secret_key.clone(),
            namespace: "provider-space".to_string(),
            ingress_policy_id: provider_ingress_policy.id,
            is_enabled: true,
        },
    )
    .await
    .expect("provider master binding should be created");
    provider_state
        .driver_registry
        .reload_master_bindings(&provider_state.db)
        .await
        .expect("provider binding registry should reload");

    let probed = wait_for_remote_probe(&consumer_state, consumer_node.id).await;
    assert_eq!(probed.capabilities.protocol_version, "v1");
    assert!(probed.capabilities.supports_list);
    assert!(probed.capabilities.supports_range_read);
    assert!(probed.capabilities.supports_stream_upload);

    let remote_policy = create_remote_policy(
        &consumer_state,
        consumer_node.id,
        "Remote Test Policy",
        "consumer-base",
    )
    .await;

    let user = auth_service::register(
        &consumer_state,
        "remoteuser",
        "remoteuser@example.com",
        "pass1234",
    )
    .await
    .expect("consumer test user should be created");

    let folder = folder_service::create(&consumer_state, user.id, "remote-folder", None)
        .await
        .expect("remote folder should be created");
    folder_service::update(
        &consumer_state,
        folder.id,
        user.id,
        None,
        NullablePatch::Absent,
        NullablePatch::Value(remote_policy.id),
    )
    .await
    .expect("remote policy should bind to folder");

    let upload_bytes = b"hello remote storage over internal api";
    let upload_path = write_temp_upload_file(
        &consumer_state,
        &format!("remote-upload-{}.txt", uuid::Uuid::new_v4()),
        upload_bytes,
    )
    .await;
    let upload_path_string = upload_path.to_string_lossy().into_owned();

    let created = file_service::store_from_temp(
        &consumer_state,
        user.id,
        file_service::StoreFromTempRequest::new(
            Some(folder.id),
            "remote.txt",
            &upload_path_string,
            i64::try_from(upload_bytes.len()).expect("upload size should fit i64"),
        ),
    )
    .await
    .expect("remote file upload should succeed");
    aster_drive::utils::cleanup_temp_file(&upload_path_string).await;

    let created_file = file_repo::find_by_id(&consumer_state.db, created.id)
        .await
        .expect("uploaded file should be queryable");
    let created_blob = file_repo::find_blob_by_id(&consumer_state.db, created_file.blob_id)
        .await
        .expect("uploaded blob should be queryable");

    assert!(created_blob.hash.starts_with("remote-"));
    assert!(created_blob.storage_path.starts_with("files/"));

    let remote_driver = consumer_state
        .driver_registry
        .get_driver(&remote_policy)
        .expect("remote policy driver should resolve");
    assert!(
        remote_driver
            .exists(&created_blob.storage_path)
            .await
            .expect("remote HEAD should succeed")
    );

    let listed_paths = remote_driver
        .as_list()
        .expect("remote driver should support list")
        .list_paths(None)
        .await
        .expect("remote list should succeed");
    assert!(
        listed_paths.contains(&created_blob.storage_path),
        "remote list should include uploaded blob path"
    );

    let provider_uploaded_path = provider_object_path(
        &provider_ingress_policy.base_path,
        &provider_binding.namespace,
        &remote_policy.base_path,
        &created_blob.storage_path,
    );
    let provider_uploaded_bytes = tokio::fs::read(&provider_uploaded_path)
        .await
        .expect("provider-side object should exist on local ingress storage");
    assert_eq!(provider_uploaded_bytes, upload_bytes);

    let downloaded_bytes = collect_download_body(
        file_service::download(&consumer_state, created_file.id, user.id, None)
            .await
            .expect("remote file download should succeed"),
    )
    .await;
    assert_eq!(downloaded_bytes, upload_bytes);

    file_service::delete(&consumer_state, created_file.id, user.id)
        .await
        .expect("remote file soft delete should succeed");
    file_service::purge(&consumer_state, created_file.id, user.id)
        .await
        .expect("remote file purge should succeed");

    assert!(
        !remote_driver
            .exists(&created_blob.storage_path)
            .await
            .expect("remote HEAD after purge should succeed")
    );
    assert!(
        tokio::fs::metadata(&provider_uploaded_path).await.is_err(),
        "provider-side object should be deleted after purge"
    );

    let empty_file = file_service::create_empty(
        &consumer_state,
        user.id,
        Some(folder.id),
        "empty-remote.txt",
    )
    .await
    .expect("remote empty file should be created");
    let empty_file = file_repo::find_by_id(&consumer_state.db, empty_file.id)
        .await
        .expect("empty remote file should exist");
    let empty_blob = file_repo::find_blob_by_id(&consumer_state.db, empty_file.blob_id)
        .await
        .expect("empty remote blob should exist");

    assert!(empty_blob.hash.starts_with("remote-"));
    assert!(empty_blob.storage_path.starts_with("files/"));
    assert!(
        remote_driver
            .exists(&empty_blob.storage_path)
            .await
            .expect("remote HEAD for empty blob should succeed")
    );

    let provider_empty_path = provider_object_path(
        &provider_ingress_policy.base_path,
        &provider_binding.namespace,
        &remote_policy.base_path,
        &empty_blob.storage_path,
    );
    let empty_meta = tokio::fs::metadata(&provider_empty_path)
        .await
        .expect("provider-side empty object should exist");
    assert_eq!(empty_meta.len(), 0);

    file_service::purge(&consumer_state, empty_file.id, user.id)
        .await
        .expect("empty remote file purge should succeed");
    assert!(
        tokio::fs::metadata(&provider_empty_path).await.is_err(),
        "provider-side empty object should be deleted after purge"
    );

    provider_server.stop().await;
}

#[actix_web::test]
async fn test_disabling_remote_node_syncs_follower_binding_and_blocks_remote_use() {
    let provider_state = common::setup().await;
    let consumer_state = common::setup().await;
    let provider_server = spawn_internal_storage_server(provider_state.follower_view()).await;

    let provider_ingress_policy = provider_state
        .policy_snapshot
        .system_default_policy()
        .expect("provider default ingress policy should exist");

    let consumer_node = managed_follower_service::create(
        &consumer_state,
        managed_follower_service::CreateRemoteNodeInput {
            name: "provider-target".to_string(),
            base_url: provider_server.base_url.clone(),
            namespace: "provider-disable-space".to_string(),
            is_enabled: true,
        },
    )
    .await
    .expect("consumer remote node should be created");
    let consumer_node_model =
        managed_follower_repo::find_by_id(&consumer_state.db, consumer_node.id)
            .await
            .expect("consumer remote node should be queryable");

    master_binding_service::upsert_from_enrollment(
        &provider_state.db,
        master_binding_service::UpsertMasterBindingInput {
            name: "consumer-access".to_string(),
            master_url: "http://master.example.com".to_string(),
            access_key: consumer_node_model.access_key.clone(),
            secret_key: consumer_node_model.secret_key.clone(),
            namespace: "provider-disable-space".to_string(),
            ingress_policy_id: provider_ingress_policy.id,
            is_enabled: true,
        },
    )
    .await
    .expect("provider master binding should be created");
    provider_state
        .driver_registry
        .reload_master_bindings(&provider_state.db)
        .await
        .expect("provider binding registry should reload");

    wait_for_remote_probe(&consumer_state, consumer_node.id).await;

    managed_follower_service::update(
        &consumer_state,
        consumer_node.id,
        managed_follower_service::UpdateRemoteNodeInput {
            is_enabled: Some(false),
            ..Default::default()
        },
    )
    .await
    .expect("disabling remote node should succeed");

    let provider_binding = master_binding_repo::find_by_access_key(
        &provider_state.db,
        &consumer_node_model.access_key,
    )
    .await
    .expect("provider binding lookup should succeed")
    .expect("provider binding should still exist");
    assert!(!provider_binding.is_enabled);

    let forbidden_client = RemoteStorageClient::new(
        &provider_server.base_url,
        &consumer_node_model.access_key,
        &consumer_node_model.secret_key,
    )
    .expect("manual remote client should initialize");
    let probe_error = forbidden_client
        .probe_capabilities()
        .await
        .expect_err("disabled binding should reject signed internal requests");
    assert_eq!(probe_error.code(), "E060");
    assert!(probe_error.message().contains("master binding is disabled"));

    let create_error = policy_service::create(
        &consumer_state,
        policy_service::CreateStoragePolicyInput {
            name: "Disabled Remote Policy".to_string(),
            connection: policy_service::StoragePolicyConnectionInput {
                driver_type: DriverType::Remote,
                endpoint: String::new(),
                bucket: String::new(),
                access_key: String::new(),
                secret_key: String::new(),
                base_path: String::new(),
                remote_node_id: Some(consumer_node.id),
            },
            max_file_size: 0,
            chunk_size: None,
            is_default: false,
            allowed_types: Some(Vec::new()),
            options: Some(StoragePolicyOptions::default()),
        },
    )
    .await
    .expect_err("disabled remote nodes should not be bindable to remote policies");
    assert_eq!(create_error.code(), "E005");
    assert!(create_error.message().contains("is disabled"));

    let remote_policy = create_remote_policy(
        &consumer_state,
        consumer_node.id,
        "Disabled Remote Policy",
        "disabled-base",
    )
    .await;
    let driver_error = match consumer_state.driver_registry.get_driver(&remote_policy) {
        Ok(_) => panic!("disabled remote nodes should not resolve into remote drivers"),
        Err(error) => error,
    };
    assert_eq!(driver_error.code(), "E060");
    assert!(driver_error.message().contains("is disabled"));

    provider_server.stop().await;
}

#[actix_web::test]
async fn test_saved_remote_node_connection_endpoint_returns_precondition_failed_for_disabled_binding()
 {
    let provider_state = common::setup().await;
    let consumer_state = common::setup().await;
    let provider_server = spawn_internal_storage_server(provider_state.follower_view()).await;

    let provider_ingress_policy = provider_state
        .policy_snapshot
        .system_default_policy()
        .expect("provider default ingress policy should exist");

    let consumer_node = managed_follower_service::create(
        &consumer_state,
        managed_follower_service::CreateRemoteNodeInput {
            name: "provider-target".to_string(),
            base_url: provider_server.base_url.clone(),
            namespace: "provider-endpoint-space".to_string(),
            is_enabled: true,
        },
    )
    .await
    .expect("consumer remote node should be created");
    let consumer_node_model =
        managed_follower_repo::find_by_id(&consumer_state.db, consumer_node.id)
            .await
            .expect("consumer remote node should be queryable");

    master_binding_service::upsert_from_enrollment(
        &provider_state.db,
        master_binding_service::UpsertMasterBindingInput {
            name: "consumer-access".to_string(),
            master_url: "http://master.example.com".to_string(),
            access_key: consumer_node_model.access_key.clone(),
            secret_key: consumer_node_model.secret_key.clone(),
            namespace: "provider-endpoint-space".to_string(),
            ingress_policy_id: provider_ingress_policy.id,
            is_enabled: true,
        },
    )
    .await
    .expect("provider master binding should be created");
    provider_state
        .driver_registry
        .reload_master_bindings(&provider_state.db)
        .await
        .expect("provider binding registry should reload");

    wait_for_remote_probe(&consumer_state, consumer_node.id).await;

    master_binding_service::sync_from_primary(
        &provider_state.follower_view(),
        &consumer_node_model.access_key,
        master_binding_service::SyncMasterBindingInput {
            name: "consumer-access".to_string(),
            namespace: "provider-endpoint-space".to_string(),
            is_enabled: false,
        },
    )
    .await
    .expect("provider binding should disable cleanly");

    let app = create_test_app!(consumer_state.clone());
    let (admin_token, _) = register_and_login!(app);

    let req = test::TestRequest::post()
        .uri(&format!(
            "/api/v1/admin/remote-nodes/{}/test",
            consumer_node.id
        ))
        .insert_header(("Cookie", common::access_cookie_header(&admin_token)))
        .insert_header(common::csrf_header_for(&admin_token))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 412);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(
        body["msg"]
            .as_str()
            .unwrap()
            .contains("master binding is disabled")
    );

    provider_server.stop().await;
}

#[actix_web::test]
async fn test_thumbnail_endpoint_returns_precondition_failed_when_remote_node_disabled() {
    let provider_state = common::setup().await;
    let consumer_state = common::setup().await;
    let provider_server = spawn_internal_storage_server(provider_state.follower_view()).await;

    let provider_ingress_policy = provider_state
        .policy_snapshot
        .system_default_policy()
        .expect("provider default ingress policy should exist");

    let consumer_node = managed_follower_service::create(
        &consumer_state,
        managed_follower_service::CreateRemoteNodeInput {
            name: "provider-target".to_string(),
            base_url: provider_server.base_url.clone(),
            namespace: "provider-thumb-space".to_string(),
            is_enabled: true,
        },
    )
    .await
    .expect("consumer remote node should be created");
    let consumer_node_model =
        managed_follower_repo::find_by_id(&consumer_state.db, consumer_node.id)
            .await
            .expect("consumer remote node should be queryable");

    master_binding_service::upsert_from_enrollment(
        &provider_state.db,
        master_binding_service::UpsertMasterBindingInput {
            name: "consumer-access".to_string(),
            master_url: "http://master.example.com".to_string(),
            access_key: consumer_node_model.access_key.clone(),
            secret_key: consumer_node_model.secret_key.clone(),
            namespace: "provider-thumb-space".to_string(),
            ingress_policy_id: provider_ingress_policy.id,
            is_enabled: true,
        },
    )
    .await
    .expect("provider master binding should be created");
    provider_state
        .driver_registry
        .reload_master_bindings(&provider_state.db)
        .await
        .expect("provider binding registry should reload");

    wait_for_remote_probe(&consumer_state, consumer_node.id).await;

    let remote_policy = create_remote_policy(
        &consumer_state,
        consumer_node.id,
        "Remote Thumb Policy",
        "thumb-base",
    )
    .await;

    let app = create_test_app!(consumer_state.clone());
    let (token, _) = register_and_login!(app);
    let user = user_repo::find_by_username(&consumer_state.db, "testuser")
        .await
        .expect("test user lookup should succeed")
        .expect("test user should exist");

    let folder = folder_service::create(&consumer_state, user.id, "remote-thumbs", None)
        .await
        .expect("remote folder should be created");
    folder_service::update(
        &consumer_state,
        folder.id,
        user.id,
        None,
        NullablePatch::Absent,
        NullablePatch::Value(remote_policy.id),
    )
    .await
    .expect("remote policy should bind to folder");

    let png_bytes = build_test_png();
    let upload_path = write_temp_upload_file(
        &consumer_state,
        &format!("remote-thumb-{}.png", uuid::Uuid::new_v4()),
        &png_bytes,
    )
    .await;
    let upload_path_string = upload_path.to_string_lossy().into_owned();
    let created = file_service::store_from_temp(
        &consumer_state,
        user.id,
        file_service::StoreFromTempRequest::new(
            Some(folder.id),
            "thumb-source.png",
            &upload_path_string,
            i64::try_from(png_bytes.len()).expect("png size should fit i64"),
        ),
    )
    .await
    .expect("remote thumbnail source upload should succeed");
    aster_drive::utils::cleanup_temp_file(&upload_path_string).await;

    let created_file = file_repo::find_by_id(&consumer_state.db, created.id)
        .await
        .expect("uploaded file should be queryable");
    let created_blob = file_repo::find_blob_by_id(&consumer_state.db, created_file.blob_id)
        .await
        .expect("uploaded blob should be queryable");
    thumbnail_service::generate_and_store(&consumer_state, &created_blob)
        .await
        .expect("thumbnail should generate while remote node is enabled");

    managed_follower_service::update(
        &consumer_state,
        consumer_node.id,
        managed_follower_service::UpdateRemoteNodeInput {
            is_enabled: Some(false),
            ..Default::default()
        },
    )
    .await
    .expect("disabling remote node should succeed");

    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/files/{}/thumbnail", created.id))
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 412);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body["msg"].as_str().unwrap().contains("remote node #"));
    assert!(body["msg"].as_str().unwrap().contains("is disabled"));

    provider_server.stop().await;
}

#[actix_web::test]
async fn test_remote_presigned_upload_writes_directly_to_provider() {
    let provider_state = common::setup().await;
    let consumer_state = common::setup().await;
    let provider_server = spawn_internal_storage_server(provider_state.follower_view()).await;

    let provider_ingress_policy = provider_state
        .policy_snapshot
        .system_default_policy()
        .expect("provider default ingress policy should exist");

    let consumer_node = managed_follower_service::create(
        &consumer_state,
        managed_follower_service::CreateRemoteNodeInput {
            name: "provider-target".to_string(),
            base_url: provider_server.base_url.clone(),
            namespace: "provider-chunked-space".to_string(),
            is_enabled: true,
        },
    )
    .await
    .expect("consumer remote node should be created");
    let consumer_node_model =
        managed_follower_repo::find_by_id(&consumer_state.db, consumer_node.id)
            .await
            .expect("consumer remote node should be queryable");

    master_binding_service::upsert_from_enrollment(
        &provider_state.db,
        master_binding_service::UpsertMasterBindingInput {
            name: "consumer-access".to_string(),
            master_url: "http://master.example.com".to_string(),
            access_key: consumer_node_model.access_key.clone(),
            secret_key: consumer_node_model.secret_key.clone(),
            namespace: "provider-chunked-space".to_string(),
            ingress_policy_id: provider_ingress_policy.id,
            is_enabled: true,
        },
    )
    .await
    .expect("provider master binding should be created");
    provider_state
        .driver_registry
        .reload_master_bindings(&provider_state.db)
        .await
        .expect("provider binding registry should reload");

    wait_for_remote_probe(&consumer_state, consumer_node.id).await;

    let remote_policy = create_remote_policy_with_options(
        &consumer_state,
        consumer_node.id,
        "Remote Presigned Policy",
        "presigned-base",
        StoragePolicyOptions {
            remote_upload_strategy: Some(RemoteUploadStrategy::Presigned),
            ..Default::default()
        },
        1024,
    )
    .await;

    let app = create_test_app!(consumer_state.clone());
    let _ = register_and_login!(app);
    let user = user_repo::find_by_username(&consumer_state.db, "testuser")
        .await
        .expect("test user lookup should succeed")
        .expect("test user should exist");
    let folder = folder_service::create(&consumer_state, user.id, "remote-presigned", None)
        .await
        .expect("remote folder should be created");
    folder_service::update(
        &consumer_state,
        folder.id,
        user.id,
        None,
        NullablePatch::Absent,
        NullablePatch::Value(remote_policy.id),
    )
    .await
    .expect("remote policy should bind to folder");

    let body = b"presigned-remote-upload".to_vec();
    let init = upload_service::init_upload(
        &consumer_state,
        user.id,
        "presigned.bin",
        i64::try_from(body.len()).expect("body length should fit i64"),
        Some(folder.id),
        None,
    )
    .await
    .expect("remote presigned upload should initialize");
    assert_eq!(init.mode, aster_drive::types::UploadMode::Presigned);

    let upload_id = init
        .upload_id
        .expect("presigned mode should return upload id");
    let presigned_url = init
        .presigned_url
        .clone()
        .expect("presigned mode should return presigned_url");
    let response = put_presigned_bytes(&presigned_url, body.clone()).await;
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    assert!(
        response.headers().get(reqwest::header::ETAG).is_some(),
        "remote presigned upload should return ETag header"
    );
    let session = upload_session_repo::find_by_id(&consumer_state.db, &upload_id)
        .await
        .expect("upload session should exist");
    let temp_key = session
        .s3_temp_key
        .clone()
        .expect("remote presigned temp key should exist");
    let uploaded_temp_path = provider_object_path(
        &provider_ingress_policy.base_path,
        &consumer_node.namespace,
        "",
        &format!("{}/{}", remote_policy.base_path.trim_matches('/'), temp_key),
    );
    let uploaded_temp = tokio::fs::read(&uploaded_temp_path)
        .await
        .expect("provider temp object should exist after presigned PUT");
    assert_eq!(uploaded_temp, body);
    let remote_driver = consumer_state
        .driver_registry
        .get_driver(&remote_policy)
        .expect("remote driver should be available");
    let remote_metadata = remote_driver
        .metadata(&temp_key)
        .await
        .expect("remote metadata should see uploaded temp object");
    assert_eq!(
        remote_metadata.size,
        u64::try_from(body.len()).expect("body length should fit u64")
    );

    let temp_dir = aster_drive::utils::paths::upload_temp_dir(
        &consumer_state.config.server.upload_temp_dir,
        &upload_id,
    );
    assert!(
        !tokio::fs::try_exists(&temp_dir)
            .await
            .expect("temp dir existence should be readable"),
        "single-file remote presigned upload should not create local chunk temp dir"
    );

    let created = upload_service::complete_upload(&consumer_state, &upload_id, user.id, None)
        .await
        .expect("remote presigned upload should complete");
    let created_file = file_repo::find_by_id(&consumer_state.db, created.id)
        .await
        .expect("uploaded file should be queryable");
    let created_blob = file_repo::find_blob_by_id(&consumer_state.db, created_file.blob_id)
        .await
        .expect("uploaded blob should be queryable");

    let stored_path = provider_object_path(
        &provider_ingress_policy.base_path,
        &consumer_node.namespace,
        &remote_policy.base_path,
        &created_blob.storage_path,
    );
    let stored = tokio::fs::read(&stored_path)
        .await
        .expect("provider should receive direct presigned upload");
    assert_eq!(stored, body);

    provider_server.stop().await;
}

#[actix_web::test]
async fn test_remote_presigned_upload_browser_cors_follows_bound_master_origin() {
    let provider_state = common::setup().await;
    let consumer_state = common::setup().await;
    let provider_ingress_policy = provider_state
        .policy_snapshot
        .system_default_policy()
        .expect("provider default ingress policy should exist");

    let consumer_node = managed_follower_service::create(
        &consumer_state,
        managed_follower_service::CreateRemoteNodeInput {
            name: "provider-target".to_string(),
            base_url: "http://provider.example.com".to_string(),
            namespace: "provider-browser-cors-space".to_string(),
            is_enabled: true,
        },
    )
    .await
    .expect("consumer remote node should be created");
    let consumer_node_model =
        managed_follower_repo::find_by_id(&consumer_state.db, consumer_node.id)
            .await
            .expect("consumer remote node should be queryable");

    master_binding_service::upsert_from_enrollment(
        &provider_state.db,
        master_binding_service::UpsertMasterBindingInput {
            name: "consumer-access".to_string(),
            master_url: "http://localhost:3000".to_string(),
            access_key: consumer_node_model.access_key.clone(),
            secret_key: consumer_node_model.secret_key.clone(),
            namespace: "provider-browser-cors-space".to_string(),
            ingress_policy_id: provider_ingress_policy.id,
            is_enabled: true,
        },
    )
    .await
    .expect("provider master binding should be created");
    provider_state
        .driver_registry
        .reload_master_bindings(&provider_state.db)
        .await
        .expect("provider binding registry should reload");

    let remote_policy = create_remote_policy_with_options(
        &consumer_state,
        consumer_node.id,
        "Remote Presigned Browser CORS Policy",
        "browser-cors-base",
        StoragePolicyOptions {
            remote_upload_strategy: Some(RemoteUploadStrategy::Presigned),
            ..Default::default()
        },
        1024,
    )
    .await;

    let app = create_test_app!(consumer_state.clone());
    let _ = register_and_login!(app);
    let user = user_repo::find_by_username(&consumer_state.db, "testuser")
        .await
        .expect("test user lookup should succeed")
        .expect("test user should exist");
    let folder = folder_service::create(&consumer_state, user.id, "remote-browser-cors", None)
        .await
        .expect("remote folder should be created");
    folder_service::update(
        &consumer_state,
        folder.id,
        user.id,
        None,
        NullablePatch::Absent,
        NullablePatch::Value(remote_policy.id),
    )
    .await
    .expect("remote policy should bind to folder");

    let body = b"presigned-browser-cors".to_vec();
    let init = upload_service::init_upload(
        &consumer_state,
        user.id,
        "presigned-browser.bin",
        i64::try_from(body.len()).expect("body length should fit i64"),
        Some(folder.id),
        None,
    )
    .await
    .expect("remote presigned upload should initialize");
    let presigned_url = init
        .presigned_url
        .expect("presigned mode should return presigned_url");
    let presigned_path = path_and_query_from_url(&presigned_url);

    let follower_app = test::init_service(
        App::new()
            .app_data(web::Data::new(provider_state.follower_view()))
            .service(
                web::scope("/api/v1").service(aster_drive::api::routes::internal_storage::routes()),
            ),
    )
    .await;

    let req = test::TestRequest::default()
        .method(actix_web::http::Method::OPTIONS)
        .uri(&presigned_path)
        .insert_header(("Origin", "http://localhost:3000"))
        .insert_header(("Access-Control-Request-Method", "PUT"))
        .insert_header(("Access-Control-Request-Headers", "content-type"))
        .to_request();
    let resp = test::call_service(&follower_app, req).await;
    assert_eq!(resp.status(), actix_web::http::StatusCode::NO_CONTENT);
    assert_eq!(
        resp.headers()
            .get(actix_web::http::header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .and_then(|value| value.to_str().ok()),
        Some("http://localhost:3000")
    );
    assert_eq!(
        resp.headers()
            .get(actix_web::http::header::ACCESS_CONTROL_ALLOW_METHODS)
            .and_then(|value| value.to_str().ok()),
        Some("PUT, OPTIONS")
    );
    assert_eq!(
        resp.headers()
            .get(actix_web::http::header::ACCESS_CONTROL_ALLOW_HEADERS)
            .and_then(|value| value.to_str().ok()),
        Some("content-type")
    );
    let vary = resp
        .headers()
        .get(actix_web::http::header::VARY)
        .and_then(|value| value.to_str().ok())
        .expect("preflight response should include Vary");
    assert!(vary.contains("Origin"));
    assert!(vary.contains("Access-Control-Request-Method"));
    assert!(vary.contains("Access-Control-Request-Headers"));

    let req = test::TestRequest::default()
        .method(actix_web::http::Method::OPTIONS)
        .uri(&presigned_path)
        .insert_header(("Origin", "http://evil.example.com"))
        .insert_header(("Access-Control-Request-Method", "PUT"))
        .insert_header(("Access-Control-Request-Headers", "content-type"))
        .to_request();
    let resp = test::call_service(&follower_app, req).await;
    assert_eq!(resp.status(), actix_web::http::StatusCode::FORBIDDEN);

    let req = test::TestRequest::default()
        .method(actix_web::http::Method::OPTIONS)
        .uri(&presigned_path)
        .insert_header(("Origin", "http://localhost:3000"))
        .insert_header(("Access-Control-Request-Method", "DELETE"))
        .insert_header(("Access-Control-Request-Headers", "content-type"))
        .to_request();
    let resp = test::call_service(&follower_app, req).await;
    assert_eq!(resp.status(), actix_web::http::StatusCode::FORBIDDEN);

    let req = test::TestRequest::default()
        .method(actix_web::http::Method::OPTIONS)
        .uri(&presigned_path)
        .insert_header(("Origin", "http://localhost:3000"))
        .insert_header(("Access-Control-Request-Method", "PUT"))
        .insert_header(("Access-Control-Request-Headers", "content-type, x-extra"))
        .to_request();
    let resp = test::call_service(&follower_app, req).await;
    assert_eq!(resp.status(), actix_web::http::StatusCode::FORBIDDEN);

    let req = test::TestRequest::put()
        .uri(&presigned_path)
        .insert_header(("Origin", "http://evil.example.com"))
        .insert_header((
            actix_web::http::header::CONTENT_TYPE,
            "application/octet-stream",
        ))
        .insert_header((
            actix_web::http::header::CONTENT_LENGTH,
            body.len().to_string(),
        ))
        .set_payload(body.clone())
        .to_request();
    let resp = test::call_service(&follower_app, req).await;
    assert_eq!(resp.status(), actix_web::http::StatusCode::FORBIDDEN);

    let req = test::TestRequest::put()
        .uri(&presigned_path)
        .insert_header(("Origin", "http://localhost:3000"))
        .insert_header((
            actix_web::http::header::CONTENT_TYPE,
            "application/octet-stream",
        ))
        .insert_header((
            actix_web::http::header::CONTENT_LENGTH,
            body.len().to_string(),
        ))
        .set_payload(body)
        .to_request();
    let resp = test::call_service(&follower_app, req).await;
    assert_eq!(resp.status(), actix_web::http::StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get(actix_web::http::header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .and_then(|value| value.to_str().ok()),
        Some("http://localhost:3000")
    );
    assert_eq!(
        resp.headers()
            .get(actix_web::http::header::ACCESS_CONTROL_EXPOSE_HEADERS)
            .and_then(|value| value.to_str().ok()),
        Some("ETag")
    );
    let vary = resp
        .headers()
        .get(actix_web::http::header::VARY)
        .and_then(|value| value.to_str().ok())
        .expect("actual PUT response should include Vary");
    assert!(vary.contains("Origin"));
    assert!(
        resp.headers().contains_key(actix_web::http::header::ETAG),
        "browser PUT should expose ETag header"
    );
}

#[actix_web::test]
async fn test_remote_presigned_upload_browser_cors_accepts_master_url_with_path_and_port() {
    let provider_state = common::setup().await;
    let consumer_state = common::setup().await;
    let provider_ingress_policy = provider_state
        .policy_snapshot
        .system_default_policy()
        .expect("provider default ingress policy should exist");

    let consumer_node = managed_follower_service::create(
        &consumer_state,
        managed_follower_service::CreateRemoteNodeInput {
            name: "provider-target".to_string(),
            base_url: "http://provider.example.com".to_string(),
            namespace: "provider-browser-origin-path-space".to_string(),
            is_enabled: true,
        },
    )
    .await
    .expect("consumer remote node should be created");
    let consumer_node_model =
        managed_follower_repo::find_by_id(&consumer_state.db, consumer_node.id)
            .await
            .expect("consumer remote node should be queryable");

    master_binding_service::upsert_from_enrollment(
        &provider_state.db,
        master_binding_service::UpsertMasterBindingInput {
            name: "consumer-access".to_string(),
            master_url: "http://localhost:3000/admin/settings/general".to_string(),
            access_key: consumer_node_model.access_key.clone(),
            secret_key: consumer_node_model.secret_key.clone(),
            namespace: "provider-browser-origin-path-space".to_string(),
            ingress_policy_id: provider_ingress_policy.id,
            is_enabled: true,
        },
    )
    .await
    .expect("provider master binding should be created");
    provider_state
        .driver_registry
        .reload_master_bindings(&provider_state.db)
        .await
        .expect("provider binding registry should reload");

    let remote_policy = create_remote_policy_with_options(
        &consumer_state,
        consumer_node.id,
        "Remote Presigned Browser Origin Path Policy",
        "browser-origin-path-base",
        StoragePolicyOptions {
            remote_upload_strategy: Some(RemoteUploadStrategy::Presigned),
            ..Default::default()
        },
        1024,
    )
    .await;

    let app = create_test_app!(consumer_state.clone());
    let _ = register_and_login!(app);
    let user = user_repo::find_by_username(&consumer_state.db, "testuser")
        .await
        .expect("test user lookup should succeed")
        .expect("test user should exist");
    let folder =
        folder_service::create(&consumer_state, user.id, "remote-browser-origin-path", None)
            .await
            .expect("remote folder should be created");
    folder_service::update(
        &consumer_state,
        folder.id,
        user.id,
        None,
        NullablePatch::Absent,
        NullablePatch::Value(remote_policy.id),
    )
    .await
    .expect("remote policy should bind to folder");

    let init = upload_service::init_upload(
        &consumer_state,
        user.id,
        "presigned-origin-path.bin",
        32,
        Some(folder.id),
        None,
    )
    .await
    .expect("remote presigned upload should initialize");
    let presigned_path = path_and_query_from_url(
        &init
            .presigned_url
            .expect("presigned mode should return presigned_url"),
    );

    let follower_app = test::init_service(
        App::new()
            .app_data(web::Data::new(provider_state.follower_view()))
            .service(
                web::scope("/api/v1").service(aster_drive::api::routes::internal_storage::routes()),
            ),
    )
    .await;

    let req = test::TestRequest::default()
        .method(actix_web::http::Method::OPTIONS)
        .uri(&presigned_path)
        .insert_header(("Origin", "http://localhost:3000"))
        .insert_header(("Access-Control-Request-Method", "PUT"))
        .insert_header(("Access-Control-Request-Headers", "content-type"))
        .to_request();
    let resp = test::call_service(&follower_app, req).await;
    assert_eq!(resp.status(), actix_web::http::StatusCode::NO_CONTENT);
    assert_eq!(
        resp.headers()
            .get(actix_web::http::header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .and_then(|value| value.to_str().ok()),
        Some("http://localhost:3000")
    );
}

#[actix_web::test]
async fn test_remote_presigned_upload_browser_cors_rejects_disabled_binding() {
    let provider_state = common::setup().await;
    let consumer_state = common::setup().await;
    let provider_ingress_policy = provider_state
        .policy_snapshot
        .system_default_policy()
        .expect("provider default ingress policy should exist");

    let consumer_node = managed_follower_service::create(
        &consumer_state,
        managed_follower_service::CreateRemoteNodeInput {
            name: "provider-target".to_string(),
            base_url: "http://provider.example.com".to_string(),
            namespace: "provider-browser-disabled-binding-space".to_string(),
            is_enabled: true,
        },
    )
    .await
    .expect("consumer remote node should be created");
    let consumer_node_model =
        managed_follower_repo::find_by_id(&consumer_state.db, consumer_node.id)
            .await
            .expect("consumer remote node should be queryable");

    let binding = master_binding_service::upsert_from_enrollment(
        &provider_state.db,
        master_binding_service::UpsertMasterBindingInput {
            name: "consumer-access".to_string(),
            master_url: "http://localhost:3000".to_string(),
            access_key: consumer_node_model.access_key.clone(),
            secret_key: consumer_node_model.secret_key.clone(),
            namespace: "provider-browser-disabled-binding-space".to_string(),
            ingress_policy_id: provider_ingress_policy.id,
            is_enabled: true,
        },
    )
    .await
    .expect("provider master binding should be created")
    .0;
    provider_state
        .driver_registry
        .reload_master_bindings(&provider_state.db)
        .await
        .expect("provider binding registry should reload");

    let remote_policy = create_remote_policy_with_options(
        &consumer_state,
        consumer_node.id,
        "Remote Presigned Disabled Binding Policy",
        "browser-disabled-binding-base",
        StoragePolicyOptions {
            remote_upload_strategy: Some(RemoteUploadStrategy::Presigned),
            ..Default::default()
        },
        1024,
    )
    .await;

    let app = create_test_app!(consumer_state.clone());
    let _ = register_and_login!(app);
    let user = user_repo::find_by_username(&consumer_state.db, "testuser")
        .await
        .expect("test user lookup should succeed")
        .expect("test user should exist");
    let folder = folder_service::create(
        &consumer_state,
        user.id,
        "remote-browser-disabled-binding",
        None,
    )
    .await
    .expect("remote folder should be created");
    folder_service::update(
        &consumer_state,
        folder.id,
        user.id,
        None,
        NullablePatch::Absent,
        NullablePatch::Value(remote_policy.id),
    )
    .await
    .expect("remote policy should bind to folder");

    let init = upload_service::init_upload(
        &consumer_state,
        user.id,
        "presigned-disabled-binding.bin",
        32,
        Some(folder.id),
        None,
    )
    .await
    .expect("remote presigned upload should initialize");
    let presigned_path = path_and_query_from_url(
        &init
            .presigned_url
            .expect("presigned mode should return presigned_url"),
    );

    let follower_state = provider_state.follower_view();
    master_binding_service::sync_from_primary(
        &follower_state,
        &binding.access_key,
        master_binding_service::SyncMasterBindingInput {
            name: binding.name.clone(),
            namespace: binding.namespace.clone(),
            is_enabled: false,
        },
    )
    .await
    .expect("binding should be disabled");

    let follower_app = test::init_service(
        App::new()
            .app_data(web::Data::new(provider_state.follower_view()))
            .service(
                web::scope("/api/v1").service(aster_drive::api::routes::internal_storage::routes()),
            ),
    )
    .await;

    let req = test::TestRequest::default()
        .method(actix_web::http::Method::OPTIONS)
        .uri(&presigned_path)
        .insert_header(("Origin", "http://localhost:3000"))
        .insert_header(("Access-Control-Request-Method", "PUT"))
        .insert_header(("Access-Control-Request-Headers", "content-type"))
        .to_request();
    let resp = test::call_service(&follower_app, req).await;
    assert_eq!(resp.status(), actix_web::http::StatusCode::FORBIDDEN);
}

#[actix_web::test]
async fn test_remote_presigned_upload_browser_cors_passthrough_without_origin_header() {
    let fixture = setup_browser_presigned_cors_fixture(
        "provider-browser-no-origin-space",
        "http://localhost:3000",
    )
    .await;
    let follower_app = test::init_service(
        App::new()
            .app_data(web::Data::new(fixture.provider_state.follower_view()))
            .service(
                web::scope("/api/v1").service(aster_drive::api::routes::internal_storage::routes()),
            ),
    )
    .await;

    let body = b"presigned-no-origin".to_vec();
    let req = test::TestRequest::put()
        .uri(&fixture.presigned_path)
        .insert_header((
            actix_web::http::header::CONTENT_TYPE,
            "application/octet-stream",
        ))
        .insert_header((
            actix_web::http::header::CONTENT_LENGTH,
            body.len().to_string(),
        ))
        .set_payload(body)
        .to_request();
    let resp = test::call_service(&follower_app, req).await;

    assert_eq!(resp.status(), actix_web::http::StatusCode::OK);
    assert!(
        !resp
            .headers()
            .contains_key(actix_web::http::header::ACCESS_CONTROL_ALLOW_ORIGIN),
        "requests without Origin should bypass browser CORS decoration"
    );
    assert!(
        !resp
            .headers()
            .contains_key(actix_web::http::header::ACCESS_CONTROL_EXPOSE_HEADERS),
        "non-browser requests should not receive expose-headers decoration"
    );
    assert!(
        resp.headers().contains_key(actix_web::http::header::ETAG),
        "plain presigned PUT should still return ETag"
    );
}

#[actix_web::test]
async fn test_remote_presigned_upload_browser_cors_missing_access_key_passthroughs_to_auth_error() {
    let fixture = setup_browser_presigned_cors_fixture(
        "provider-browser-missing-access-key-space",
        "http://localhost:3000",
    )
    .await;
    let presigned_path =
        rewrite_path_query_param(&fixture.presigned_path, "aster_access_key", None);
    let follower_app = test::init_service(
        App::new()
            .app_data(web::Data::new(fixture.provider_state.follower_view()))
            .service(
                web::scope("/api/v1").service(aster_drive::api::routes::internal_storage::routes()),
            ),
    )
    .await;

    let req = test::TestRequest::put()
        .uri(&presigned_path)
        .insert_header(("Origin", "http://localhost:3000"))
        .insert_header((
            actix_web::http::header::CONTENT_TYPE,
            "application/octet-stream",
        ))
        .insert_header((actix_web::http::header::CONTENT_LENGTH, "4"))
        .set_payload(b"test".as_slice())
        .to_request();
    let resp = test::call_service(&follower_app, req).await;

    assert_eq!(resp.status(), actix_web::http::StatusCode::UNAUTHORIZED);
    assert!(
        !resp
            .headers()
            .contains_key(actix_web::http::header::ACCESS_CONTROL_ALLOW_ORIGIN),
        "middleware should leave missing access_key requests to auth layer"
    );
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["code"], 2002);
    assert_eq!(body["msg"], "missing query parameter 'aster_access_key'");
}

#[actix_web::test]
async fn test_remote_presigned_upload_browser_cors_invalid_origin_header_returns_bad_request() {
    let fixture = setup_browser_presigned_cors_fixture(
        "provider-browser-invalid-origin-space",
        "http://localhost:3000",
    )
    .await;
    let follower_app = test::init_service(
        App::new()
            .app_data(web::Data::new(fixture.provider_state.follower_view()))
            .service(
                web::scope("/api/v1").service(aster_drive::api::routes::internal_storage::routes()),
            ),
    )
    .await;

    let req = test::TestRequest::default()
        .method(actix_web::http::Method::OPTIONS)
        .uri(&fixture.presigned_path)
        .insert_header(("Origin", "http://localhost:3000/admin"))
        .insert_header(("Access-Control-Request-Method", "PUT"))
        .insert_header(("Access-Control-Request-Headers", "content-type"))
        .to_request();
    let err = test::try_call_service(&follower_app, req)
        .await
        .expect_err("invalid Origin header should be rejected before routing");
    let resp = err.error_response();

    assert_eq!(resp.status(), actix_web::http::StatusCode::BAD_REQUEST);
    assert!(
        !resp
            .headers()
            .contains_key(actix_web::http::header::ACCESS_CONTROL_ALLOW_ORIGIN),
        "invalid Origin should fail before emitting CORS allow headers"
    );
    let body = actix_web::body::to_bytes(resp.into_body())
        .await
        .expect("bad request body should be readable");
    let body: serde_json::Value =
        serde_json::from_slice(&body).expect("bad request body should be valid json");
    assert_eq!(body["code"], 1000);
    assert_eq!(
        body["msg"],
        "CORS origin must not include a path: 'http://localhost:3000/admin'"
    );
}

#[actix_web::test]
async fn test_remote_presigned_upload_browser_cors_invalid_preflight_header_name_returns_bad_request()
 {
    let fixture = setup_browser_presigned_cors_fixture(
        "provider-browser-invalid-header-name-space",
        "http://localhost:3000",
    )
    .await;
    let follower_app = test::init_service(
        App::new()
            .app_data(web::Data::new(fixture.provider_state.follower_view()))
            .service(
                web::scope("/api/v1").service(aster_drive::api::routes::internal_storage::routes()),
            ),
    )
    .await;

    let req = test::TestRequest::default()
        .method(actix_web::http::Method::OPTIONS)
        .uri(&fixture.presigned_path)
        .insert_header(("Origin", "http://localhost:3000"))
        .insert_header(("Access-Control-Request-Method", "PUT"))
        .insert_header(("Access-Control-Request-Headers", "content-type, @bad"))
        .to_request();
    let err = test::try_call_service(&follower_app, req)
        .await
        .expect_err("invalid preflight header name should be rejected");
    let resp = err.error_response();

    assert_eq!(resp.status(), actix_web::http::StatusCode::BAD_REQUEST);
    let body = actix_web::body::to_bytes(resp.into_body())
        .await
        .expect("bad request body should be readable");
    let body: serde_json::Value =
        serde_json::from_slice(&body).expect("bad request body should be valid json");
    assert_eq!(body["code"], 1000);
    assert_eq!(body["msg"], "invalid Access-Control-Request-Headers");
}

#[actix_web::test]
async fn test_remote_presigned_upload_browser_cors_keeps_headers_on_expired_signature() {
    let fixture = setup_browser_presigned_cors_fixture(
        "provider-browser-expired-signature-space",
        "http://localhost:3000",
    )
    .await;
    let expired_path =
        rewrite_path_query_param(&fixture.presigned_path, "aster_expires", Some("1"));
    let follower_app = test::init_service(
        App::new()
            .app_data(web::Data::new(fixture.provider_state.follower_view()))
            .service(
                web::scope("/api/v1").service(aster_drive::api::routes::internal_storage::routes()),
            ),
    )
    .await;

    let req = test::TestRequest::put()
        .uri(&expired_path)
        .insert_header(("Origin", "http://localhost:3000"))
        .insert_header((
            actix_web::http::header::CONTENT_TYPE,
            "application/octet-stream",
        ))
        .insert_header((actix_web::http::header::CONTENT_LENGTH, "4"))
        .set_payload(b"test".as_slice())
        .to_request();
    let resp = test::call_service(&follower_app, req).await;

    assert_eq!(resp.status(), actix_web::http::StatusCode::UNAUTHORIZED);
    assert_eq!(
        resp.headers()
            .get(actix_web::http::header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .and_then(|value| value.to_str().ok()),
        Some("http://localhost:3000")
    );
    assert_eq!(
        resp.headers()
            .get(actix_web::http::header::ACCESS_CONTROL_EXPOSE_HEADERS)
            .and_then(|value| value.to_str().ok()),
        Some("ETag")
    );
    let vary = resp
        .headers()
        .get(actix_web::http::header::VARY)
        .and_then(|value| value.to_str().ok())
        .expect("expired presigned response should include Vary");
    assert!(vary.contains("Origin"));
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["code"], 2002);
    assert_eq!(body["msg"], "remote presigned URL has expired");
}

#[actix_web::test]
async fn test_remote_presigned_upload_browser_cors_keeps_headers_on_signature_mismatch() {
    let fixture = setup_browser_presigned_cors_fixture(
        "provider-browser-signature-mismatch-space",
        "http://localhost:3000",
    )
    .await;
    let bad_signature_path =
        rewrite_path_query_param(&fixture.presigned_path, "aster_signature", Some("deadbeef"));
    let follower_app = test::init_service(
        App::new()
            .app_data(web::Data::new(fixture.provider_state.follower_view()))
            .service(
                web::scope("/api/v1").service(aster_drive::api::routes::internal_storage::routes()),
            ),
    )
    .await;

    let req = test::TestRequest::put()
        .uri(&bad_signature_path)
        .insert_header(("Origin", "http://localhost:3000"))
        .insert_header((
            actix_web::http::header::CONTENT_TYPE,
            "application/octet-stream",
        ))
        .insert_header((actix_web::http::header::CONTENT_LENGTH, "4"))
        .set_payload(b"test".as_slice())
        .to_request();
    let resp = test::call_service(&follower_app, req).await;

    assert_eq!(resp.status(), actix_web::http::StatusCode::UNAUTHORIZED);
    assert_eq!(
        resp.headers()
            .get(actix_web::http::header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .and_then(|value| value.to_str().ok()),
        Some("http://localhost:3000")
    );
    assert_eq!(
        resp.headers()
            .get(actix_web::http::header::ACCESS_CONTROL_EXPOSE_HEADERS)
            .and_then(|value| value.to_str().ok()),
        Some("ETag")
    );
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["code"], 2000);
    assert_eq!(body["msg"], "remote presigned signature mismatch");
}

#[actix_web::test]
async fn test_remote_presigned_upload_browser_cors_accepts_master_url_with_default_https_port() {
    let fixture = setup_browser_presigned_cors_fixture(
        "provider-browser-default-port-space",
        " HTTPS://LOCALHOST:443/admin/settings/general ",
    )
    .await;
    let follower_app = test::init_service(
        App::new()
            .app_data(web::Data::new(fixture.provider_state.follower_view()))
            .service(
                web::scope("/api/v1").service(aster_drive::api::routes::internal_storage::routes()),
            ),
    )
    .await;

    let req = test::TestRequest::default()
        .method(actix_web::http::Method::OPTIONS)
        .uri(&fixture.presigned_path)
        .insert_header(("Origin", "https://localhost"))
        .insert_header(("Access-Control-Request-Method", "PUT"))
        .insert_header(("Access-Control-Request-Headers", "content-type"))
        .to_request();
    let resp = test::call_service(&follower_app, req).await;

    assert_eq!(resp.status(), actix_web::http::StatusCode::NO_CONTENT);
    assert_eq!(
        resp.headers()
            .get(actix_web::http::header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .and_then(|value| value.to_str().ok()),
        Some("https://localhost")
    );
}

#[actix_web::test]
async fn test_remote_presigned_multipart_upload_composes_on_provider_without_assembled_temp() {
    let provider_state = common::setup().await;
    let consumer_state = common::setup().await;
    let provider_server = spawn_internal_storage_server(provider_state.follower_view()).await;

    let provider_ingress_policy = provider_state
        .policy_snapshot
        .system_default_policy()
        .expect("provider default ingress policy should exist");

    let consumer_node = managed_follower_service::create(
        &consumer_state,
        managed_follower_service::CreateRemoteNodeInput {
            name: "provider-target".to_string(),
            base_url: provider_server.base_url.clone(),
            namespace: "provider-presigned-multipart-space".to_string(),
            is_enabled: true,
        },
    )
    .await
    .expect("consumer remote node should be created");
    let consumer_node_model =
        managed_follower_repo::find_by_id(&consumer_state.db, consumer_node.id)
            .await
            .expect("consumer remote node should be queryable");

    master_binding_service::upsert_from_enrollment(
        &provider_state.db,
        master_binding_service::UpsertMasterBindingInput {
            name: "consumer-access".to_string(),
            master_url: "http://master.example.com".to_string(),
            access_key: consumer_node_model.access_key.clone(),
            secret_key: consumer_node_model.secret_key.clone(),
            namespace: "provider-presigned-multipart-space".to_string(),
            ingress_policy_id: provider_ingress_policy.id,
            is_enabled: true,
        },
    )
    .await
    .expect("provider master binding should be created");
    provider_state
        .driver_registry
        .reload_master_bindings(&provider_state.db)
        .await
        .expect("provider binding registry should reload");

    wait_for_remote_probe(&consumer_state, consumer_node.id).await;

    let remote_policy = create_remote_policy_with_options(
        &consumer_state,
        consumer_node.id,
        "Remote Presigned Multipart Policy",
        "presigned-multipart-base",
        StoragePolicyOptions {
            remote_upload_strategy: Some(RemoteUploadStrategy::Presigned),
            ..Default::default()
        },
        4,
    )
    .await;

    let app = create_test_app!(consumer_state.clone());
    let _ = register_and_login!(app);
    let user = user_repo::find_by_username(&consumer_state.db, "testuser")
        .await
        .expect("test user lookup should succeed")
        .expect("test user should exist");
    let folder =
        folder_service::create(&consumer_state, user.id, "remote-presigned-multipart", None)
            .await
            .expect("remote folder should be created");
    folder_service::update(
        &consumer_state,
        folder.id,
        user.id,
        None,
        NullablePatch::Absent,
        NullablePatch::Value(remote_policy.id),
    )
    .await
    .expect("remote policy should bind to folder");

    let body = b"multipart-remote-upload".to_vec();
    let init = upload_service::init_upload(
        &consumer_state,
        user.id,
        "multipart.bin",
        i64::try_from(body.len()).expect("body length should fit i64"),
        Some(folder.id),
        None,
    )
    .await
    .expect("remote presigned multipart upload should initialize");
    assert_eq!(
        init.mode,
        aster_drive::types::UploadMode::PresignedMultipart
    );

    let upload_id = init
        .upload_id
        .clone()
        .expect("presigned multipart mode should return upload id");
    let session = upload_session_repo::find_by_id(&consumer_state.db, &upload_id)
        .await
        .expect("upload session should exist");
    let remote_multipart_id = session
        .s3_multipart_id
        .clone()
        .expect("remote multipart upload id should be stored");
    let chunk_size = usize::try_from(
        init.chunk_size
            .expect("presigned multipart mode should return chunk size"),
    )
    .expect("chunk size should fit usize");
    let total_chunks = usize::try_from(
        init.total_chunks
            .expect("presigned multipart mode should return total_chunks"),
    )
    .expect("total_chunks should fit usize");

    let requested_parts =
        (1..=i32::try_from(total_chunks).expect("chunk count should fit i32")).collect::<Vec<_>>();
    let urls = upload_service::presign_parts(&consumer_state, &upload_id, user.id, requested_parts)
        .await
        .expect("presign multipart part URLs should succeed");

    let mut completed_parts = Vec::with_capacity(total_chunks);
    for part_number in 1..=total_chunks {
        let start = (part_number - 1) * chunk_size;
        let end = std::cmp::min(start + chunk_size, body.len());
        let part_body = body[start..end].to_vec();
        let url = urls
            .get(&i32::try_from(part_number).expect("part number should fit i32"))
            .expect("part URL should exist");
        let response = put_presigned_bytes(url, part_body).await;
        assert_eq!(response.status(), reqwest::StatusCode::OK);
        let etag = response
            .headers()
            .get(reqwest::header::ETAG)
            .and_then(|value| value.to_str().ok())
            .expect("multipart part upload should return ETag")
            .trim_matches('"')
            .to_string();
        completed_parts.push((
            i32::try_from(part_number).expect("part number should fit i32"),
            etag,
        ));
    }

    let progress = upload_service::get_progress(&consumer_state, &upload_id, user.id)
        .await
        .expect("multipart upload progress should be queryable");
    assert_eq!(
        progress.chunks_on_disk,
        (1..=i32::try_from(total_chunks).expect("chunk count should fit i32")).collect::<Vec<_>>()
    );

    let temp_dir = aster_drive::utils::paths::upload_temp_dir(
        &consumer_state.config.server.upload_temp_dir,
        &upload_id,
    );
    let assembled_path = aster_drive::utils::paths::upload_assembled_path(
        &consumer_state.config.server.upload_temp_dir,
        &upload_id,
    );
    assert!(
        !tokio::fs::try_exists(&temp_dir)
            .await
            .expect("temp dir existence should be readable"),
        "remote presigned multipart upload should not create local chunk temp dir"
    );
    assert!(
        !tokio::fs::try_exists(&assembled_path)
            .await
            .expect("assembled path existence should be readable"),
        "remote presigned multipart upload should not create local assembled temp file"
    );

    let created = upload_service::complete_upload(
        &consumer_state,
        &upload_id,
        user.id,
        Some(completed_parts),
    )
    .await
    .expect("remote presigned multipart upload should complete");
    let created_file = file_repo::find_by_id(&consumer_state.db, created.id)
        .await
        .expect("uploaded file should be queryable");
    let created_blob = file_repo::find_blob_by_id(&consumer_state.db, created_file.blob_id)
        .await
        .expect("uploaded blob should be queryable");

    let stored_path = provider_object_path(
        &provider_ingress_policy.base_path,
        &consumer_node.namespace,
        &remote_policy.base_path,
        &created_blob.storage_path,
    );
    let stored = tokio::fs::read(&stored_path)
        .await
        .expect("provider should receive composed multipart upload");
    assert_eq!(stored, body);

    let first_part_path = provider_object_path(
        &provider_ingress_policy.base_path,
        &consumer_node.namespace,
        &remote_policy.base_path,
        &format!("uploads/{remote_multipart_id}/parts/1"),
    );
    assert!(
        !tokio::fs::try_exists(&first_part_path)
            .await
            .expect("part path existence should be readable"),
        "remote multipart compose should cleanup follower temp parts"
    );

    provider_server.stop().await;
}
