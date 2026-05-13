use super::paths::{normalize_relative_local_path, resolve_managed_local_path};
use std::fs;

#[test]
fn normalize_relative_local_path_keeps_normal_segments() {
    let normalized = normalize_relative_local_path(" archive/2026 ").unwrap();
    assert_eq!(normalized, "archive/2026");
}

#[test]
fn normalize_relative_local_path_rejects_escape_attempts() {
    let error = normalize_relative_local_path("../secret").unwrap_err();
    assert!(
        error
            .message()
            .contains("server.follower.managed_ingress_local_root")
    );
}

#[test]
fn normalize_relative_local_path_rejects_backslash_escape_attempts() {
    let error = normalize_relative_local_path("..\\secret").unwrap_err();
    assert!(
        error
            .message()
            .contains("server.follower.managed_ingress_local_root")
    );
}

#[test]
fn resolve_managed_local_path_allows_missing_child_inside_root() {
    let root = std::env::temp_dir().join(format!(
        "aster-managed-ingress-root-{}",
        uuid::Uuid::new_v4()
    ));
    fs::create_dir_all(&root).unwrap();

    let resolved = resolve_managed_local_path(root.to_str().unwrap(), "profiles/new").unwrap();
    assert_eq!(
        resolved,
        fs::canonicalize(&root)
            .unwrap()
            .join("profiles")
            .join("new")
    );

    let _ = fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[test]
fn resolve_managed_local_path_rejects_symlink_escape() {
    let root = std::env::temp_dir().join(format!(
        "aster-managed-ingress-root-{}",
        uuid::Uuid::new_v4()
    ));
    let outside = std::env::temp_dir().join(format!(
        "aster-managed-ingress-outside-{}",
        uuid::Uuid::new_v4()
    ));
    fs::create_dir_all(&root).unwrap();
    fs::create_dir_all(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, root.join("escape")).unwrap();

    let error = resolve_managed_local_path(root.to_str().unwrap(), "escape/profile").unwrap_err();
    assert!(
        error
            .message()
            .contains("server.follower.managed_ingress_local_root")
    );

    let _ = fs::remove_dir_all(&root);
    let _ = fs::remove_dir_all(&outside);
}
