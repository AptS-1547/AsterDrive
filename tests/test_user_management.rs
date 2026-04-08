#[macro_use]
mod common;

use actix_web::test;
use serde_json::Value;

fn avatar_upload_payload() -> (String, Vec<u8>) {
    let boundary = "----AsterAvatarBoundary".to_string();
    let image = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        8,
        8,
        image::Rgba([255, 120, 0, 255]),
    ));
    let mut png = std::io::Cursor::new(Vec::new());
    image.write_to(&mut png, image::ImageFormat::Png).unwrap();

    let mut body = Vec::new();
    body.extend_from_slice(
        format!(
            "--{boundary}\r\n\
             Content-Disposition: form-data; name=\"file\"; filename=\"avatar.png\"\r\n\
             Content-Type: image/png\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(&png.into_inner());
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    (boundary, body)
}

// ── 策略删除保护：有 blob 引用则拒绝 ───────────────────────

#[actix_web::test]
async fn test_policy_delete_with_blobs_rejected() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    // 上传文件（会在默认策略创建 blob）
    let _file_id = upload_test_file!(app, token);

    // 获取策略 ID
    let req = test::TestRequest::get()
        .uri("/api/v1/admin/policies")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    let policy_id = body["data"]["items"][0]["id"].as_i64().unwrap();

    // 尝试删除策略 → 应被拒绝（有 blob 引用）
    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/admin/policies/{policy_id}"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        400,
        "should reject policy delete with blobs, got {}",
        resp.status()
    );
}

// ── 用户强制删除：级联清理所有数据 ─────────────────────────

#[actix_web::test]
async fn test_force_delete_user() {
    let state = common::setup().await;
    let avatar_base_path = state
        .runtime_config
        .get(aster_drive::config::avatar::AVATAR_DIR_KEY)
        .expect("avatar_dir should exist");
    let app = create_test_app!(state);

    // 注册第一个用户（admin，id=1）
    let (admin_token, _) = register_and_login!(app);

    // 注册第二个用户
    let req = test::TestRequest::post()
        .uri("/api/v1/auth/register")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "username": "victim",
            "email": "victim@example.com",
            "password": "password123"
        }))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let victim_id = body["data"]["id"].as_i64().unwrap();

    // 登录第二个用户
    let req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "identifier": "victim",
            "password": "password123"
        }))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    let victim_token =
        common::extract_cookie(&resp, "aster_access").expect("access cookie missing");

    // 用第二个用户上传文件
    let _file_id = upload_test_file!(app, victim_token);

    // 用第二个用户上传头像
    let (boundary, payload) = avatar_upload_payload();
    let req = test::TestRequest::post()
        .uri("/api/v1/auth/profile/avatar/upload")
        .insert_header(("Cookie", format!("aster_access={victim_token}")))
        .insert_header((
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        ))
        .set_payload(payload)
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let avatar_user_dir =
        std::path::PathBuf::from(&avatar_base_path).join(format!("user/{victim_id}"));
    let avatar_v1_dir = avatar_user_dir.join("v1");
    let avatar_512 = avatar_v1_dir.join("512.webp");
    let avatar_1024 = avatar_v1_dir.join("1024.webp");
    assert!(
        avatar_512.exists(),
        "avatar 512 should exist before force delete"
    );
    assert!(
        avatar_1024.exists(),
        "avatar 1024 should exist before force delete"
    );

    // 确认第二个用户可以在 admin 列表中看到
    let req = test::TestRequest::get()
        .uri("/api/v1/admin/users")
        .insert_header(("Cookie", format!("aster_access={admin_token}")))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    let users = body["data"]["items"].as_array().unwrap();
    assert!(users.iter().any(|u| u["id"].as_i64() == Some(victim_id)));

    // admin 强制删除第二个用户
    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/admin/users/{victim_id}"))
        .insert_header(("Cookie", format!("aster_access={admin_token}")))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        200,
        "force delete should succeed, got {}",
        resp.status()
    );

    // 确认用户不存在了
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/admin/users/{victim_id}"))
        .insert_header(("Cookie", format!("aster_access={admin_token}")))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 404);
    assert!(
        !avatar_512.exists(),
        "avatar 512 should be deleted during force delete"
    );
    assert!(
        !avatar_1024.exists(),
        "avatar 1024 should be deleted during force delete"
    );
    assert!(
        !avatar_v1_dir.exists(),
        "avatar version dir should be deleted during force delete"
    );
    assert!(
        !avatar_user_dir.exists(),
        "avatar user dir should be deleted during force delete"
    );
}

#[actix_web::test]
async fn test_force_delete_user_with_gravatar_profile() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (admin_token, _) = register_and_login!(app);

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/register")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "username": "gravatar-victim",
            "email": "gravatar-victim@example.com",
            "password": "password123"
        }))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let victim_id = body["data"]["id"].as_i64().unwrap();

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "identifier": "gravatar-victim",
            "password": "password123"
        }))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    let victim_token =
        common::extract_cookie(&resp, "aster_access").expect("access cookie missing");

    let req = test::TestRequest::put()
        .uri("/api/v1/auth/profile/avatar/source")
        .insert_header(("Cookie", format!("aster_access={victim_token}")))
        .set_json(serde_json::json!({
            "source": "gravatar"
        }))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/admin/users/{victim_id}"))
        .insert_header(("Cookie", format!("aster_access={admin_token}")))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

#[actix_web::test]
async fn test_force_delete_user_tolerates_missing_avatar_object() {
    let state = common::setup().await;
    let avatar_base_path = state
        .runtime_config
        .get(aster_drive::config::avatar::AVATAR_DIR_KEY)
        .expect("avatar_dir should exist");
    let app = create_test_app!(state);
    let (admin_token, _) = register_and_login!(app);

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/register")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "username": "missavatar",
            "email": "missavatar@example.com",
            "password": "password123"
        }))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let victim_id = body["data"]["id"].as_i64().unwrap();

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "identifier": "missavatar",
            "password": "password123"
        }))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    let victim_token =
        common::extract_cookie(&resp, "aster_access").expect("access cookie missing");

    let (boundary, payload) = avatar_upload_payload();
    let req = test::TestRequest::post()
        .uri("/api/v1/auth/profile/avatar/upload")
        .insert_header(("Cookie", format!("aster_access={victim_token}")))
        .insert_header((
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        ))
        .set_payload(payload)
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let avatar_user_dir =
        std::path::PathBuf::from(&avatar_base_path).join(format!("user/{victim_id}"));
    let avatar_v1_dir = avatar_user_dir.join("v1");
    let avatar_512 = avatar_v1_dir.join("512.webp");
    let avatar_1024 = avatar_v1_dir.join("1024.webp");
    assert!(avatar_512.exists());
    assert!(avatar_1024.exists());

    std::fs::remove_file(&avatar_512).unwrap();
    assert!(!avatar_512.exists());
    assert!(avatar_1024.exists());

    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/admin/users/{victim_id}"))
        .insert_header(("Cookie", format!("aster_access={admin_token}")))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    assert!(!avatar_1024.exists());
    assert!(!avatar_v1_dir.exists());
    assert!(!avatar_user_dir.exists());
}

// ── 不能删除初始管理员 id=1 ────────────────────────────────

#[actix_web::test]
async fn test_admin_create_user_uses_default_quota_and_policy() {
    use aster_drive::db::repository::policy_group_repo;

    let state = common::setup().await;
    let expected_default_id = policy_group_repo::find_default_group(&state.db)
        .await
        .unwrap()
        .expect("default policy group should exist")
        .id;
    let app = create_test_app!(state);
    let (admin_token, _) = register_and_login!(app);

    let req = test::TestRequest::put()
        .uri("/api/v1/admin/config/default_storage_quota")
        .insert_header(("Cookie", format!("aster_access={admin_token}")))
        .set_json(serde_json::json!({ "value": "1048576" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::post()
        .uri("/api/v1/admin/users")
        .insert_header(("Cookie", format!("aster_access={admin_token}")))
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "username": "quotauser",
            "email": "quotauser@example.com",
            "password": "password123"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let user_id = body["data"]["id"].as_i64().unwrap();
    assert_eq!(body["data"]["storage_quota"], 1_048_576);
    assert!(user_id > 0);
    assert_eq!(
        body["data"]["policy_group_id"].as_i64().unwrap(),
        expected_default_id
    );
}

#[actix_web::test]
async fn test_cannot_delete_initial_admin() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (admin_token, _) = register_and_login!(app);

    // 尝试删除 id=1
    let req = test::TestRequest::delete()
        .uri("/api/v1/admin/users/1")
        .insert_header(("Cookie", format!("aster_access={admin_token}")))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        400,
        "should reject deleting initial admin, got {}",
        resp.status()
    );
}

// ── 不能删除 admin 角色用户 ────────────────────────────────

#[actix_web::test]
async fn test_cannot_delete_admin_role() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (admin_token, _) = register_and_login!(app);

    // 注册第二个用户
    let req = test::TestRequest::post()
        .uri("/api/v1/auth/register")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "username": "admin2",
            "email": "admin2@example.com",
            "password": "password123"
        }))
        .to_request();
    let _: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;

    // 获取 admin2 的 ID
    let req = test::TestRequest::get()
        .uri("/api/v1/admin/users")
        .insert_header(("Cookie", format!("aster_access={admin_token}")))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    let users = body["data"]["items"].as_array().unwrap();
    let admin2_id = users.iter().find(|u| u["username"] == "admin2").unwrap()["id"]
        .as_i64()
        .unwrap();

    // 提升为 admin
    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/admin/users/{admin2_id}"))
        .insert_header(("Cookie", format!("aster_access={admin_token}")))
        .set_json(serde_json::json!({"role": "admin"}))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // 尝试删除 admin2 → 应被拒绝
    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/admin/users/{admin2_id}"))
        .insert_header(("Cookie", format!("aster_access={admin_token}")))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        400,
        "should reject deleting admin role user, got {}",
        resp.status()
    );
}
