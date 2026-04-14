#[macro_use]
mod common;

use std::io::Cursor;
use std::time::Duration;

fn write_temp_fixture(name: &str, contents: &str) -> String {
    let dir = format!("/tmp/asterdrive-webdav-lock-test-{}", uuid::Uuid::new_v4());
    std::fs::create_dir_all(&dir).unwrap();
    let path = format!("{dir}/{name}");
    std::fs::write(&path, contents).unwrap();
    path
}

#[actix_web::test]
async fn test_db_lock_system_deep_lock_supports_check_refresh_discover_and_delete() {
    use aster_drive::db::repository::{folder_repo, lock_repo};
    use aster_drive::services::{auth_service, file_service, folder_service};
    use aster_drive::webdav::db_lock_system::DbLockSystem;
    use dav_server::davpath::DavPath;
    use dav_server::ls::DavLockSystem;
    use xmltree::Element;

    let state = common::setup().await;
    let user = auth_service::register(&state, "davlocks", "davlocks@example.com", "pass1234")
        .await
        .unwrap();

    let projects = folder_service::create(&state, user.id, "projects", None)
        .await
        .unwrap();
    let docs = folder_service::create(&state, user.id, "docs", Some(projects.id))
        .await
        .unwrap();
    let temp_path = write_temp_fixture("note.txt", "deep lock content");
    file_service::store_from_temp(
        &state,
        user.id,
        Some(docs.id),
        "note.txt",
        &temp_path,
        "deep lock content".len() as i64,
        None,
        false,
    )
    .await
    .unwrap();

    let lock_system = DbLockSystem::new(state.db.clone(), user.id, None);
    let folder_path = DavPath::new("/projects/").unwrap();
    let child_path = DavPath::new("/projects/docs/note.txt").unwrap();
    let owner = Element::parse(Cursor::new(
        br#"<D:owner xmlns:D="DAV:"><D:href>tester</D:href></D:owner>"#,
    ))
    .unwrap();

    let lock = lock_system
        .lock(
            &folder_path,
            Some("tester"),
            Some(&owner),
            Some(Duration::from_secs(120)),
            false,
            true,
        )
        .await
        .unwrap();
    assert!(lock.deep);
    assert_eq!(lock.principal.as_deref(), Some("tester"));
    assert!(!lock.token.is_empty());

    let locked_folder = folder_repo::find_by_id(&state.db, projects.id)
        .await
        .unwrap();
    assert!(locked_folder.is_locked);

    let conflict = lock_system
        .check(&child_path, None, false, false, &[])
        .await
        .unwrap_err();
    assert_eq!(conflict.token, lock.token);

    lock_system
        .check(
            &child_path,
            None,
            false,
            false,
            std::slice::from_ref(&lock.token),
        )
        .await
        .unwrap();

    let discovered = lock_system.discover(&child_path).await;
    assert_eq!(discovered.len(), 1);
    assert_eq!(discovered[0].token, lock.token);
    assert_eq!(discovered[0].principal, None);
    assert!(discovered[0].owner.is_some());

    let refreshed = lock_system
        .refresh(&folder_path, &lock.token, Some(Duration::from_secs(30)))
        .await
        .unwrap();
    assert_eq!(refreshed.token, lock.token);
    assert_eq!(refreshed.principal, None);
    assert!(refreshed.owner.is_some());
    assert_eq!(refreshed.timeout, Some(Duration::from_secs(30)));

    let persisted = lock_repo::find_by_token(&state.db, &lock.token)
        .await
        .unwrap()
        .expect("refreshed lock should still exist");
    assert!(persisted.timeout_at.is_some());

    lock_system.delete(&folder_path).await.unwrap();
    assert!(
        lock_repo::find_by_token(&state.db, &lock.token)
            .await
            .unwrap()
            .is_none()
    );
    let unlocked_folder = folder_repo::find_by_id(&state.db, projects.id)
        .await
        .unwrap();
    assert!(!unlocked_folder.is_locked);
}

#[actix_web::test]
async fn test_db_lock_system_replaces_expired_locks_and_rejects_active_conflicts() {
    use aster_drive::db::repository::{file_repo, lock_repo};
    use aster_drive::services::{auth_service, file_service, lock_service};
    use aster_drive::types::EntityType;
    use aster_drive::webdav::db_lock_system::DbLockSystem;
    use chrono::Duration as ChronoDuration;
    use dav_server::davpath::DavPath;
    use dav_server::ls::DavLockSystem;

    let state = common::setup().await;
    let user = auth_service::register(&state, "davexpired", "davexpired@example.com", "pass1234")
        .await
        .unwrap();

    let temp_path = write_temp_fixture("expired.txt", "expired lock content");
    let file = file_service::store_from_temp(
        &state,
        user.id,
        None,
        "expired.txt",
        &temp_path,
        "expired lock content".len() as i64,
        None,
        false,
    )
    .await
    .unwrap();

    let expired_lock = lock_service::lock(
        &state,
        EntityType::File,
        file.id,
        Some(user.id),
        Some("expired".to_string()),
        Some(ChronoDuration::seconds(-1)),
    )
    .await
    .unwrap();

    let lock_system = DbLockSystem::new(state.db.clone(), user.id, None);
    let file_path = DavPath::new("/expired.txt").unwrap();

    let replacement = lock_system
        .lock(
            &file_path,
            Some("tester"),
            None,
            Some(Duration::from_secs(60)),
            false,
            false,
        )
        .await
        .unwrap();
    assert_ne!(replacement.token, expired_lock.token);
    assert!(
        lock_repo::find_by_token(&state.db, &expired_lock.token)
            .await
            .unwrap()
            .is_none()
    );

    let locked_file = file_repo::find_by_id(&state.db, file.id).await.unwrap();
    assert!(locked_file.is_locked);

    let conflict = lock_system
        .lock(
            &file_path,
            Some("tester"),
            None,
            Some(Duration::from_secs(60)),
            false,
            false,
        )
        .await
        .unwrap_err();
    assert_eq!(conflict.token, replacement.token);

    assert!(
        lock_system
            .unlock(&file_path, "missing-token")
            .await
            .is_err()
    );

    lock_system
        .unlock(&file_path, &replacement.token)
        .await
        .unwrap();
    assert!(
        lock_repo::find_by_token(&state.db, &replacement.token)
            .await
            .unwrap()
            .is_none()
    );
    let unlocked_file = file_repo::find_by_id(&state.db, file.id).await.unwrap();
    assert!(!unlocked_file.is_locked);
}
