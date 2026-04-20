//! 远端存储集成测试。

#[macro_use]
mod common;

use std::path::{Path, PathBuf};
use std::time::Duration;

use actix_web::{App, HttpServer, web};
use aster_drive::db::repository::{file_repo, policy_repo};
use aster_drive::entities::storage_policy;
use aster_drive::services::{
    auth_service, file_service, folder_service, master_binding_service, remote_node_service,
};
use aster_drive::types::{
    DriverType, NullablePatch, StoredStoragePolicyAllowedTypes, StoredStoragePolicyOptions,
};
use chrono::Utc;
use futures::TryStreamExt;
use sea_orm::Set;

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
            options: Set(StoredStoragePolicyOptions::empty()),
            is_default: Set(false),
            chunk_size: Set(5_242_880),
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

async fn wait_for_remote_probe(
    state: &aster_drive::runtime::PrimaryAppState,
    node_id: i64,
) -> remote_node_service::RemoteNodeInfo {
    for attempt in 0..20 {
        match remote_node_service::test_connection(state, node_id).await {
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

#[actix_web::test]
async fn test_remote_storage_end_to_end_via_internal_api() {
    let provider_state = common::setup().await;
    let consumer_state = common::setup().await;
    let provider_server = spawn_internal_storage_server(provider_state.follower_view()).await;

    let provider_ingress_policy = provider_state
        .policy_snapshot
        .system_default_policy()
        .expect("provider default ingress policy should exist");

    let (provider_binding, _) = master_binding_service::upsert_from_enrollment(
        &provider_state.db,
        master_binding_service::UpsertMasterBindingInput {
            name: "consumer-access".to_string(),
            master_url: "http://master.example.com".to_string(),
            access_key: "remote-test-access".to_string(),
            secret_key: "remote-test-secret".to_string(),
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

    let consumer_node = remote_node_service::create(
        &consumer_state,
        remote_node_service::CreateRemoteNodeInput {
            name: "provider-target".to_string(),
            base_url: provider_server.base_url.clone(),
            access_key: "remote-test-access".to_string(),
            secret_key: "remote-test-secret".to_string(),
            namespace: "provider-space".to_string(),
            is_enabled: true,
        },
    )
    .await
    .expect("consumer remote node should be created");

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
