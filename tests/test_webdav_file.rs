#[macro_use]
mod common;

use std::io::SeekFrom;

use aster_drive::webdav::dav::{DavFile, FsError};
use bytes::Bytes;

fn write_temp_fixture(name: &str, contents: &str) -> String {
    let dir = format!("/tmp/asterdrive-webdav-file-test-{}", uuid::Uuid::new_v4());
    std::fs::create_dir_all(&dir).unwrap();
    let path = format!("{dir}/{name}");
    std::fs::write(&path, contents).unwrap();
    path
}

#[actix_web::test]
async fn test_aster_dav_file_read_mode_enforces_read_only_behaviour() {
    use aster_drive::webdav::file::AsterDavFile;
    use aster_drive::webdav::metadata::AsterDavMeta;

    let temp_path = write_temp_fixture("read-only.txt", "abcdef");
    let file = tokio::fs::File::open(&temp_path).await.unwrap();
    let mut dav_file = AsterDavFile::for_read(file, temp_path, AsterDavMeta::root());

    dav_file.metadata().await.unwrap();

    assert_eq!(
        dav_file.read_bytes(3).await.unwrap(),
        Bytes::from_static(b"abc")
    );
    assert_eq!(dav_file.seek(SeekFrom::Start(0)).await.unwrap(), 0);
    assert_eq!(
        dav_file.read_bytes(6).await.unwrap(),
        Bytes::from_static(b"abcdef")
    );
    assert!(dav_file.read_bytes(1).await.unwrap().is_empty());
    assert!(matches!(
        dav_file.write_bytes(Bytes::from_static(b"x")).await,
        Err(FsError::Forbidden)
    ));
    assert!(matches!(
        dav_file.write_buf(Box::new(Bytes::from_static(b"x"))).await,
        Err(FsError::Forbidden)
    ));
}

#[actix_web::test]
async fn test_aster_dav_file_write_mode_skips_empty_flush_and_persists_written_content() {
    use aster_drive::db::repository::file_repo;
    use aster_drive::services::auth_service;
    use aster_drive::webdav::file::AsterDavFile;

    let state = common::setup().await;
    let user = auth_service::register(
        &state,
        "davfilewriter",
        "davfilewriter@example.com",
        "pass1234",
    )
    .await
    .unwrap();

    let mut empty_file = AsterDavFile::for_write(
        state.clone(),
        user.id,
        None,
        "empty-dav-file.txt".to_string(),
        None,
    )
    .await
    .unwrap();

    empty_file.metadata().await.unwrap();
    assert!(matches!(
        empty_file.read_bytes(1).await,
        Err(FsError::Forbidden)
    ));
    assert_eq!(empty_file.seek(SeekFrom::Start(0)).await.unwrap(), 0);
    empty_file.flush().await.unwrap();

    assert!(
        file_repo::find_by_name_in_folder(&state.db, user.id, None, "empty-dav-file.txt")
            .await
            .unwrap()
            .is_none()
    );

    let mut written_file = AsterDavFile::for_write(
        state.clone(),
        user.id,
        None,
        "buffered-dav-file.txt".to_string(),
        None,
    )
    .await
    .unwrap();

    written_file
        .write_bytes(Bytes::from_static(b"hello "))
        .await
        .unwrap();
    assert_eq!(written_file.seek(SeekFrom::Current(0)).await.unwrap(), 6);
    written_file
        .write_buf(Box::new(Bytes::from_static(b"world")))
        .await
        .unwrap();
    written_file.flush().await.unwrap();

    let stored =
        file_repo::find_by_name_in_folder(&state.db, user.id, None, "buffered-dav-file.txt")
            .await
            .unwrap()
            .expect("buffered WebDAV flush should create a file record");
    assert_eq!(stored.size, 11);
}

#[actix_web::test]
async fn test_aster_dav_fs_reports_quota_and_roundtrips_custom_props() {
    use aster_drive::db::repository::user_repo;
    use aster_drive::services::{auth_service, file_service};
    use aster_drive::webdav::dav::{DavFileSystem, DavPath, DavProp};
    use aster_drive::webdav::fs::AsterDavFs;
    use sea_orm::{ActiveModelTrait, Set};

    let state = common::setup().await;
    let user = auth_service::register(&state, "davfsprops", "davfsprops@example.com", "pass1234")
        .await
        .unwrap();

    let content = "quota props";
    let temp_path = write_temp_fixture("quota-props.txt", content);
    file_service::store_from_temp(
        &state,
        user.id,
        None,
        "quota-props.txt",
        &temp_path,
        content.len() as i64,
        None,
        false,
    )
    .await
    .unwrap();

    let dav_fs = AsterDavFs::new(state.clone(), user.id, None);
    let file_path = DavPath::new("/quota-props.txt").unwrap();

    assert!(!dav_fs.have_props(&file_path).await);

    let (used, total) = dav_fs.get_quota().await.unwrap();
    assert_eq!(used, content.len() as u64);
    assert_eq!(total, None);

    let mut updated_user: aster_drive::entities::user::ActiveModel =
        user_repo::find_by_id(&state.db, user.id)
            .await
            .unwrap()
            .into();
    updated_user.storage_quota = Set(128);
    updated_user.update(&state.db).await.unwrap();

    let (used, total) = dav_fs.get_quota().await.unwrap();
    assert_eq!(used, content.len() as u64);
    assert_eq!(total, Some(128));

    let set_results = dav_fs
        .patch_props(
            &file_path,
            vec![(
                true,
                DavProp {
                    name: "color".to_string(),
                    prefix: None,
                    namespace: Some("urn:aster:test".to_string()),
                    xml: Some(b"blue".to_vec()),
                },
            )],
        )
        .await
        .unwrap();
    assert_eq!(set_results.len(), 1);
    assert_eq!(set_results[0].0, http::StatusCode::OK);
    assert!(dav_fs.have_props(&file_path).await);

    let props_without_content = dav_fs.get_props(&file_path, false).await.unwrap();
    assert_eq!(props_without_content.len(), 1);
    assert_eq!(
        props_without_content[0].namespace.as_deref(),
        Some("urn:aster:test")
    );
    assert!(props_without_content[0].xml.is_none());

    let props_with_content = dav_fs.get_props(&file_path, true).await.unwrap();
    assert_eq!(props_with_content.len(), 1);
    assert_eq!(props_with_content[0].xml.as_deref(), Some(&b"blue"[..]));

    let remove_results = dav_fs
        .patch_props(
            &file_path,
            vec![(
                false,
                DavProp {
                    name: "color".to_string(),
                    prefix: None,
                    namespace: Some("urn:aster:test".to_string()),
                    xml: None,
                },
            )],
        )
        .await
        .unwrap();
    assert_eq!(remove_results.len(), 1);
    assert_eq!(remove_results[0].0, http::StatusCode::OK);
    assert!(!dav_fs.have_props(&file_path).await);

    let missing_path = DavPath::new("/missing.txt").unwrap();
    assert!(matches!(
        dav_fs.get_props(&missing_path, false).await,
        Err(FsError::NotFound)
    ));
}
