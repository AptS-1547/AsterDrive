#[macro_use]
mod common;

use actix_web::test;
use serde_json::Value;
use std::io::Cursor;

fn avatar_upload_payload() -> (String, Vec<u8>) {
    let boundary = "----AsterAvatarBoundary".to_string();
    let image = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        8,
        8,
        image::Rgba([0, 160, 255, 255]),
    ));
    let mut png = Cursor::new(Vec::new());
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

#[actix_web::test]
async fn test_admin_scope_requires_authentication() {
    let state = common::setup().await;
    let app = create_test_app!(state);

    let req = test::TestRequest::get()
        .uri("/api/v1/admin/config/schema")
        .to_request();
    let err = test::try_call_service(&app, req).await.unwrap_err();
    let resp = err.error_response();
    assert_eq!(resp.status(), 401);
}

#[actix_web::test]
async fn test_admin_scope_rejects_non_admin_users() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (admin_token, _) = register_and_login!(app);
    admin_create_user!(
        app,
        admin_token,
        "plainadminscope",
        "plainadminscope@example.com",
        "password123"
    );
    let (token, _) = login_user!(app, "plainadminscope", "password123");

    let req = test::TestRequest::get()
        .uri("/api/v1/admin/config/schema")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let err = test::try_call_service(&app, req).await.unwrap_err();
    let resp = err.error_response();
    assert_eq!(resp.status(), 403);
}

#[actix_web::test]
async fn test_admin_scope_allows_admin_users() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let req = test::TestRequest::get()
        .uri("/api/v1/admin/config/schema")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert!(body["data"].is_array());
    let keys = body["data"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|item| item.get("key").and_then(Value::as_str))
        .collect::<Vec<_>>();
    assert!(keys.contains(&"auth_cookie_secure"));
    assert!(keys.contains(&"auth_allow_user_registration"));
    assert!(keys.contains(&"auth_access_token_ttl_secs"));
    assert!(keys.contains(&"auth_refresh_token_ttl_secs"));
    assert!(keys.contains(&"branding_title"));
    assert!(keys.contains(&"branding_description"));
    assert!(keys.contains(&"branding_favicon_url"));

    let auth_ttl = body["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["key"] == "auth_access_token_ttl_secs")
        .unwrap();
    assert_eq!(
        auth_ttl["label_i18n_key"],
        "settings_item_auth_access_token_ttl_secs_label"
    );
    assert_eq!(
        auth_ttl["description_i18n_key"],
        "settings_item_auth_access_token_ttl_secs_desc"
    );

    let register_toggle = body["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["key"] == "auth_allow_user_registration")
        .unwrap();
    assert_eq!(
        register_toggle["label_i18n_key"],
        "settings_item_auth_allow_user_registration_label"
    );
    assert_eq!(
        register_toggle["description_i18n_key"],
        "settings_item_auth_allow_user_registration_desc"
    );
    assert_eq!(register_toggle["category"], "user");

    let branding_title = body["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["key"] == "branding_title")
        .unwrap();
    assert_eq!(
        branding_title["label_i18n_key"],
        "settings_item_branding_title_label"
    );
}

#[actix_web::test]
async fn test_admin_locks() {
    let state = common::setup().await;
    let app = create_test_app!(state);

    // 第一个用户自动成为 admin
    let (token, _) = register_and_login!(app);

    // 列出锁（应为空）
    let req = test::TestRequest::get()
        .uri("/api/v1/admin/locks")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["items"].as_array().unwrap().len(), 0);
    assert_eq!(body["data"]["total"], 0);

    // 清理过期锁
    let req = test::TestRequest::delete()
        .uri("/api/v1/admin/locks/expired")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["removed"], 0);
}

#[actix_web::test]
async fn test_admin_users() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    // 再注册两个普通用户
    for (username, email) in [
        ("user2", "user2@example.com"),
        ("user3", "user3@example.com"),
    ] {
        let req = test::TestRequest::post()
            .uri("/api/v1/auth/register")
            .peer_addr("127.0.0.1:12345".parse().unwrap())
            .set_json(serde_json::json!({
                "username": username,
                "email": email,
                "password": "password123"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 201);
    }

    // 分页列出用户
    let req = test::TestRequest::get()
        .uri("/api/v1/admin/users?limit=2&offset=1")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    let data = &body["data"];
    let users = data["items"].as_array().unwrap();
    assert_eq!(data["limit"], 2);
    assert_eq!(data["offset"], 1);
    assert_eq!(data["total"], 3);
    assert_eq!(users.len(), 2);
    assert_eq!(users[0]["username"], "user2");
    assert_eq!(users[1]["username"], "user3");
    assert_eq!(users[0]["profile"]["avatar"]["source"], "none");
}

#[actix_web::test]
async fn test_admin_team_crud() {
    let state = common::setup().await;
    let default_group_id = state
        .policy_snapshot
        .system_default_policy_group()
        .expect("default policy group should exist")
        .id;
    let default_policy_id = aster_drive::db::repository::policy_repo::find_default(&state.db)
        .await
        .unwrap()
        .expect("default policy should exist")
        .id;
    let alternate_group_id = aster_drive::services::policy_service::create_group(
        &state,
        aster_drive::services::policy_service::CreateStoragePolicyGroupInput {
            name: "Operations Archive".to_string(),
            description: Some("Secondary team routing".to_string()),
            is_enabled: true,
            is_default: false,
            items: vec![
                aster_drive::services::policy_service::StoragePolicyGroupItemInput {
                    policy_id: default_policy_id,
                    priority: 1,
                    min_file_size: 0,
                    max_file_size: 0,
                },
            ],
        },
    )
    .await
    .unwrap()
    .id;
    let app = create_test_app!(state);
    let (admin_token, _) = register_and_login!(app);

    admin_create_user!(
        app,
        admin_token,
        "team-admin",
        "team-admin@example.com",
        "password123"
    );
    let (team_admin_token, _) = login_user!(app, "team-admin", "password123");

    let req = test::TestRequest::post()
        .uri("/api/v1/admin/teams")
        .insert_header(("Cookie", format!("aster_access={admin_token}")))
        .set_json(serde_json::json!({
            "name": "Operations",
            "description": "Shared operations workspace",
            "admin_identifier": "team-admin",
            "policy_group_id": default_group_id
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["code"], 0, "{body}");
    let team = &body["data"];
    let team_id = team["id"].as_i64().unwrap();
    assert_eq!(team["name"], "Operations");
    assert_eq!(team["created_by_username"], "testuser");
    assert_eq!(team["member_count"], 1);
    assert_eq!(team["policy_group_id"], default_group_id);

    let req = test::TestRequest::get()
        .uri("/api/v1/admin/teams?keyword=Operations")
        .insert_header(("Cookie", format!("aster_access={admin_token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["total"], 1);
    assert_eq!(body["data"]["items"][0]["id"], team_id);

    let req = test::TestRequest::get()
        .uri("/api/v1/teams")
        .insert_header(("Cookie", format!("aster_access={team_admin_token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"].as_array().unwrap().len(), 1);
    assert_eq!(body["data"][0]["id"], team_id);
    assert_eq!(body["data"][0]["my_role"], "admin");

    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/admin/teams/{team_id}"))
        .insert_header(("Cookie", format!("aster_access={admin_token}")))
        .set_json(serde_json::json!({
            "name": "Operations Core",
            "description": "Updated by admin",
            "policy_group_id": alternate_group_id
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["name"], "Operations Core");
    assert_eq!(body["data"]["description"], "Updated by admin");
    assert_eq!(body["data"]["policy_group_id"], alternate_group_id);

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/register")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "username": "team-analyst",
            "email": "team-analyst@example.com",
            "password": "password123"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);

    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/admin/teams/{team_id}/members?limit=1"))
        .insert_header(("Cookie", format!("aster_access={admin_token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["total"], 1);
    assert_eq!(body["data"]["owner_count"], 0);
    assert_eq!(body["data"]["manager_count"], 1);
    assert_eq!(body["data"]["items"].as_array().unwrap().len(), 1);
    assert_eq!(body["data"]["items"][0]["username"], "team-admin");
    assert_eq!(body["data"]["items"][0]["role"], "admin");

    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/admin/teams/{team_id}/members"))
        .insert_header(("Cookie", format!("aster_access={admin_token}")))
        .set_json(serde_json::json!({
            "identifier": "team-analyst",
            "role": "member"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let analyst_id = body["data"]["user_id"].as_i64().unwrap();
    assert_eq!(body["data"]["username"], "team-analyst");
    assert_eq!(body["data"]["role"], "member");

    let req = test::TestRequest::patch()
        .uri(&format!(
            "/api/v1/admin/teams/{team_id}/members/{analyst_id}"
        ))
        .insert_header(("Cookie", format!("aster_access={admin_token}")))
        .set_json(serde_json::json!({
            "role": "admin"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["role"], "admin");

    let req = test::TestRequest::get()
        .uri(&format!(
            "/api/v1/admin/teams/{team_id}/members?role=admin&status=active&limit=1&offset=1"
        ))
        .insert_header(("Cookie", format!("aster_access={admin_token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["total"], 2);
    assert_eq!(body["data"]["limit"], 1);
    assert_eq!(body["data"]["offset"], 1);
    assert_eq!(body["data"]["owner_count"], 0);
    assert_eq!(body["data"]["manager_count"], 2);
    assert_eq!(body["data"]["items"].as_array().unwrap().len(), 1);
    assert_eq!(body["data"]["items"][0]["username"], "team-analyst");

    let req = test::TestRequest::delete()
        .uri(&format!(
            "/api/v1/admin/teams/{team_id}/members/{analyst_id}"
        ))
        .insert_header(("Cookie", format!("aster_access={admin_token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/admin/teams/{team_id}"))
        .insert_header(("Cookie", format!("aster_access={admin_token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/admin/teams/{team_id}"))
        .insert_header(("Cookie", format!("aster_access={admin_token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert!(body["data"]["archived_at"].is_string());

    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/admin/teams/{team_id}/members"))
        .insert_header(("Cookie", format!("aster_access={admin_token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["items"].as_array().unwrap().len(), 1);
    assert_eq!(body["data"]["items"][0]["username"], "team-admin");

    let req = test::TestRequest::get()
        .uri("/api/v1/admin/teams")
        .insert_header(("Cookie", format!("aster_access={admin_token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["total"], 0);

    let req = test::TestRequest::get()
        .uri("/api/v1/admin/teams?archived=true")
        .insert_header(("Cookie", format!("aster_access={admin_token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["total"], 1);
    assert_eq!(body["data"]["items"][0]["id"], team_id);

    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/admin/teams/{team_id}/restore"))
        .insert_header(("Cookie", format!("aster_access={admin_token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["id"], team_id);

    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/teams/{team_id}/audit-logs"))
        .insert_header(("Cookie", format!("aster_access={team_admin_token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    let user_audit_items = body["data"]["items"].as_array().unwrap();
    let user_actions: Vec<&str> = user_audit_items
        .iter()
        .filter_map(|entry| entry["action"].as_str())
        .collect();
    assert!(user_actions.contains(&"admin_create_team"));
    assert!(user_actions.contains(&"admin_update_team"));
    assert!(user_actions.contains(&"team_member_add"));
    assert!(user_actions.contains(&"team_member_update"));
    assert!(user_actions.contains(&"team_member_remove"));
    assert!(user_actions.contains(&"admin_archive_team"));
    assert!(user_actions.contains(&"admin_restore_team"));
    assert!(
        user_audit_items
            .iter()
            .all(|entry| entry.get("ip_address").is_none())
    );
    assert!(
        user_audit_items
            .iter()
            .all(|entry| entry.get("details").is_none())
    );

    let req = test::TestRequest::get()
        .uri(&format!(
            "/api/v1/admin/audit-logs?entity_type=team&entity_id={team_id}"
        ))
        .insert_header(("Cookie", format!("aster_access={admin_token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    let admin_actions: Vec<&str> = body["data"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|entry| entry["action"].as_str())
        .collect();
    assert!(admin_actions.contains(&"admin_create_team"));
    assert!(admin_actions.contains(&"admin_update_team"));
    assert!(admin_actions.contains(&"team_member_add"));
    assert!(admin_actions.contains(&"team_member_update"));
    assert!(admin_actions.contains(&"team_member_remove"));
    assert!(admin_actions.contains(&"admin_archive_team"));
    assert!(admin_actions.contains(&"admin_restore_team"));

    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/admin/teams/{team_id}/audit-logs"))
        .insert_header(("Cookie", format!("aster_access={admin_token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    let admin_team_audit_items = body["data"]["items"].as_array().unwrap();
    assert!(
        admin_team_audit_items
            .iter()
            .all(|entry| entry.get("ip_address").is_none())
    );
    assert!(
        admin_team_audit_items
            .iter()
            .all(|entry| entry.get("details").is_none())
    );

    let req = test::TestRequest::get()
        .uri("/api/v1/admin/teams")
        .insert_header(("Cookie", format!("aster_access={admin_token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["total"], 1);
}

#[actix_web::test]
async fn test_admin_overview() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/register")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "username": "overview-user",
            "email": "overview-user@example.com",
            "password": "password123"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);

    let file_id = upload_test_file!(app, token);

    let req = test::TestRequest::post()
        .uri("/api/v1/shares")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({ "file_id": file_id }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);

    let req = test::TestRequest::get()
        .uri("/api/v1/admin/overview?days=3&timezone=Asia/Shanghai&event_limit=1")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    let data = &body["data"];

    assert_eq!(data["timezone"], "Asia/Shanghai");
    assert_eq!(data["days"], 3);
    assert_eq!(data["stats"]["total_users"], 2);
    assert_eq!(data["stats"]["active_users"], 2);
    assert_eq!(data["stats"]["disabled_users"], 0);
    assert_eq!(data["stats"]["total_files"], 1);
    assert_eq!(data["stats"]["total_blobs"], 1);
    assert_eq!(data["stats"]["total_shares"], 1);
    assert!(data["stats"]["total_file_bytes"].as_i64().unwrap() > 0);
    assert!(data["stats"]["total_blob_bytes"].as_i64().unwrap() > 0);
    assert_eq!(
        data["stats"]["total_file_bytes"],
        data["stats"]["total_blob_bytes"]
    );
    assert!(data["stats"]["audit_events_today"].as_u64().unwrap() >= 5);
    assert_eq!(data["stats"]["new_users_today"], 2);
    assert_eq!(data["stats"]["uploads_today"], 1);
    assert_eq!(data["stats"]["shares_today"], 1);

    let reports = data["daily_reports"].as_array().unwrap();
    assert_eq!(reports.len(), 3);
    let shanghai_today = chrono::Utc::now()
        .with_timezone(&chrono_tz::Asia::Shanghai)
        .date_naive();
    assert_eq!(
        reports[0]["date"],
        shanghai_today.format("%Y-%m-%d").to_string()
    );
    assert_eq!(
        reports[1]["date"],
        (shanghai_today - chrono::Duration::days(1))
            .format("%Y-%m-%d")
            .to_string()
    );
    assert_eq!(
        reports[2]["date"],
        (shanghai_today - chrono::Duration::days(2))
            .format("%Y-%m-%d")
            .to_string()
    );
    assert_eq!(reports[0]["new_users"], 2);
    assert_eq!(reports[0]["sign_ins"], 1);
    assert_eq!(reports[0]["uploads"], 1);
    assert_eq!(reports[0]["share_creations"], 1);

    let events = data["recent_events"].as_array().unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["action"], "share_create");
}

#[actix_web::test]
async fn test_admin_overview_rejects_invalid_timezone() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let req = test::TestRequest::get()
        .uri("/api/v1/admin/overview?timezone=Not/AZone")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);
}

#[actix_web::test]
async fn test_admin_can_read_uploaded_user_avatar() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (admin_token, _) = register_and_login!(app);

    let user_id = admin_create_user!(
        app,
        admin_token,
        "avatar-user",
        "avatar-user@example.com",
        "password123"
    );
    let (user_token, _) = login_user!(app, "avatar-user", "password123");

    let (boundary, payload) = avatar_upload_payload();
    let req = test::TestRequest::post()
        .uri("/api/v1/auth/profile/avatar/upload")
        .insert_header(("Cookie", format!("aster_access={user_token}")))
        .insert_header((
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        ))
        .set_payload(payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/admin/users/{user_id}"))
        .insert_header(("Cookie", format!("aster_access={admin_token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["profile"]["avatar"]["source"], "upload");
    assert_eq!(
        body["data"]["profile"]["avatar"]["url_512"],
        format!("/admin/users/{user_id}/avatar/512?v=1")
    );

    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/admin/users/{user_id}/avatar/512"))
        .insert_header(("Cookie", format!("aster_access={admin_token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.headers().get("content-type").unwrap(), "image/webp");
}

#[actix_web::test]
async fn test_admin_can_read_user_display_name() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (admin_token, _) = register_and_login!(app);

    let user_id = admin_create_user!(
        app,
        admin_token,
        "named-user",
        "named-user@example.com",
        "password123"
    );
    let (user_token, _) = login_user!(app, "named-user", "password123");

    let req = test::TestRequest::patch()
        .uri("/api/v1/auth/profile")
        .insert_header(("Cookie", format!("aster_access={user_token}")))
        .set_json(serde_json::json!({
            "display_name": "Named User"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::get()
        .uri("/api/v1/admin/users")
        .insert_header(("Cookie", format!("aster_access={admin_token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    let listed_user = body["data"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == user_id)
        .expect("created user should be listed");
    assert_eq!(listed_user["profile"]["display_name"], "Named User");

    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/admin/users/{user_id}"))
        .insert_header(("Cookie", format!("aster_access={admin_token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["profile"]["display_name"], "Named User");
}

#[actix_web::test]
async fn test_admin_create_user() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let req = test::TestRequest::post()
        .uri("/api/v1/admin/users")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "username": "newuser",
            "email": "newuser@example.com",
            "password": "password123"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let user = &body["data"];
    assert_eq!(user["username"], "newuser");
    assert_eq!(user["email"], "newuser@example.com");
    assert_eq!(user["role"], "user");
    assert_eq!(user["status"], "active");
    assert_eq!(user["storage_quota"], 0);
    assert!(user.get("password_hash").is_none());

    let req = test::TestRequest::get()
        .uri("/api/v1/admin/users?keyword=newuser")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["total"], 1);
    assert_eq!(body["data"]["items"][0]["username"], "newuser");
}

#[actix_web::test]
async fn test_non_admin_cannot_create_user() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (admin_token, _) = register_and_login!(app);
    admin_create_user!(
        app,
        admin_token,
        "plainuser",
        "plainuser@example.com",
        "password123"
    );
    let (token, _) = login_user!(app, "plainuser", "password123");

    let req = test::TestRequest::post()
        .uri("/api/v1/admin/users")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "username": "blockeduser",
            "email": "blockeduser@example.com",
            "password": "password123"
        }))
        .to_request();
    let err = test::try_call_service(&app, req).await.unwrap_err();
    let resp = err.error_response();
    assert_eq!(resp.status(), 403);
}

#[actix_web::test]
async fn test_admin_users_server_side_filters() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    for (username, email) in [
        ("filter-alice", "filter-alice@example.com"),
        ("filter-bob", "filter-bob@example.com"),
        ("filter-charlie", "filter-charlie@example.com"),
    ] {
        let req = test::TestRequest::post()
            .uri("/api/v1/auth/register")
            .peer_addr("127.0.0.1:12345".parse().unwrap())
            .set_json(serde_json::json!({
                "username": username,
                "email": email,
                "password": "password123"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 201);
    }

    // 提升 alice 为 admin，禁用 bob
    let req = test::TestRequest::get()
        .uri("/api/v1/admin/users")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    let users = body["data"]["items"].as_array().unwrap();
    let alice_id = users
        .iter()
        .find(|u| u["username"] == "filter-alice")
        .unwrap()["id"]
        .as_i64()
        .unwrap();
    let bob_id = users
        .iter()
        .find(|u| u["username"] == "filter-bob")
        .unwrap()["id"]
        .as_i64()
        .unwrap();

    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/admin/users/{alice_id}"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({"role": "admin"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/admin/users/{bob_id}"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({"status": "disabled"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::get()
        .uri("/api/v1/admin/users?keyword=alice")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    let items = body["data"]["items"].as_array().unwrap();
    assert_eq!(body["data"]["total"], 1);
    assert_eq!(items[0]["username"], "filter-alice");

    let req = test::TestRequest::get()
        .uri("/api/v1/admin/users?role=admin")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    let items = body["data"]["items"].as_array().unwrap();
    assert_eq!(body["data"]["total"], 2);
    assert!(items.iter().all(|u| u["role"] == "admin"));

    let req = test::TestRequest::get()
        .uri("/api/v1/admin/users?status=disabled")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    let items = body["data"]["items"].as_array().unwrap();
    assert_eq!(body["data"]["total"], 1);
    assert_eq!(items[0]["username"], "filter-bob");
}

#[actix_web::test]
async fn test_admin_policies() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let req = test::TestRequest::post()
        .uri("/api/v1/admin/policies")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "name": "Archive S3",
            "driver_type": "s3",
            "endpoint": "https://s3.example.com",
            "bucket": "archive",
            "access_key": "ak",
            "secret_key": "sk",
            "base_path": "backups"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);

    // 列出策略分页（应有 2 个策略，当前取第 2 个）
    let req = test::TestRequest::get()
        .uri("/api/v1/admin/policies?limit=1&offset=1")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    let policies = body["data"]["items"].as_array().unwrap();
    assert_eq!(body["data"]["limit"], 1);
    assert_eq!(body["data"]["offset"], 1);
    assert_eq!(body["data"]["total"], 2);
    assert_eq!(policies.len(), 1);
    assert_eq!(policies[0]["name"], "Archive S3");
    assert_eq!(policies[0]["is_default"], false);
}

#[actix_web::test]
async fn test_admin_config() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    // 设置配置
    let req = test::TestRequest::put()
        .uri("/api/v1/admin/config/test_key")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({ "value": "test_value" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // 读取配置
    let req = test::TestRequest::get()
        .uri("/api/v1/admin/config/test_key")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["value"], "test_value");

    // 列出所有配置
    let req = test::TestRequest::get()
        .uri("/api/v1/admin/config")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert!(!body["data"]["items"].as_array().unwrap().is_empty());
    assert!(body["data"]["total"].as_u64().unwrap() >= 1);

    // 删除配置
    let req = test::TestRequest::delete()
        .uri("/api/v1/admin/config/test_key")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

#[actix_web::test]
async fn test_admin_config_action_sends_test_email() {
    let state = common::setup().await;
    let mail_sender = state.mail_sender.clone();
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let req = test::TestRequest::post()
        .uri("/api/v1/admin/config/mail/action")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "action": "send_test_email",
            "target_email": "deliver@example.com"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(
        body["data"]["message"],
        "Test email sent to deliver@example.com"
    );

    let memory_sender = aster_drive::services::mail_service::memory_sender_ref(&mail_sender)
        .expect("memory mail sender should be available in tests");
    let message = memory_sender
        .last_message()
        .expect("test email should be sent");
    assert_eq!(message.to.address, "deliver@example.com");
    assert_eq!(message.subject, "AsterDrive SMTP test");
    assert!(message.text_body.contains("Triggered by: testuser"));
}

#[actix_web::test]
async fn test_admin_config_action_defaults_to_admin_email() {
    let state = common::setup().await;
    let mail_sender = state.mail_sender.clone();
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let req = test::TestRequest::post()
        .uri("/api/v1/admin/config/mail/action")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "action": "send_test_email"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let memory_sender = aster_drive::services::mail_service::memory_sender_ref(&mail_sender)
        .expect("memory mail sender should be available in tests");
    let message = memory_sender
        .last_message()
        .expect("test email should be sent");
    assert_eq!(message.to.address, "test@example.com");
}

#[actix_web::test]
async fn test_admin_shares() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);
    let file_id = upload_test_file!(app, token);

    // 创建分享
    let req = test::TestRequest::post()
        .uri("/api/v1/shares")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({ "file_id": file_id }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let share_id = body["data"]["id"].as_i64().unwrap();

    // admin 列出所有分享
    let req = test::TestRequest::get()
        .uri("/api/v1/admin/shares")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["items"].as_array().unwrap().len(), 1);
    assert_eq!(body["data"]["total"], 1);

    // admin 删除分享
    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/admin/shares/{share_id}"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

#[actix_web::test]
async fn test_admin_force_unlock() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);
    let file_id = upload_test_file!(app, token);

    // 锁定文件
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/files/{file_id}/lock"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({ "locked": true }))
        .to_request();
    test::call_service(&app, req).await;

    // admin 列出锁
    let req = test::TestRequest::get()
        .uri("/api/v1/admin/locks")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    let locks = body["data"]["items"].as_array().unwrap();
    assert_eq!(body["data"]["total"], 1);
    assert_eq!(locks.len(), 1);
    let lock_id = locks[0]["id"].as_i64().unwrap();

    // admin 强制解锁
    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/admin/locks/{lock_id}"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // 文件应该可以删除了
    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/files/{file_id}"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

#[actix_web::test]
async fn test_admin_batch_update_user() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    // 创建普通用户
    let req = test::TestRequest::post()
        .uri("/api/v1/auth/register")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "username": "batchuser",
            "email": "batchuser@example.com",
            "password": "password123"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let user_id = body["data"]["id"].as_i64().unwrap();
    assert_eq!(body["data"]["role"], "user");
    assert_eq!(body["data"]["status"], "active");
    assert_eq!(body["data"]["storage_quota"], 0);
    assert_eq!(body["data"]["email_verified"], false);

    // 单次 PATCH 同时更新 email_verified + role + status + storage_quota
    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/admin/users/{user_id}"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "email_verified": true,
            "role": "admin",
            "status": "disabled",
            "storage_quota": 1073741824
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    let user = &body["data"];
    assert_eq!(user["email_verified"], true);
    assert_eq!(user["role"], "admin");
    assert_eq!(user["status"], "disabled");
    assert_eq!(user["storage_quota"], 1073741824);

    // 验证 GET 也返回更新后的值
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/admin/users/{user_id}"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["email_verified"], true);
    assert_eq!(body["data"]["role"], "admin");
    assert_eq!(body["data"]["status"], "disabled");
    assert_eq!(body["data"]["storage_quota"], 1073741824);
}

#[actix_web::test]
async fn test_admin_cannot_unverify_initial_admin() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let req = test::TestRequest::patch()
        .uri("/api/v1/admin/users/1")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "email_verified": false
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["msg"], "cannot unverify the initial admin account");
}

#[actix_web::test]
async fn test_admin_can_reset_user_password() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (admin_token, _) = register_and_login!(app);

    let user_id = admin_create_user!(
        app,
        admin_token,
        "resetuser",
        "resetuser@example.com",
        "password123"
    );
    let (old_access, old_refresh) = login_user!(app, "resetuser", "password123");

    let req = test::TestRequest::put()
        .uri(&format!("/api/v1/admin/users/{user_id}/password"))
        .insert_header(("Cookie", format!("aster_access={admin_token}")))
        .set_json(serde_json::json!({
            "password": "resetpass789"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::get()
        .uri("/api/v1/auth/me")
        .insert_header(("Cookie", format!("aster_access={old_access}")))
        .to_request();
    let result = test::try_call_service(&app, req).await;
    match result {
        Ok(resp) => assert_eq!(resp.status(), 401),
        Err(err) => {
            let resp = err.error_response();
            assert_eq!(resp.status(), 401);
        }
    }

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/refresh")
        .insert_header(("Cookie", format!("aster_refresh={old_refresh}")))
        .to_request();
    let result = test::try_call_service(&app, req).await;
    match result {
        Ok(resp) => assert_eq!(resp.status(), 401),
        Err(err) => {
            let resp = err.error_response();
            assert_eq!(resp.status(), 401);
        }
    }

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "identifier": "resetuser",
            "password": "password123"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 401);

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "identifier": "resetuser",
            "password": "resetpass789"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

#[actix_web::test]
async fn test_admin_can_revoke_user_sessions() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (admin_token, _) = register_and_login!(app);

    let user_id = admin_create_user!(
        app,
        admin_token,
        "revokeuser",
        "revokeuser@example.com",
        "password123"
    );
    let (user_access, user_refresh) = login_user!(app, "revokeuser", "password123");

    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/admin/users/{user_id}/sessions/revoke"))
        .insert_header(("Cookie", format!("aster_access={admin_token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::get()
        .uri("/api/v1/auth/me")
        .insert_header(("Cookie", format!("aster_access={user_access}")))
        .to_request();
    assert_service_status!(app, req, 401);

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/refresh")
        .insert_header(("Cookie", format!("aster_refresh={user_refresh}")))
        .to_request();
    assert_service_status!(app, req, 401);
}

#[actix_web::test]
async fn test_admin_role_change_removes_admin_access_without_revoking_session() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (admin_token, _) = register_and_login!(app);

    let user_id = admin_create_user!(
        app,
        admin_token,
        "managedadmin",
        "managedadmin@example.com",
        "password123"
    );

    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/admin/users/{user_id}"))
        .insert_header(("Cookie", format!("aster_access={admin_token}")))
        .set_json(serde_json::json!({ "role": "admin" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let (elevated_access, elevated_refresh) = login_user!(app, "managedadmin", "password123");

    let req = test::TestRequest::get()
        .uri("/api/v1/admin/config/schema")
        .insert_header(("Cookie", format!("aster_access={elevated_access}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/admin/users/{user_id}"))
        .insert_header(("Cookie", format!("aster_access={admin_token}")))
        .set_json(serde_json::json!({ "role": "user" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::get()
        .uri("/api/v1/auth/me")
        .insert_header(("Cookie", format!("aster_access={elevated_access}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/refresh")
        .insert_header(("Cookie", format!("aster_refresh={elevated_refresh}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let rotated_access = common::extract_cookie(&resp, "aster_access").unwrap();

    let req = test::TestRequest::get()
        .uri("/api/v1/admin/config/schema")
        .insert_header(("Cookie", format!("aster_access={elevated_access}")))
        .to_request();
    assert_service_status!(app, req, 403);

    let req = test::TestRequest::get()
        .uri("/api/v1/admin/config/schema")
        .insert_header(("Cookie", format!("aster_access={rotated_access}")))
        .to_request();
    assert_service_status!(app, req, 403);
}

#[actix_web::test]
async fn test_non_admin_cannot_reset_user_password() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (admin_token, _) = register_and_login!(app);

    let user_id = admin_create_user!(
        app,
        admin_token,
        "victimreset",
        "victimreset@example.com",
        "password123"
    );
    admin_create_user!(
        app,
        admin_token,
        "plainuser",
        "plainuser@example.com",
        "password123"
    );
    let (user_token, _) = login_user!(app, "plainuser", "password123");

    let req = test::TestRequest::put()
        .uri(&format!("/api/v1/admin/users/{user_id}/password"))
        .insert_header(("Cookie", format!("aster_access={user_token}")))
        .set_json(serde_json::json!({
            "password": "resetpass789"
        }))
        .to_request();
    let err = test::try_call_service(&app, req).await.unwrap_err();
    let resp = err.error_response();
    assert_eq!(resp.status(), 403);

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "identifier": "victimreset",
            "password": "password123"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::get()
        .uri("/api/v1/admin/users")
        .insert_header(("Cookie", format!("aster_access={admin_token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}
