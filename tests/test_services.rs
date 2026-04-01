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

    aster_drive::services::auth_service::register(&state, "bobb", "bob@example.com", "pass123")
        .await
        .unwrap();

    let login_result = aster_drive::services::auth_service::login(&state, "bobb", "pass123")
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
        aster_drive::services::auth_service::register(&state, "user1", "u1@example.com", "pass123")
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
        aster_drive::services::auth_service::register(&state, "user2", "u2@example.com", "pass123")
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
        "pass123",
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
        "pass123",
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
    let user = auth_service::register(&state, "tokunlock", "tokunlock@example.com", "pass123")
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
    let user = auth_service::register(&state, "lockcleanup", "lockcleanup@example.com", "pass123")
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
        "pass123",
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
async fn test_version_restore_truncates_future_versions_without_deleting_target_blob() {
    let state = common::setup().await;

    let user = aster_drive::services::auth_service::register(
        &state,
        "restoreuser",
        "restore@example.com",
        "pass123",
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
        "pass123",
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
        aster_drive::services::auth_service::register(&state, "cycl", "cyc@example.com", "pass123")
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
        "pass123",
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
