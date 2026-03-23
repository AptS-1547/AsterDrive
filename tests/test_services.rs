#[macro_use]
mod common;

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

    // 登录 → (access_token, refresh_token)
    let (access, refresh) =
        aster_drive::services::auth_service::login(&state, "alice", "password123")
            .await
            .unwrap();
    assert!(!access.is_empty());
    assert!(!refresh.is_empty());

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
async fn test_auth_service_verify_token() {
    let state = common::setup().await;

    aster_drive::services::auth_service::register(&state, "bobb", "bob@example.com", "pass123")
        .await
        .unwrap();

    let (access, _) = aster_drive::services::auth_service::login(&state, "bobb", "pass123")
        .await
        .unwrap();

    // 验证 access token
    let claims =
        aster_drive::services::auth_service::verify_token(&access, &state.config.auth.jwt_secret)
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
    assert_eq!(versions.iter().map(|v| v.version).collect::<Vec<_>>(), vec![3, 2, 1]);

    let v3 = versions.iter().find(|v| v.version == 3).unwrap().clone();
    let v2 = versions.iter().find(|v| v.version == 2).unwrap().clone();
    let v1 = versions.iter().find(|v| v.version == 1).unwrap().clone();
    let old_current_blob_id = latest.blob_id;

    let restored = aster_drive::services::version_service::restore_version(
        &state,
        file.id,
        v2.id,
        user.id,
    )
    .await
    .unwrap();

    assert_eq!(restored.blob_id, v2.blob_id);

    let versions = aster_drive::services::version_service::list_versions(&state, file.id, user.id)
        .await
        .unwrap();
    assert_eq!(versions.len(), 1);
    assert_eq!(versions[0].version, 1);
    assert_eq!(versions[0].blob_id, v1.blob_id);

    assert!(aster_drive::db::repository::file_repo::find_blob_by_id(&state.db, v1.blob_id)
        .await
        .is_ok());
    assert!(aster_drive::db::repository::file_repo::find_blob_by_id(&state.db, v2.blob_id)
        .await
        .is_ok());
    assert!(aster_drive::db::repository::file_repo::find_blob_by_id(&state.db, v3.blob_id)
        .await
        .is_err());
    assert!(aster_drive::db::repository::file_repo::find_blob_by_id(&state.db, old_current_blob_id)
        .await
        .is_err());

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
    assert_eq!(versions.iter().map(|v| v.version).collect::<Vec<_>>(), vec![2, 1]);
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
        Some(b.id),
        None,
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
        Some(c.id),
        None,
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
        Some("Updated Name".to_string()),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
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
