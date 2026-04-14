#[macro_use]
mod common;

use std::collections::BTreeSet;

fn write_service_fixture(name: &str, contents: &str) -> String {
    let dir = format!("/tmp/asterdrive-services-test-{}", uuid::Uuid::new_v4());
    std::fs::create_dir_all(&dir).unwrap();
    let path = format!("{dir}/{name}");
    std::fs::write(&path, contents).unwrap();
    path
}

async fn store_service_file(
    state: &aster_drive::runtime::AppState,
    user_id: i64,
    folder_id: Option<i64>,
    name: &str,
    contents: &str,
) -> i64 {
    let path = write_service_fixture(name, contents);
    aster_drive::services::file_service::store_from_temp(
        state,
        user_id,
        folder_id,
        name,
        &path,
        contents.len() as i64,
        None,
        false,
    )
    .await
    .unwrap()
    .id
}

async fn user_storage_used(state: &aster_drive::runtime::AppState, user_id: i64) -> i64 {
    aster_drive::db::repository::user_repo::find_by_id(&state.db, user_id)
        .await
        .unwrap()
        .storage_used
}

// ─── Auth Service ─────────────────────────────────────────────────

#[actix_web::test]
async fn test_auth_service_register_login() {
    let state = common::setup().await;

    // 注册
    let user = aster_drive::services::auth_service::register(
        &state,
        "alice",
        "alice@example.com",
        "password123",
    )
    .await
    .unwrap();
    assert_eq!(user.username, "alice");

    // 第一个用户是 admin
    assert!(user.role.is_admin());

    // 登录 → LoginResult { access_token, refresh_token, user_id }
    let result = aster_drive::services::auth_service::login(&state, "alice", "password123")
        .await
        .unwrap();
    assert!(!result.access_token.is_empty());
    assert!(!result.refresh_token.is_empty());
    assert_eq!(result.user_id, user.id);

    // 错误密码
    let err = aster_drive::services::auth_service::login(&state, "alice", "wrongpass").await;
    assert!(err.is_err());

    // 重复注册
    let err = aster_drive::services::auth_service::register(
        &state,
        "alice",
        "alice2@example.com",
        "password123",
    )
    .await;
    assert!(err.is_err());
}

#[actix_web::test]
async fn test_auth_service_rejects_password_shorter_than_eight_chars() {
    let state = common::setup().await;

    let err = aster_drive::services::auth_service::register(
        &state,
        "alice",
        "alice@example.com",
        "pass123",
    )
    .await
    .unwrap_err();

    assert_eq!(err.message(), "password must be at least 8 characters");
}

#[actix_web::test]
async fn test_auth_service_change_password() {
    let state = common::setup().await;

    let user = aster_drive::services::auth_service::register(
        &state,
        "alice",
        "alice@example.com",
        "password123",
    )
    .await
    .unwrap();

    aster_drive::services::auth_service::change_password(
        &state,
        user.id,
        "password123",
        "newpass456",
    )
    .await
    .unwrap();

    let old_login =
        aster_drive::services::auth_service::login(&state, "alice", "password123").await;
    assert!(old_login.is_err());

    let new_login = aster_drive::services::auth_service::login(&state, "alice", "newpass456")
        .await
        .unwrap();
    assert_eq!(new_login.user_id, user.id);
}

#[actix_web::test]
async fn test_auth_service_set_password() {
    let state = common::setup().await;

    let user = aster_drive::services::auth_service::register(
        &state,
        "alice",
        "alice@example.com",
        "password123",
    )
    .await
    .unwrap();

    aster_drive::services::auth_service::set_password(&state, user.id, "resetpass789")
        .await
        .unwrap();

    let old_login =
        aster_drive::services::auth_service::login(&state, "alice", "password123").await;
    assert!(old_login.is_err());

    let new_login = aster_drive::services::auth_service::login(&state, "alice", "resetpass789")
        .await
        .unwrap();
    assert_eq!(new_login.user_id, user.id);
}

#[actix_web::test]
async fn test_auth_service_verify_token() {
    let state = common::setup().await;

    aster_drive::services::auth_service::register(&state, "bobb", "bob@example.com", "pass1234")
        .await
        .unwrap();

    let login_result = aster_drive::services::auth_service::login(&state, "bobb", "pass1234")
        .await
        .unwrap();

    // 验证 access token
    let claims = aster_drive::services::auth_service::verify_token(
        &login_result.access_token,
        &state.config.auth.jwt_secret,
    )
    .unwrap();
    assert_eq!(claims.sub, claims.user_id.to_string());

    // 假 token
    let err = aster_drive::services::auth_service::verify_token(
        "fake.token.here",
        &state.config.auth.jwt_secret,
    );
    assert!(err.is_err());
}

// ─── File Service ─────────────────────────────────────────────────

#[actix_web::test]
async fn test_file_service_get_info() {
    let state = common::setup().await;

    let user =
        aster_drive::services::auth_service::register(&state, "user1", "u1@example.com", "pass1234")
            .await
            .unwrap();

    // 上传临时文件
    let temp_dir = format!("/tmp/asterdrive-svc-test-{}", uuid::Uuid::new_v4());
    std::fs::create_dir_all(&temp_dir).unwrap();
    let temp_path = format!("{}/test.txt", temp_dir);
    std::fs::write(&temp_path, "hello service test").unwrap();

    let file = aster_drive::services::file_service::store_from_temp(
        &state,
        user.id,
        None,
        "service_test.txt",
        &temp_path,
        18,
        None,
        false,
    )
    .await
    .unwrap();

    // get_info
    let info = aster_drive::services::file_service::get_info(&state, file.id, user.id)
        .await
        .unwrap();
    assert_eq!(info.name, "service_test.txt");
    assert_eq!(info.user_id, user.id);

    // 别人的文件
    let user2 =
        aster_drive::services::auth_service::register(&state, "user2", "u2@example.com", "pass1234")
            .await
            .unwrap();
    let err = aster_drive::services::file_service::get_info(&state, file.id, user2.id).await;
    assert!(err.is_err());
}

#[actix_web::test]
async fn test_collect_folder_tree_respects_deleted_visibility() {
    use aster_drive::services::{auth_service, folder_service, webdav_service};

    let state = common::setup().await;
    let user = auth_service::register(
        &state,
        "treewalker",
        "treewalker@example.com",
        "password123",
    )
    .await
    .unwrap();

    let root = folder_service::create(&state, user.id, "root", None)
        .await
        .unwrap();
    let active_child = folder_service::create(&state, user.id, "active", Some(root.id))
        .await
        .unwrap();
    let deleted_child = folder_service::create(&state, user.id, "deleted", Some(root.id))
        .await
        .unwrap();
    let deleted_grandchild =
        folder_service::create(&state, user.id, "deleted-leaf", Some(deleted_child.id))
            .await
            .unwrap();

    let root_file = store_service_file(&state, user.id, Some(root.id), "root.txt", "root").await;
    let active_file = store_service_file(
        &state,
        user.id,
        Some(active_child.id),
        "active.txt",
        "active",
    )
    .await;
    let deleted_file = store_service_file(
        &state,
        user.id,
        Some(deleted_child.id),
        "deleted.txt",
        "deleted",
    )
    .await;
    let deleted_grandchild_file = store_service_file(
        &state,
        user.id,
        Some(deleted_grandchild.id),
        "deleted-leaf.txt",
        "deleted leaf",
    )
    .await;

    folder_service::delete(&state, deleted_child.id, user.id)
        .await
        .unwrap();

    let (visible_files, visible_folder_ids) =
        webdav_service::collect_folder_tree(&state.db, user.id, root.id, false)
            .await
            .unwrap();
    let visible_file_ids = visible_files
        .into_iter()
        .map(|file| file.id)
        .collect::<BTreeSet<_>>();
    let visible_folder_ids = visible_folder_ids.into_iter().collect::<BTreeSet<_>>();

    assert_eq!(
        visible_folder_ids,
        [root.id, active_child.id].into_iter().collect()
    );
    assert_eq!(
        visible_file_ids,
        [root_file, active_file].into_iter().collect()
    );

    let (all_files, all_folder_ids) =
        webdav_service::collect_folder_tree(&state.db, user.id, root.id, true)
            .await
            .unwrap();
    let all_file_ids = all_files
        .into_iter()
        .map(|file| file.id)
        .collect::<BTreeSet<_>>();
    let all_folder_ids = all_folder_ids.into_iter().collect::<BTreeSet<_>>();

    assert_eq!(
        all_folder_ids,
        [
            root.id,
            active_child.id,
            deleted_child.id,
            deleted_grandchild.id
        ]
        .into_iter()
        .collect()
    );
    assert_eq!(
        all_file_ids,
        [
            root_file,
            active_file,
            deleted_file,
            deleted_grandchild_file
        ]
        .into_iter()
        .collect()
    );
}

#[actix_web::test]
async fn test_collect_folder_tree_handles_empty_leaf_folder() {
    use aster_drive::services::{auth_service, folder_service, webdav_service};

    let state = common::setup().await;
    let user = auth_service::register(&state, "treeleaf", "treeleaf@example.com", "password123")
        .await
        .unwrap();

    let leaf = folder_service::create(&state, user.id, "leaf", None)
        .await
        .unwrap();

    let (visible_files, visible_folder_ids) =
        webdav_service::collect_folder_tree(&state.db, user.id, leaf.id, false)
            .await
            .unwrap();
    assert!(visible_files.is_empty());
    assert_eq!(visible_folder_ids, vec![leaf.id]);

    let (all_files, all_folder_ids) =
        webdav_service::collect_folder_tree(&state.db, user.id, leaf.id, true)
            .await
            .unwrap();
    assert!(all_files.is_empty());
    assert_eq!(all_folder_ids, vec![leaf.id]);
}

#[actix_web::test]
async fn test_list_trash_keeps_original_paths_for_files_and_folders() {
    use aster_drive::services::{auth_service, file_service, folder_service, trash_service};

    let state = common::setup().await;
    let user = auth_service::register(
        &state,
        "trashpaths",
        "trashpaths@example.com",
        "password123",
    )
    .await
    .unwrap();

    let projects = folder_service::create(&state, user.id, "Projects", None)
        .await
        .unwrap();
    let reports = folder_service::create(&state, user.id, "Reports", Some(projects.id))
        .await
        .unwrap();
    let archive = folder_service::create(&state, user.id, "Archive", Some(projects.id))
        .await
        .unwrap();

    let file_id =
        store_service_file(&state, user.id, Some(reports.id), "report.txt", "report").await;

    file_service::delete(&state, file_id, user.id)
        .await
        .unwrap();
    folder_service::delete(&state, archive.id, user.id)
        .await
        .unwrap();

    let trash = trash_service::list_trash(&state, user.id, 10, 0, 10, None)
        .await
        .unwrap();

    assert_eq!(trash.folders_total, 1);
    assert_eq!(trash.files_total, 1);
    assert_eq!(trash.folders.len(), 1);
    assert_eq!(trash.files.len(), 1);
    assert_eq!(trash.folders[0].id, archive.id);
    assert_eq!(trash.folders[0].original_path, "/Projects");
    assert_eq!(trash.files[0].id, file_id);
    assert_eq!(trash.files[0].original_path, "/Projects/Reports");
}

#[actix_web::test]
async fn test_list_trash_handles_root_and_shared_parent_paths() {
    use aster_drive::services::{auth_service, file_service, folder_service, trash_service};

    let state = common::setup().await;
    let user = auth_service::register(&state, "trashmix", "trashmix@example.com", "password123")
        .await
        .unwrap();

    let shared = folder_service::create(&state, user.id, "Shared", None)
        .await
        .unwrap();
    let docs = folder_service::create(&state, user.id, "Docs", Some(shared.id))
        .await
        .unwrap();
    let nested_folder_a = folder_service::create(&state, user.id, "Archive-A", Some(shared.id))
        .await
        .unwrap();
    let nested_folder_b = folder_service::create(&state, user.id, "Archive-B", Some(shared.id))
        .await
        .unwrap();
    let root_folder = folder_service::create(&state, user.id, "RootTrash", None)
        .await
        .unwrap();

    let nested_file_a =
        store_service_file(&state, user.id, Some(docs.id), "nested-a.txt", "nested a").await;
    let nested_file_b =
        store_service_file(&state, user.id, Some(docs.id), "nested-b.txt", "nested b").await;
    let root_file = store_service_file(&state, user.id, None, "root.txt", "root").await;

    for file_id in [nested_file_a, nested_file_b, root_file] {
        file_service::delete(&state, file_id, user.id)
            .await
            .unwrap();
    }
    for folder_id in [nested_folder_a.id, nested_folder_b.id, root_folder.id] {
        folder_service::delete(&state, folder_id, user.id)
            .await
            .unwrap();
    }

    let trash = trash_service::list_trash(&state, user.id, 10, 0, 10, None)
        .await
        .unwrap();

    assert_eq!(trash.folders_total, 3);
    assert_eq!(trash.files_total, 3);

    let nested_folder_paths = trash
        .folders
        .iter()
        .filter(|item| item.id == nested_folder_a.id || item.id == nested_folder_b.id)
        .map(|item| item.original_path.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(nested_folder_paths, BTreeSet::from(["/Shared"]));

    let root_folder_item = trash
        .folders
        .iter()
        .find(|item| item.id == root_folder.id)
        .unwrap();
    assert_eq!(root_folder_item.original_path, "/");

    let nested_file_paths = trash
        .files
        .iter()
        .filter(|item| item.id == nested_file_a || item.id == nested_file_b)
        .map(|item| item.original_path.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(nested_file_paths, BTreeSet::from(["/Shared/Docs"]));

    let root_file_item = trash
        .files
        .iter()
        .find(|item| item.id == root_file)
        .unwrap();
    assert_eq!(root_file_item.original_path, "/");
}

#[actix_web::test]
async fn test_list_trash_zero_limits_keep_totals_and_empty_items() {
    use aster_drive::services::{auth_service, file_service, folder_service, trash_service};

    let state = common::setup().await;
    let user = auth_service::register(&state, "trashzero", "trashzero@example.com", "password123")
        .await
        .unwrap();

    let root_folder = folder_service::create(&state, user.id, "ZeroFolder", None)
        .await
        .unwrap();
    let root_file = store_service_file(&state, user.id, None, "zero.txt", "zero").await;

    folder_service::delete(&state, root_folder.id, user.id)
        .await
        .unwrap();
    file_service::delete(&state, root_file, user.id)
        .await
        .unwrap();

    let trash = trash_service::list_trash(&state, user.id, 0, 0, 0, None)
        .await
        .unwrap();

    assert_eq!(trash.folders_total, 1);
    assert_eq!(trash.files_total, 1);
    assert!(trash.folders.is_empty());
    assert!(trash.files.is_empty());
    assert!(trash.next_file_cursor.is_none());
}

// ─── Lock Service ─────────────────────────────────────────────────

#[actix_web::test]
async fn test_lock_service_lock_unlock() {
    let state = common::setup().await;

    let user = aster_drive::services::auth_service::register(
        &state,
        "locker",
        "locker@example.com",
        "pass1234",
    )
    .await
    .unwrap();

    // 创建文件夹来锁
    let folder = aster_drive::services::folder_service::create(&state, user.id, "LockTest", None)
        .await
        .unwrap();
    assert!(!folder.is_locked);

    // 锁定
    let lock = aster_drive::services::lock_service::lock(
        &state,
        aster_drive::types::EntityType::Folder,
        folder.id,
        Some(user.id),
        None,
        None,
    )
    .await
    .unwrap();
    assert!(!lock.token.is_empty());

    // 锁定后 is_locked 应该为 true
    let f = aster_drive::db::repository::folder_repo::find_by_id(&state.db, folder.id)
        .await
        .unwrap();
    assert!(f.is_locked);

    // 重复锁定应失败
    let err = aster_drive::services::lock_service::lock(
        &state,
        aster_drive::types::EntityType::Folder,
        folder.id,
        Some(user.id),
        None,
        None,
    )
    .await;
    assert!(err.is_err());

    // 删除应失败（is_locked=true）
    let err = aster_drive::services::folder_service::delete(&state, folder.id, user.id).await;
    assert!(err.is_err());

    // 解锁
    aster_drive::services::lock_service::unlock(
        &state,
        aster_drive::types::EntityType::Folder,
        folder.id,
        user.id,
    )
    .await
    .unwrap();

    // is_locked 应该回到 false
    let f = aster_drive::db::repository::folder_repo::find_by_id(&state.db, folder.id)
        .await
        .unwrap();
    assert!(!f.is_locked);

    // 删除成功
    aster_drive::services::folder_service::delete(&state, folder.id, user.id)
        .await
        .unwrap();
}

#[actix_web::test]
async fn test_lock_service_force_unlock() {
    let state = common::setup().await;

    let user = aster_drive::services::auth_service::register(
        &state,
        "admin1",
        "admin1@example.com",
        "pass1234",
    )
    .await
    .unwrap();

    let folder = aster_drive::services::folder_service::create(&state, user.id, "ForceTest", None)
        .await
        .unwrap();

    let lock = aster_drive::services::lock_service::lock(
        &state,
        aster_drive::types::EntityType::Folder,
        folder.id,
        Some(user.id),
        None,
        None,
    )
    .await
    .unwrap();

    // 强制解锁（admin 操作）
    aster_drive::services::lock_service::force_unlock(&state, lock.id)
        .await
        .unwrap();

    let f = aster_drive::db::repository::folder_repo::find_by_id(&state.db, folder.id)
        .await
        .unwrap();
    assert!(!f.is_locked);
}

#[actix_web::test]
async fn test_lock_service_unlock_by_token_clears_file_lock_state() {
    use aster_drive::db::repository::{file_repo, lock_repo};
    use aster_drive::services::{auth_service, file_service, lock_service};

    let state = common::setup().await;
    let user = auth_service::register(&state, "tokunlock", "tokunlock@example.com", "pass1234")
        .await
        .unwrap();

    let temp_dir = format!("/tmp/asterdrive-lock-token-test-{}", uuid::Uuid::new_v4());
    std::fs::create_dir_all(&temp_dir).unwrap();
    let temp_path = format!("{temp_dir}/locked.txt");
    std::fs::write(&temp_path, "lock by token").unwrap();

    let file = file_service::store_from_temp(
        &state,
        user.id,
        None,
        "locked.txt",
        &temp_path,
        "lock by token".len() as i64,
        None,
        false,
    )
    .await
    .unwrap();

    let lock = lock_service::lock(
        &state,
        aster_drive::types::EntityType::File,
        file.id,
        Some(user.id),
        None,
        None,
    )
    .await
    .unwrap();

    let locked = file_repo::find_by_id(&state.db, file.id).await.unwrap();
    assert!(locked.is_locked);

    lock_service::unlock_by_token(&state, &lock.token)
        .await
        .unwrap();

    let unlocked = file_repo::find_by_id(&state.db, file.id).await.unwrap();
    assert!(!unlocked.is_locked);
    assert!(
        lock_repo::find_by_token(&state.db, &lock.token)
            .await
            .unwrap()
            .is_none()
    );
}

#[actix_web::test]
async fn test_lock_service_cleanup_expired_unlocks_only_expired_resources() {
    use aster_drive::db::repository::{folder_repo, lock_repo};
    use aster_drive::services::{auth_service, folder_service, lock_service};
    use chrono::Duration;

    let state = common::setup().await;
    let user = auth_service::register(&state, "lockcleanup", "lockcleanup@example.com", "pass1234")
        .await
        .unwrap();

    let expired_folder = folder_service::create(&state, user.id, "ExpiredLock", None)
        .await
        .unwrap();
    let active_folder = folder_service::create(&state, user.id, "ActiveLock", None)
        .await
        .unwrap();

    let expired_lock = lock_service::lock(
        &state,
        aster_drive::types::EntityType::Folder,
        expired_folder.id,
        Some(user.id),
        None,
        Some(Duration::seconds(-1)),
    )
    .await
    .unwrap();
    let active_lock = lock_service::lock(
        &state,
        aster_drive::types::EntityType::Folder,
        active_folder.id,
        Some(user.id),
        None,
        Some(Duration::minutes(10)),
    )
    .await
    .unwrap();

    let cleaned = lock_service::cleanup_expired(&state).await.unwrap();
    assert_eq!(cleaned, 1);

    let expired = folder_repo::find_by_id(&state.db, expired_folder.id)
        .await
        .unwrap();
    let active = folder_repo::find_by_id(&state.db, active_folder.id)
        .await
        .unwrap();
    assert!(!expired.is_locked);
    assert!(active.is_locked);
    assert!(
        lock_repo::find_by_token(&state.db, &expired_lock.token)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        lock_repo::find_by_token(&state.db, &active_lock.token)
            .await
            .unwrap()
            .is_some()
    );
}

// ─── Version Service ──────────────────────────────────────────────

#[actix_web::test]
async fn test_version_service_list_delete() {
    let state = common::setup().await;

    let user = aster_drive::services::auth_service::register(
        &state,
        "veruser",
        "ver@example.com",
        "pass1234",
    )
    .await
    .unwrap();

    // 上传 v1
    let temp_dir = format!("/tmp/asterdrive-ver-test-{}", uuid::Uuid::new_v4());
    std::fs::create_dir_all(&temp_dir).unwrap();
    let temp1 = format!("{}/v1.txt", temp_dir);
    std::fs::write(&temp1, "version 1").unwrap();

    let file = aster_drive::services::file_service::store_from_temp(
        &state,
        user.id,
        None,
        "versioned.txt",
        &temp1,
        9,
        None,
        false,
    )
    .await
    .unwrap();

    // 无版本
    let versions = aster_drive::services::version_service::list_versions(&state, file.id, user.id)
        .await
        .unwrap();
    assert_eq!(versions.len(), 0);

    // 覆盖 → v2（产生 v1 版本记录）
    let temp2 = format!("{}/v2.txt", temp_dir);
    std::fs::write(&temp2, "version 2 content").unwrap();

    let _ = aster_drive::services::file_service::store_from_temp(
        &state,
        user.id,
        None,
        "versioned.txt",
        &temp2,
        17,
        Some(file.id),
        false,
    )
    .await
    .unwrap();

    // 应有 1 个版本
    let versions = aster_drive::services::version_service::list_versions(&state, file.id, user.id)
        .await
        .unwrap();
    assert_eq!(versions.len(), 1);
    assert_eq!(versions[0].version, 1);

    // 删除版本
    aster_drive::services::version_service::delete_version(
        &state,
        file.id,
        versions[0].id,
        user.id,
    )
    .await
    .unwrap();

    let versions = aster_drive::services::version_service::list_versions(&state, file.id, user.id)
        .await
        .unwrap();
    assert_eq!(versions.len(), 0);
}

#[actix_web::test]
async fn test_delete_version_keeps_history_numbers_dense() {
    let state = common::setup().await;

    let user = aster_drive::services::auth_service::register(
        &state,
        "densever",
        "densever@example.com",
        "pass1234",
    )
    .await
    .unwrap();

    let temp1 = write_service_fixture("dense-v1.txt", "1111");
    let file = aster_drive::services::file_service::store_from_temp(
        &state,
        user.id,
        None,
        "dense-versioned.txt",
        &temp1,
        4,
        None,
        false,
    )
    .await
    .unwrap();

    let temp2 = write_service_fixture("dense-v2.txt", "2222");
    aster_drive::services::file_service::store_from_temp(
        &state,
        user.id,
        None,
        "dense-versioned.txt",
        &temp2,
        4,
        Some(file.id),
        false,
    )
    .await
    .unwrap();

    let temp3 = write_service_fixture("dense-v3.txt", "3333");
    aster_drive::services::file_service::store_from_temp(
        &state,
        user.id,
        None,
        "dense-versioned.txt",
        &temp3,
        4,
        Some(file.id),
        false,
    )
    .await
    .unwrap();

    let versions = aster_drive::services::version_service::list_versions(&state, file.id, user.id)
        .await
        .unwrap();
    assert_eq!(
        versions
            .iter()
            .map(|version| version.version)
            .collect::<Vec<_>>(),
        vec![2, 1]
    );

    aster_drive::services::version_service::delete_version(
        &state,
        file.id,
        versions[1].id,
        user.id,
    )
    .await
    .unwrap();

    let versions = aster_drive::services::version_service::list_versions(&state, file.id, user.id)
        .await
        .unwrap();
    assert_eq!(versions.len(), 1);
    assert_eq!(versions[0].version, 1);

    let temp4 = write_service_fixture("dense-v4.txt", "4444");
    aster_drive::services::file_service::store_from_temp(
        &state,
        user.id,
        None,
        "dense-versioned.txt",
        &temp4,
        4,
        Some(file.id),
        false,
    )
    .await
    .unwrap();

    let versions = aster_drive::services::version_service::list_versions(&state, file.id, user.id)
        .await
        .unwrap();
    assert_eq!(
        versions
            .iter()
            .map(|version| version.version)
            .collect::<Vec<_>>(),
        vec![2, 1]
    );
}

#[actix_web::test]
async fn test_version_storage_used_tracks_overwrite_delete_and_restore() {
    let state = common::setup().await;

    let user = aster_drive::services::auth_service::register(
        &state,
        "versionquota",
        "versionquota@example.com",
        "pass1234",
    )
    .await
    .unwrap();

    let temp1 = write_service_fixture("quota-v1.txt", "1111");
    let file = aster_drive::services::file_service::store_from_temp(
        &state,
        user.id,
        None,
        "quota-versioned.txt",
        &temp1,
        4,
        None,
        false,
    )
    .await
    .unwrap();
    assert_eq!(user_storage_used(&state, user.id).await, 4);

    let temp2 = write_service_fixture("quota-v2.txt", "222222");
    aster_drive::services::file_service::store_from_temp(
        &state,
        user.id,
        None,
        "quota-versioned.txt",
        &temp2,
        6,
        Some(file.id),
        false,
    )
    .await
    .unwrap();
    assert_eq!(user_storage_used(&state, user.id).await, 10);

    let temp3 = write_service_fixture("quota-v3.txt", "33333333");
    aster_drive::services::file_service::store_from_temp(
        &state,
        user.id,
        None,
        "quota-versioned.txt",
        &temp3,
        8,
        Some(file.id),
        false,
    )
    .await
    .unwrap();
    assert_eq!(user_storage_used(&state, user.id).await, 18);

    let versions = aster_drive::services::version_service::list_versions(&state, file.id, user.id)
        .await
        .unwrap();
    assert_eq!(
        versions
            .iter()
            .map(|version| version.size)
            .collect::<Vec<_>>(),
        vec![6, 4]
    );

    aster_drive::services::version_service::delete_version(
        &state,
        file.id,
        versions[1].id,
        user.id,
    )
    .await
    .unwrap();
    assert_eq!(user_storage_used(&state, user.id).await, 14);

    let versions = aster_drive::services::version_service::list_versions(&state, file.id, user.id)
        .await
        .unwrap();
    let restored = aster_drive::services::version_service::restore_version(
        &state,
        file.id,
        versions[0].id,
        user.id,
    )
    .await
    .unwrap();
    assert_eq!(restored.size, 6);
    assert_eq!(user_storage_used(&state, user.id).await, 6);

    let versions = aster_drive::services::version_service::list_versions(&state, file.id, user.id)
        .await
        .unwrap();
    assert!(versions.is_empty());
}

#[actix_web::test]
async fn test_version_cleanup_excess_reclaims_storage_used() {
    let state = common::setup().await;

    let user = aster_drive::services::auth_service::register(
        &state,
        "versionlimit",
        "versionlimit@example.com",
        "pass1234",
    )
    .await
    .unwrap();

    let mut max_versions =
        aster_drive::db::repository::config_repo::find_by_key(&state.db, "max_versions_per_file")
            .await
            .unwrap()
            .unwrap();
    max_versions.value = "1".to_string();
    state.runtime_config.apply(max_versions);

    let temp1 = write_service_fixture("limit-v1.txt", "1111");
    let file = aster_drive::services::file_service::store_from_temp(
        &state,
        user.id,
        None,
        "limit-versioned.txt",
        &temp1,
        4,
        None,
        false,
    )
    .await
    .unwrap();
    assert_eq!(user_storage_used(&state, user.id).await, 4);

    let temp2 = write_service_fixture("limit-v2.txt", "222222");
    aster_drive::services::file_service::store_from_temp(
        &state,
        user.id,
        None,
        "limit-versioned.txt",
        &temp2,
        6,
        Some(file.id),
        false,
    )
    .await
    .unwrap();
    assert_eq!(user_storage_used(&state, user.id).await, 10);

    let temp3 = write_service_fixture("limit-v3.txt", "33333333");
    aster_drive::services::file_service::store_from_temp(
        &state,
        user.id,
        None,
        "limit-versioned.txt",
        &temp3,
        8,
        Some(file.id),
        false,
    )
    .await
    .unwrap();

    let versions = aster_drive::services::version_service::list_versions(&state, file.id, user.id)
        .await
        .unwrap();
    assert_eq!(versions.len(), 1);
    assert_eq!(versions[0].version, 1);
    assert_eq!(versions[0].size, 6);
    assert_eq!(user_storage_used(&state, user.id).await, 14);
}

#[actix_web::test]
async fn test_version_restore_truncates_future_versions_without_deleting_target_blob() {
    let state = common::setup().await;

    let user = aster_drive::services::auth_service::register(
        &state,
        "restoreuser",
        "restore@example.com",
        "pass1234",
    )
    .await
    .unwrap();

    let temp_dir = format!("/tmp/asterdrive-restore-test-{}", uuid::Uuid::new_v4());
    std::fs::create_dir_all(&temp_dir).unwrap();

    let temp1 = format!("{}/v1.txt", temp_dir);
    std::fs::write(&temp1, "version 1").unwrap();
    let file = aster_drive::services::file_service::store_from_temp(
        &state,
        user.id,
        None,
        "restore.txt",
        &temp1,
        9,
        None,
        false,
    )
    .await
    .unwrap();

    let temp2 = format!("{}/v2.txt", temp_dir);
    std::fs::write(&temp2, "version 2").unwrap();
    aster_drive::services::file_service::store_from_temp(
        &state,
        user.id,
        None,
        "restore.txt",
        &temp2,
        9,
        Some(file.id),
        false,
    )
    .await
    .unwrap();

    let temp3 = format!("{}/v3.txt", temp_dir);
    std::fs::write(&temp3, "version 3").unwrap();
    aster_drive::services::file_service::store_from_temp(
        &state,
        user.id,
        None,
        "restore.txt",
        &temp3,
        9,
        Some(file.id),
        false,
    )
    .await
    .unwrap();

    let temp4 = format!("{}/v4.txt", temp_dir);
    std::fs::write(&temp4, "version 4").unwrap();
    let latest = aster_drive::services::file_service::store_from_temp(
        &state,
        user.id,
        None,
        "restore.txt",
        &temp4,
        9,
        Some(file.id),
        false,
    )
    .await
    .unwrap();

    let versions = aster_drive::services::version_service::list_versions(&state, file.id, user.id)
        .await
        .unwrap();
    assert_eq!(
        versions.iter().map(|v| v.version).collect::<Vec<_>>(),
        vec![3, 2, 1]
    );

    let v3 = versions.iter().find(|v| v.version == 3).unwrap().clone();
    let v2 = versions.iter().find(|v| v.version == 2).unwrap().clone();
    let v1 = versions.iter().find(|v| v.version == 1).unwrap().clone();
    let old_current_blob_id = latest.blob_id;

    let restored =
        aster_drive::services::version_service::restore_version(&state, file.id, v2.id, user.id)
            .await
            .unwrap();

    assert_eq!(restored.blob_id, v2.blob_id);

    let versions = aster_drive::services::version_service::list_versions(&state, file.id, user.id)
        .await
        .unwrap();
    assert_eq!(versions.len(), 1);
    assert_eq!(versions[0].version, 1);
    assert_eq!(versions[0].blob_id, v1.blob_id);

    assert!(
        aster_drive::db::repository::file_repo::find_blob_by_id(&state.db, v1.blob_id)
            .await
            .is_ok()
    );
    assert!(
        aster_drive::db::repository::file_repo::find_blob_by_id(&state.db, v2.blob_id)
            .await
            .is_ok()
    );
    assert!(
        aster_drive::db::repository::file_repo::find_blob_by_id(&state.db, v3.blob_id)
            .await
            .is_err()
    );
    assert!(
        aster_drive::db::repository::file_repo::find_blob_by_id(&state.db, old_current_blob_id)
            .await
            .is_err()
    );

    let temp5 = format!("{}/v5.txt", temp_dir);
    std::fs::write(&temp5, "version 5").unwrap();
    aster_drive::services::file_service::store_from_temp(
        &state,
        user.id,
        None,
        "restore.txt",
        &temp5,
        9,
        Some(file.id),
        false,
    )
    .await
    .unwrap();

    let versions = aster_drive::services::version_service::list_versions(&state, file.id, user.id)
        .await
        .unwrap();
    assert_eq!(
        versions.iter().map(|v| v.version).collect::<Vec<_>>(),
        vec![2, 1]
    );
}

// ─── Copy Naming ──────────────────────────────────────────────────

#[actix_web::test]
async fn test_copy_file_naming() {
    let state = common::setup().await;

    let user = aster_drive::services::auth_service::register(
        &state,
        "copier",
        "copier@example.com",
        "pass1234",
    )
    .await
    .unwrap();

    let temp_dir = format!("/tmp/asterdrive-copy-test-{}", uuid::Uuid::new_v4());
    std::fs::create_dir_all(&temp_dir).unwrap();
    let temp = format!("{}/f.txt", temp_dir);
    std::fs::write(&temp, "copy me").unwrap();

    let file = aster_drive::services::file_service::store_from_temp(
        &state, user.id, None, "doc.txt", &temp, 7, None, false,
    )
    .await
    .unwrap();

    // 复制 1 → "doc (1).txt"
    let copy1 = aster_drive::services::file_service::copy_file(&state, file.id, user.id, None)
        .await
        .unwrap();
    assert_eq!(copy1.name, "doc (1).txt");

    // 复制 2 → "doc (2).txt"
    let copy2 = aster_drive::services::file_service::copy_file(&state, file.id, user.id, None)
        .await
        .unwrap();
    assert_eq!(copy2.name, "doc (2).txt");
}

// ─── Folder Service ───────────────────────────────────────────────

#[actix_web::test]
async fn test_folder_service_cycle_detection() {
    let state = common::setup().await;

    let user =
        aster_drive::services::auth_service::register(&state, "cycl", "cyc@example.com", "pass1234")
            .await
            .unwrap();

    let a = aster_drive::services::folder_service::create(&state, user.id, "A", None)
        .await
        .unwrap();
    let b = aster_drive::services::folder_service::create(&state, user.id, "B", Some(a.id))
        .await
        .unwrap();

    // 把 A 移到 B 下面 → 循环，应失败
    let err = aster_drive::services::folder_service::update(
        &state,
        a.id,
        user.id,
        None,
        aster_drive::types::NullablePatch::Value(b.id),
        aster_drive::types::NullablePatch::Absent,
    )
    .await;
    assert!(err.is_err());

    // 正常移动应该 OK
    let c = aster_drive::services::folder_service::create(&state, user.id, "C", None)
        .await
        .unwrap();
    let result = aster_drive::services::folder_service::update(
        &state,
        b.id,
        user.id,
        None,
        aster_drive::types::NullablePatch::Value(c.id),
        aster_drive::types::NullablePatch::Absent,
    )
    .await;
    assert!(result.is_ok());
}

// ─── Property Service ─────────────────────────────────────────────

#[actix_web::test]
async fn test_property_service_dav_readonly() {
    let state = common::setup().await;

    let user = aster_drive::services::auth_service::register(
        &state,
        "prop",
        "prop@example.com",
        "pass1234",
    )
    .await
    .unwrap();

    let folder = aster_drive::services::folder_service::create(&state, user.id, "PropTest", None)
        .await
        .unwrap();

    // 普通命名空间 OK
    let prop = aster_drive::services::property_service::set(
        &state,
        aster_drive::types::EntityType::Folder,
        folder.id,
        user.id,
        "aster:",
        "color",
        Some("blue"),
    )
    .await
    .unwrap();
    assert_eq!(prop.name, "color");

    // DAV: 命名空间被拒绝
    let err = aster_drive::services::property_service::set(
        &state,
        aster_drive::types::EntityType::Folder,
        folder.id,
        user.id,
        "DAV:",
        "getcontenttype",
        Some("text/plain"),
    )
    .await;
    assert!(err.is_err());
}

// ─── Driver Registry Invalidation ────────────────────────────────

#[actix_web::test]
async fn test_driver_registry_invalidate_on_policy_update() {
    let state = common::setup().await;

    // 获取默认策略
    let policies = aster_drive::db::repository::policy_repo::find_all(&state.db)
        .await
        .unwrap();
    let policy = &policies[0];

    // 首次 get_driver → 缓存创建
    let driver1 = state.driver_registry.get_driver(policy).unwrap();

    // 再次获取 → 应返回同一个缓存实例（Arc 指针相同）
    let driver2 = state.driver_registry.get_driver(policy).unwrap();
    assert!(
        std::sync::Arc::ptr_eq(&driver1, &driver2),
        "cached driver should be the same Arc instance"
    );

    // 通过 service 更新策略（会触发 invalidate）
    aster_drive::services::policy_service::update(
        &state,
        policy.id,
        aster_drive::services::policy_service::UpdateStoragePolicyInput {
            name: Some("Updated Name".to_string()),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    // 更新后获取 → 应是新的实例（缓存已失效，重新创建）
    let updated_policy = aster_drive::db::repository::policy_repo::find_by_id(&state.db, policy.id)
        .await
        .unwrap();
    let driver3 = state.driver_registry.get_driver(&updated_policy).unwrap();
    assert!(
        !std::sync::Arc::ptr_eq(&driver1, &driver3),
        "driver should be recreated after policy update"
    );
}

#[actix_web::test]
async fn test_share_service_batch_delete_validates_ids_before_scope_work() {
    let state = common::setup().await;
    let oversized = vec![1_i64; aster_drive::services::batch_service::MAX_BATCH_ITEMS + 1];

    let err =
        match aster_drive::services::share_service::batch_delete_shares(&state, 999, &[]).await {
            Ok(_) => panic!("empty personal batch delete should fail validation"),
            Err(err) => err,
        };
    assert_eq!(err.code(), "E005");
    assert!(
        err.to_string()
            .contains("at least one share ID is required")
    );

    let err =
        match aster_drive::services::share_service::batch_delete_shares(&state, 999, &oversized)
            .await
        {
            Ok(_) => panic!("oversized personal batch delete should fail validation"),
            Err(err) => err,
        };
    assert_eq!(err.code(), "E005");
    assert!(err.to_string().contains("batch size cannot exceed"));

    let err =
        match aster_drive::services::share_service::batch_delete_team_shares(&state, 999, 999, &[])
            .await
        {
            Ok(_) => panic!("empty team batch delete should fail validation"),
            Err(err) => err,
        };
    assert_eq!(err.code(), "E005");
    assert!(
        err.to_string()
            .contains("at least one share ID is required")
    );

    let err = match aster_drive::services::share_service::batch_delete_team_shares(
        &state, 999, 999, &oversized,
    )
    .await
    {
        Ok(_) => panic!("oversized team batch delete should fail validation"),
        Err(err) => err,
    };
    assert_eq!(err.code(), "E005");
    assert!(err.to_string().contains("batch size cannot exceed"));
}

#[actix_web::test]
async fn test_share_download_failure_rolls_back_download_quota() {
    let state = common::setup().await;
    let user = aster_drive::services::auth_service::register(
        &state,
        "sharedownload",
        "sharedownload@example.com",
        "password123",
    )
    .await
    .unwrap();

    let file_id = store_service_file(&state, user.id, None, "download.txt", "download").await;
    let share = aster_drive::services::share_service::create_share(
        &state,
        user.id,
        Some(file_id),
        None,
        None,
        None,
        1,
    )
    .await
    .unwrap();

    let file = aster_drive::db::repository::file_repo::find_by_id(&state.db, file_id)
        .await
        .unwrap();
    let blob = aster_drive::db::repository::file_repo::find_blob_by_id(&state.db, file.blob_id)
        .await
        .unwrap();
    let policy = state
        .policy_snapshot
        .get_policy_or_err(blob.policy_id)
        .unwrap();
    let stored_path = std::path::Path::new(&policy.base_path).join(&blob.storage_path);
    std::fs::remove_file(&stored_path).unwrap();

    let err =
        aster_drive::services::share_service::download_shared_file(&state, &share.token, None)
            .await
            .unwrap_err();
    assert_ne!(err.code(), "E053");

    let reloaded = aster_drive::db::repository::share_repo::find_by_id(&state.db, share.id)
        .await
        .unwrap();
    assert_eq!(reloaded.download_count, 0);

    let err =
        aster_drive::services::share_service::download_shared_file(&state, &share.token, None)
            .await
            .unwrap_err();
    assert_ne!(err.code(), "E053");

    let reloaded = aster_drive::db::repository::share_repo::find_by_id(&state.db, share.id)
        .await
        .unwrap();
    assert_eq!(reloaded.download_count, 0);
}

#[actix_web::test]
async fn test_team_service_accepts_128_multibyte_characters_in_name() {
    let state = common::setup().await;
    let owner = aster_drive::services::auth_service::register(
        &state,
        "teamunicode",
        "teamunicode@example.com",
        "password123",
    )
    .await
    .unwrap();

    let valid_name = "你".repeat(128);
    let team = aster_drive::services::team_service::create_team(
        &state,
        owner.id,
        aster_drive::services::team_service::CreateTeamInput {
            name: valid_name.clone(),
            description: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(team.name, valid_name);

    let err = aster_drive::services::team_service::create_team(
        &state,
        owner.id,
        aster_drive::services::team_service::CreateTeamInput {
            name: "你".repeat(129),
            description: None,
        },
    )
    .await
    .unwrap_err();
    assert_eq!(err.code(), "E005");
    assert!(
        err.to_string()
            .contains("team name must be at most 128 characters")
    );
}

#[actix_web::test]
async fn test_team_service_clamps_negative_default_storage_quota() {
    let state = common::setup().await;
    let owner = aster_drive::services::auth_service::register(
        &state,
        "teamquota",
        "teamquota@example.com",
        "password123",
    )
    .await
    .unwrap();

    let mut updated =
        aster_drive::db::repository::config_repo::find_by_key(&state.db, "default_storage_quota")
            .await
            .unwrap()
            .unwrap();
    updated.value = "-1".to_string();
    state.runtime_config.apply(updated);

    let team = aster_drive::services::team_service::create_team(
        &state,
        owner.id,
        aster_drive::services::team_service::CreateTeamInput {
            name: "Quota Clamp".to_string(),
            description: None,
        },
    )
    .await
    .unwrap();

    assert_eq!(team.storage_quota, 0);
}

#[actix_web::test]
async fn test_team_service_rejects_create_without_default_policy_group() {
    use sea_orm::ConnectionTrait;

    let state = common::setup().await;
    let owner = aster_drive::services::auth_service::register(
        &state,
        "teamnodefault",
        "teamnodefault@example.com",
        "password123",
    )
    .await
    .unwrap();

    state
        .db
        .execute_unprepared("UPDATE storage_policy_groups SET is_default = 0;")
        .await
        .unwrap();
    state.policy_snapshot.reload(&state.db).await.unwrap();

    let err = aster_drive::services::team_service::create_team(
        &state,
        owner.id,
        aster_drive::services::team_service::CreateTeamInput {
            name: "No Default Policy Group".to_string(),
            description: None,
        },
    )
    .await
    .unwrap_err();

    assert_eq!(err.code(), "E005");
    assert!(
        err.message()
            .contains("no system default storage policy group configured")
    );
}

#[actix_web::test]
async fn test_team_service_degrades_missing_creator_rows() {
    use sea_orm::{ConnectionTrait, IntoActiveModel, Set};

    let state = common::setup().await;
    let owner = aster_drive::services::auth_service::register(
        &state,
        "missowner",
        "missowner@example.com",
        "password123",
    )
    .await
    .unwrap();
    let member = aster_drive::services::auth_service::register(
        &state,
        "missmember",
        "missmember@example.com",
        "password123",
    )
    .await
    .unwrap();

    let team = aster_drive::services::team_service::create_team(
        &state,
        owner.id,
        aster_drive::services::team_service::CreateTeamInput {
            name: "Missing Creator".to_string(),
            description: None,
        },
    )
    .await
    .unwrap();

    aster_drive::services::team_service::add_member(
        &state,
        team.id,
        owner.id,
        aster_drive::services::team_service::AddTeamMemberInput {
            user_id: Some(member.id),
            identifier: None,
            role: aster_drive::types::TeamMemberRole::Member,
        },
    )
    .await
    .unwrap();

    state
        .db
        .execute_unprepared("PRAGMA foreign_keys=OFF;")
        .await
        .unwrap();
    let mut broken_team = aster_drive::db::repository::team_repo::find_by_id(&state.db, team.id)
        .await
        .unwrap()
        .into_active_model();
    broken_team.created_by = Set(i64::MAX);
    aster_drive::db::repository::team_repo::update(&state.db, broken_team)
        .await
        .unwrap();
    state
        .db
        .execute_unprepared("PRAGMA foreign_keys=ON;")
        .await
        .unwrap();

    let loaded = aster_drive::services::team_service::get_team(&state, team.id, owner.id)
        .await
        .unwrap();
    assert_eq!(loaded.created_by_username, "<deleted_user>");

    let teams = aster_drive::services::team_service::list_teams(&state, member.id, false)
        .await
        .unwrap();
    assert_eq!(teams.len(), 1);
    assert_eq!(teams[0].created_by_username, "<deleted_user>");
}

#[actix_web::test]
async fn test_folder_repo_find_expired_deleted_includes_team_folders() {
    use chrono::{Duration, Utc};
    use sea_orm::Set;

    let state = common::setup().await;
    let owner = aster_drive::services::auth_service::register(
        &state,
        "trashteamowner",
        "trashteamowner@example.com",
        "password123",
    )
    .await
    .unwrap();
    let team = aster_drive::services::team_service::create_team(
        &state,
        owner.id,
        aster_drive::services::team_service::CreateTeamInput {
            name: "Trash Team".to_string(),
            description: None,
        },
    )
    .await
    .unwrap();

    let deleted_at = Utc::now() - Duration::days(10);
    let created = aster_drive::db::repository::folder_repo::create(
        &state.db,
        aster_drive::entities::folder::ActiveModel {
            name: Set("Team Trash".to_string()),
            parent_id: Set(None),
            team_id: Set(Some(team.id)),
            user_id: Set(owner.id),
            policy_id: Set(None),
            created_at: Set(deleted_at),
            updated_at: Set(deleted_at),
            deleted_at: Set(Some(deleted_at)),
            is_locked: Set(false),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let expired =
        aster_drive::db::repository::folder_repo::find_expired_deleted(&state.db, Utc::now())
            .await
            .unwrap();

    assert!(expired.iter().any(|folder| folder.id == created.id));
}

#[actix_web::test]
async fn test_folder_repo_find_all_by_user_includes_team_folders() {
    use chrono::Utc;
    use sea_orm::Set;

    let state = common::setup().await;
    let owner = aster_drive::services::auth_service::register(
        &state,
        "foldown",
        "foldown@example.com",
        "password123",
    )
    .await
    .unwrap();
    let member = aster_drive::services::auth_service::register(
        &state,
        "foldmem",
        "foldmem@example.com",
        "password123",
    )
    .await
    .unwrap();

    let team = aster_drive::services::team_service::create_team(
        &state,
        owner.id,
        aster_drive::services::team_service::CreateTeamInput {
            name: "Folder Scope".to_string(),
            description: None,
        },
    )
    .await
    .unwrap();

    aster_drive::services::team_service::add_member(
        &state,
        team.id,
        owner.id,
        aster_drive::services::team_service::AddTeamMemberInput {
            user_id: Some(member.id),
            identifier: None,
            role: aster_drive::types::TeamMemberRole::Member,
        },
    )
    .await
    .unwrap();

    let now = Utc::now();
    let personal = aster_drive::db::repository::folder_repo::create(
        &state.db,
        aster_drive::entities::folder::ActiveModel {
            name: Set("Personal".to_string()),
            parent_id: Set(None),
            team_id: Set(None),
            user_id: Set(member.id),
            policy_id: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
            deleted_at: Set(None),
            is_locked: Set(false),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let team_folder = aster_drive::db::repository::folder_repo::create(
        &state.db,
        aster_drive::entities::folder::ActiveModel {
            name: Set("Team".to_string()),
            parent_id: Set(None),
            team_id: Set(Some(team.id)),
            user_id: Set(member.id),
            policy_id: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
            deleted_at: Set(None),
            is_locked: Set(false),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let folders = aster_drive::db::repository::folder_repo::find_all_by_user(&state.db, member.id)
        .await
        .unwrap();
    let folder_ids: BTreeSet<i64> = folders.into_iter().map(|folder| folder.id).collect();

    assert!(folder_ids.contains(&personal.id));
    assert!(folder_ids.contains(&team_folder.id));
}

#[actix_web::test]
async fn test_folder_repo_top_level_deleted_pagination_is_stable_for_equal_timestamps() {
    use chrono::Utc;
    use sea_orm::Set;

    let state = common::setup().await;
    let user = aster_drive::services::auth_service::register(
        &state,
        "trashord",
        "trashord@example.com",
        "password123",
    )
    .await
    .unwrap();

    let deleted_at = Utc::now();
    let first = aster_drive::db::repository::folder_repo::create(
        &state.db,
        aster_drive::entities::folder::ActiveModel {
            name: Set("first".to_string()),
            parent_id: Set(None),
            team_id: Set(None),
            user_id: Set(user.id),
            policy_id: Set(None),
            created_at: Set(deleted_at),
            updated_at: Set(deleted_at),
            deleted_at: Set(Some(deleted_at)),
            is_locked: Set(false),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let second = aster_drive::db::repository::folder_repo::create(
        &state.db,
        aster_drive::entities::folder::ActiveModel {
            name: Set("second".to_string()),
            parent_id: Set(None),
            team_id: Set(None),
            user_id: Set(user.id),
            policy_id: Set(None),
            created_at: Set(deleted_at),
            updated_at: Set(deleted_at),
            deleted_at: Set(Some(deleted_at)),
            is_locked: Set(false),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let (page_one, total) =
        aster_drive::db::repository::folder_repo::find_top_level_deleted_paginated(
            &state.db, user.id, 1, 0,
        )
        .await
        .unwrap();
    let (page_two, _) = aster_drive::db::repository::folder_repo::find_top_level_deleted_paginated(
        &state.db, user.id, 1, 1,
    )
    .await
    .unwrap();

    assert_eq!(total, 2);
    assert_eq!(page_one.len(), 1);
    assert_eq!(page_two.len(), 1);
    assert_eq!(page_one[0].id, second.id);
    assert_eq!(page_two[0].id, first.id);
}

#[actix_web::test]
async fn test_team_service_list_teams_for_member() {
    let state = common::setup().await;
    let owner = aster_drive::services::auth_service::register(
        &state,
        "listteams-owner",
        "listteams-owner@example.com",
        "password123",
    )
    .await
    .unwrap();
    let member = aster_drive::services::auth_service::register(
        &state,
        "listteams-member",
        "listteams-member@example.com",
        "password123",
    )
    .await
    .unwrap();

    let team = aster_drive::services::team_service::create_team(
        &state,
        owner.id,
        aster_drive::services::team_service::CreateTeamInput {
            name: "List Teams".to_string(),
            description: None,
        },
    )
    .await
    .unwrap();

    aster_drive::services::team_service::add_member(
        &state,
        team.id,
        owner.id,
        aster_drive::services::team_service::AddTeamMemberInput {
            user_id: Some(member.id),
            identifier: None,
            role: aster_drive::types::TeamMemberRole::Member,
        },
    )
    .await
    .unwrap();

    let teams = aster_drive::services::team_service::list_teams(&state, member.id, false)
        .await
        .unwrap();
    assert_eq!(teams.len(), 1);
    assert_eq!(teams[0].id, team.id);
    assert_eq!(teams[0].my_role, aster_drive::types::TeamMemberRole::Member);
}

#[actix_web::test]
async fn test_team_service_list_user_team_ids_filters_archived_teams() {
    let state = common::setup().await;
    let owner = aster_drive::services::auth_service::register(
        &state,
        "teamids-owner",
        "listteamids-owner@example.com",
        "password123",
    )
    .await
    .unwrap();
    let member = aster_drive::services::auth_service::register(
        &state,
        "teamids-member",
        "listteamids-member@example.com",
        "password123",
    )
    .await
    .unwrap();

    let active_team = aster_drive::services::team_service::create_team(
        &state,
        owner.id,
        aster_drive::services::team_service::CreateTeamInput {
            name: "Active Team".to_string(),
            description: None,
        },
    )
    .await
    .unwrap();
    let archived_team = aster_drive::services::team_service::create_team(
        &state,
        owner.id,
        aster_drive::services::team_service::CreateTeamInput {
            name: "Archived Team".to_string(),
            description: None,
        },
    )
    .await
    .unwrap();

    for team_id in [active_team.id, archived_team.id] {
        aster_drive::services::team_service::add_member(
            &state,
            team_id,
            owner.id,
            aster_drive::services::team_service::AddTeamMemberInput {
                user_id: Some(member.id),
                identifier: None,
                role: aster_drive::types::TeamMemberRole::Member,
            },
        )
        .await
        .unwrap();
    }

    aster_drive::services::team_service::archive_team(&state, archived_team.id, owner.id)
        .await
        .unwrap();

    let active_team_ids =
        aster_drive::services::team_service::list_user_team_ids(&state, member.id, false)
            .await
            .unwrap();
    assert_eq!(active_team_ids.len(), 1);
    assert!(active_team_ids.contains(&active_team.id));
    assert!(!active_team_ids.contains(&archived_team.id));

    let archived_team_ids =
        aster_drive::services::team_service::list_user_team_ids(&state, member.id, true)
            .await
            .unwrap();
    assert_eq!(archived_team_ids.len(), 1);
    assert!(archived_team_ids.contains(&archived_team.id));
    assert!(!archived_team_ids.contains(&active_team.id));
}

#[actix_web::test]
async fn test_team_archive_cleanup_deletes_expired_team_data() {
    use chrono::{Duration, Utc};
    use sea_orm::{IntoActiveModel, Set};

    let state = common::setup().await;
    let owner = aster_drive::services::auth_service::register(
        &state,
        "cleanup-owner",
        "cleanup-owner@example.com",
        "password123",
    )
    .await
    .unwrap();
    let team = aster_drive::services::team_service::create_team(
        &state,
        owner.id,
        aster_drive::services::team_service::CreateTeamInput {
            name: "Cleanup Team".to_string(),
            description: None,
        },
    )
    .await
    .unwrap();

    let default_policy_id = aster_drive::db::repository::policy_repo::find_default(&state.db)
        .await
        .unwrap()
        .expect("default policy should exist")
        .id;
    let now = Utc::now();
    let folder = aster_drive::db::repository::folder_repo::create(
        &state.db,
        aster_drive::entities::folder::ActiveModel {
            name: Set("cleanup-folder".to_string()),
            parent_id: Set(None),
            team_id: Set(Some(team.id)),
            user_id: Set(owner.id),
            policy_id: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
            deleted_at: Set(None),
            is_locked: Set(false),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let blob = aster_drive::db::repository::file_repo::create_blob(
        &state.db,
        aster_drive::entities::file_blob::ActiveModel {
            hash: Set(format!("cleanup-blob-{}", uuid::Uuid::new_v4())),
            size: Set(12),
            policy_id: Set(default_policy_id),
            storage_path: Set(format!("files/{}", uuid::Uuid::new_v4())),
            ref_count: Set(1),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let file = aster_drive::db::repository::file_repo::create(
        &state.db,
        aster_drive::entities::file::ActiveModel {
            name: Set("cleanup.txt".to_string()),
            folder_id: Set(Some(folder.id)),
            team_id: Set(Some(team.id)),
            blob_id: Set(blob.id),
            size: Set(12),
            user_id: Set(owner.id),
            mime_type: Set("text/plain".to_string()),
            created_at: Set(now),
            updated_at: Set(now),
            deleted_at: Set(None),
            is_locked: Set(false),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    aster_drive::db::repository::property_repo::upsert(
        &state.db,
        aster_drive::types::EntityType::Folder,
        folder.id,
        "test",
        "label",
        Some("cleanup"),
    )
    .await
    .unwrap();

    aster_drive::services::lock_service::lock(
        &state,
        aster_drive::types::EntityType::Folder,
        folder.id,
        Some(owner.id),
        None,
        None,
    )
    .await
    .unwrap();

    aster_drive::db::repository::share_repo::create(
        &state.db,
        aster_drive::entities::share::ActiveModel {
            token: Set(uuid::Uuid::new_v4().simple().to_string()),
            user_id: Set(owner.id),
            team_id: Set(Some(team.id)),
            file_id: Set(Some(file.id)),
            folder_id: Set(None),
            password: Set(None),
            expires_at: Set(None),
            max_downloads: Set(0),
            download_count: Set(0),
            view_count: Set(0),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let upload_id = uuid::Uuid::new_v4().to_string();
    aster_drive::db::repository::upload_session_repo::create(
        &state.db,
        aster_drive::entities::upload_session::ActiveModel {
            id: Set(upload_id.clone()),
            user_id: Set(owner.id),
            team_id: Set(Some(team.id)),
            filename: Set("pending.bin".to_string()),
            total_size: Set(10),
            chunk_size: Set(10),
            total_chunks: Set(1),
            received_count: Set(0),
            folder_id: Set(Some(folder.id)),
            policy_id: Set(default_policy_id),
            status: Set(aster_drive::types::UploadSessionStatus::Uploading),
            s3_temp_key: Set(Some(format!("files/{upload_id}"))),
            s3_multipart_id: Set(None),
            file_id: Set(None),
            created_at: Set(now),
            expires_at: Set(now + Duration::hours(1)),
            updated_at: Set(now),
        },
    )
    .await
    .unwrap();

    aster_drive::services::team_service::archive_team(&state, team.id, owner.id)
        .await
        .unwrap();

    let mut archived_team = aster_drive::db::repository::team_repo::find_by_id(&state.db, team.id)
        .await
        .unwrap()
        .into_active_model();
    archived_team.archived_at = Set(Some(Utc::now() - Duration::days(8)));
    archived_team.updated_at = Set(Utc::now() - Duration::days(8));
    aster_drive::db::repository::team_repo::update(&state.db, archived_team)
        .await
        .unwrap();

    let deleted = aster_drive::services::team_service::cleanup_expired_archived_teams(&state)
        .await
        .unwrap();
    assert_eq!(deleted, 1);
    assert!(
        aster_drive::db::repository::team_repo::find_by_id(&state.db, team.id)
            .await
            .is_err()
    );
    assert!(
        aster_drive::db::repository::file_repo::find_by_id(&state.db, file.id)
            .await
            .is_err()
    );
    assert!(
        aster_drive::db::repository::folder_repo::find_by_id(&state.db, folder.id)
            .await
            .is_err()
    );
    assert!(
        aster_drive::db::repository::team_member_repo::find_by_team_and_user(
            &state.db, team.id, owner.id
        )
        .await
        .unwrap()
        .is_none()
    );
    assert!(
        aster_drive::db::repository::share_repo::find_by_team(&state.db, team.id)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        aster_drive::db::repository::upload_session_repo::find_by_team(&state.db, team.id)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        aster_drive::db::repository::lock_repo::find_by_path_prefix(
            &state.db,
            &format!("/teams/{}/", team.id),
        )
        .await
        .unwrap()
        .is_empty()
    );
    assert!(
        aster_drive::db::repository::property_repo::find_by_entity(
            &state.db,
            aster_drive::types::EntityType::Folder,
            folder.id,
        )
        .await
        .unwrap()
        .is_empty()
    );
}

#[actix_web::test]
async fn test_team_archive_cleanup_respects_configured_retention() {
    use chrono::{Duration, Utc};
    use sea_orm::{IntoActiveModel, Set};

    let state = common::setup().await;
    let owner = aster_drive::services::auth_service::register(
        &state,
        "clnretainown",
        "cleanup-retention-owner@example.com",
        "password123",
    )
    .await
    .unwrap();
    let team = aster_drive::services::team_service::create_team(
        &state,
        owner.id,
        aster_drive::services::team_service::CreateTeamInput {
            name: "Retention Team".to_string(),
            description: None,
        },
    )
    .await
    .unwrap();

    let mut config = aster_drive::db::repository::config_repo::find_by_key(
        &state.db,
        "team_archive_retention_days",
    )
    .await
    .unwrap()
    .unwrap();
    config.value = "30".to_string();
    state.runtime_config.apply(config);

    aster_drive::services::team_service::archive_team(&state, team.id, owner.id)
        .await
        .unwrap();

    let mut archived_team = aster_drive::db::repository::team_repo::find_by_id(&state.db, team.id)
        .await
        .unwrap()
        .into_active_model();
    archived_team.archived_at = Set(Some(Utc::now() - Duration::days(8)));
    archived_team.updated_at = Set(Utc::now() - Duration::days(8));
    aster_drive::db::repository::team_repo::update(&state.db, archived_team)
        .await
        .unwrap();

    let deleted = aster_drive::services::team_service::cleanup_expired_archived_teams(&state)
        .await
        .unwrap();
    assert_eq!(deleted, 0);

    let archived = aster_drive::db::repository::team_repo::find_archived_by_id(&state.db, team.id)
        .await
        .unwrap();
    assert_eq!(archived.id, team.id);
}
