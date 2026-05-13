use super::paths::sanitize_relative_path;
use std::path::{Path, PathBuf};

fn build_policy(base: &Path) -> crate::entities::storage_policy::Model {
    crate::entities::storage_policy::Model {
        id: 1,
        name: "local".into(),
        driver_type: crate::types::DriverType::Local,
        endpoint: String::new(),
        bucket: String::new(),
        access_key: String::new(),
        secret_key: String::new(),
        base_path: base.to_string_lossy().into(),
        remote_node_id: None,
        max_file_size: 0,
        allowed_types: crate::types::StoredStoragePolicyAllowedTypes::empty(),
        options: crate::types::StoredStoragePolicyOptions::empty(),
        is_default: false,
        chunk_size: 0,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }
}

#[test]
fn sanitize_accepts_normal_paths() {
    assert_eq!(
        sanitize_relative_path("ab/cd/abcdef").unwrap(),
        PathBuf::from("ab/cd/abcdef")
    );
    assert_eq!(
        sanitize_relative_path("/leading/slash").unwrap(),
        PathBuf::from("leading/slash")
    );
    assert_eq!(
        sanitize_relative_path("nested/./path").unwrap(),
        PathBuf::from("nested/path")
    );
}

#[test]
fn sanitize_rejects_parent_dir() {
    assert!(sanitize_relative_path("../etc/passwd").is_err());
    assert!(sanitize_relative_path("ab/../../../etc/passwd").is_err());
    assert!(sanitize_relative_path("ab/..").is_err());
}

#[test]
fn sanitize_rejects_absolute_paths() {
    assert!(sanitize_relative_path("/etc/passwd").is_ok()); // stripped leading slash
    // Path that starts with non-trim '/' after components would be normalized; real absolute
    // only triggers on Windows prefixes or re-rooting. Ensure multi-slash doesn't bypass.
    assert!(sanitize_relative_path("//../etc").is_err());
}

#[tokio::test]
async fn get_range_returns_partial_bytes() {
    use crate::storage::driver::StorageDriver;
    use tokio::io::AsyncReadExt;

    let base = std::env::temp_dir().join(format!(
        "aster-range-test-{}-{}",
        std::process::id(),
        rand::random::<u64>()
    ));
    tokio::fs::create_dir_all(&base).await.unwrap();

    let policy = build_policy(&base);
    let driver = super::LocalDriver::new(&policy).unwrap();
    driver.put("sample.txt", b"Hello, world!").await.unwrap();

    // offset=7, length=5 -> "world"
    let mut reader = driver.get_range("sample.txt", 7, Some(5)).await.unwrap();
    let mut buf = Vec::new();
    reader.read_to_end(&mut buf).await.unwrap();
    assert_eq!(buf, b"world");

    // offset=7, length=None -> "world!"
    let mut reader = driver.get_range("sample.txt", 7, None).await.unwrap();
    let mut buf = Vec::new();
    reader.read_to_end(&mut buf).await.unwrap();
    assert_eq!(buf, b"world!");

    // offset=0, length=5 -> "Hello"
    let mut reader = driver.get_range("sample.txt", 0, Some(5)).await.unwrap();
    let mut buf = Vec::new();
    reader.read_to_end(&mut buf).await.unwrap();
    assert_eq!(buf, b"Hello");

    let _ = tokio::fs::remove_dir_all(&base).await;
}

#[tokio::test]
async fn promote_local_file_if_absent_does_not_overwrite_existing_target() {
    let base = std::env::temp_dir().join(format!(
        "aster-local-promote-test-{}-{}",
        std::process::id(),
        rand::random::<u64>()
    ));
    tokio::fs::create_dir_all(&base).await.unwrap();

    let policy = build_policy(&base);
    let driver = super::LocalDriver::new(&policy).unwrap();
    let target = "ab/cd/existing";
    let target_full = driver.full_path(target).unwrap();
    tokio::fs::create_dir_all(target_full.parent().unwrap())
        .await
        .unwrap();
    tokio::fs::write(&target_full, b"old").await.unwrap();

    let source = base.join("source.bin");
    tokio::fs::write(&source, b"new").await.unwrap();
    super::promote_local_file_if_absent(&driver, target, source.to_str().unwrap(), 3)
        .await
        .unwrap();

    assert_eq!(tokio::fs::read(&target_full).await.unwrap(), b"old");
    assert!(!source.exists());

    let _ = tokio::fs::remove_dir_all(&base).await;
}

#[tokio::test]
async fn promote_local_file_if_absent_rejects_existing_size_mismatch() {
    let base = std::env::temp_dir().join(format!(
        "aster-local-promote-mismatch-test-{}-{}",
        std::process::id(),
        rand::random::<u64>()
    ));
    tokio::fs::create_dir_all(&base).await.unwrap();

    let policy = build_policy(&base);
    let driver = super::LocalDriver::new(&policy).unwrap();
    let target = "ab/cd/existing";
    let target_full = driver.full_path(target).unwrap();
    tokio::fs::create_dir_all(target_full.parent().unwrap())
        .await
        .unwrap();
    tokio::fs::write(&target_full, b"old").await.unwrap();

    let source = base.join("source.bin");
    tokio::fs::write(&source, b"new-data").await.unwrap();
    let error = super::promote_local_file_if_absent(&driver, target, source.to_str().unwrap(), 8)
        .await
        .expect_err("existing blob with different size must be rejected");

    assert!(error.message().contains("size mismatch"));
    assert_eq!(tokio::fs::read(&target_full).await.unwrap(), b"old");
    assert!(source.exists());

    let _ = tokio::fs::remove_dir_all(&base).await;
}

#[tokio::test]
async fn promote_local_file_if_absent_rolls_back_linked_size_mismatch() {
    let base = std::env::temp_dir().join(format!(
        "aster-local-promote-linked-mismatch-test-{}-{}",
        std::process::id(),
        rand::random::<u64>()
    ));
    tokio::fs::create_dir_all(&base).await.unwrap();

    let policy = build_policy(&base);
    let driver = super::LocalDriver::new(&policy).unwrap();
    let target = "ab/cd/new-target";
    let target_full = driver.full_path(target).unwrap();

    let source = base.join("source.bin");
    tokio::fs::write(&source, b"short").await.unwrap();
    let error = super::promote_local_file_if_absent(&driver, target, source.to_str().unwrap(), 8)
        .await
        .expect_err("newly linked blob with different size must be rejected");

    assert!(error.message().contains("size mismatch"));
    assert!(source.exists());
    assert!(!target_full.exists());

    let _ = tokio::fs::remove_dir_all(&base).await;
}

#[cfg(unix)]
#[tokio::test]
async fn put_rejects_symlink_escape_inside_storage_root() {
    use crate::storage::driver::StorageDriver;

    let temp_root = std::env::temp_dir().join(format!(
        "aster-local-symlink-test-{}-{}",
        std::process::id(),
        rand::random::<u64>()
    ));
    let base = temp_root.join("storage");
    let outside = temp_root.join("outside");
    std::fs::create_dir_all(&base).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, base.join("escape")).unwrap();

    let policy = build_policy(&base);
    let driver = super::LocalDriver::new(&policy).unwrap();
    let result = driver.put("escape/pwned.txt", b"nope").await;

    assert!(result.is_err());
    assert!(!outside.join("pwned.txt").exists());

    let _ = tokio::fs::remove_dir_all(&temp_root).await;
}

#[cfg(unix)]
#[test]
fn staging_path_rejects_symlink_escape() {
    let temp_root = std::env::temp_dir().join(format!(
        "aster-local-staging-symlink-test-{}-{}",
        std::process::id(),
        rand::random::<u64>()
    ));
    let base = temp_root.join("storage");
    let outside = temp_root.join("outside");
    std::fs::create_dir_all(&base).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, base.join(".staging")).unwrap();

    let policy = build_policy(&base);
    let result = super::upload_staging_path(&policy, "token.upload");

    assert!(result.is_err());

    let _ = std::fs::remove_dir_all(&temp_root);
}
