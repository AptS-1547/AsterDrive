#[macro_use]
mod common;

use actix_web::test;
use chrono::{Duration, Utc};
use sea_orm::{ActiveModelTrait, Set};
use serde_json::{Value, json};
use std::collections::BTreeMap;

use aster_drive::config::{RuntimeConfig, site_url::PUBLIC_SITE_URL_KEY};
use aster_drive::db::repository::{user_repo, wopi_session_repo};
use aster_drive::entities::wopi_session;
use aster_drive::services::preview_app_service::{
    PREVIEW_APPS_CONFIG_KEY, PreviewAppProvider, PreviewOpenMode, PublicPreviewAppConfig,
    PublicPreviewAppDefinition, default_public_preview_apps,
};

const TEST_WOPI_APP_KEY: &str = "custom.onlyoffice";
const TEST_WOPI_ALT_APP_KEY: &str = "custom.onlyoffice.alt";
const TEST_WOPI_ORIGIN: &str = "http://localhost:8080";
const TEST_WOPI_ACTION_URL: &str =
    "http://localhost:8080/hosting/wopi/word/edit?WOPISrc={{wopi_src}}";

macro_rules! register_named_user_and_login {
    ($app:expr, $db:expr, $mail_sender:expr, $username:expr, $email:expr, $password:expr) => {{
        let req = test::TestRequest::post()
            .uri("/api/v1/auth/register")
            .peer_addr("127.0.0.1:12345".parse().unwrap())
            .set_json(json!({
                "username": $username,
                "email": $email,
                "password": $password
            }))
            .to_request();
        let resp = test::call_service(&$app, req).await;
        assert_eq!(resp.status(), 201);
        let body: serde_json::Value = test::read_body_json(resp).await;
        let user_id = body["data"]["id"].as_i64().unwrap();
        let _ = confirm_latest_contact_verification!($app, $db, $mail_sender);

        let req = test::TestRequest::post()
            .uri("/api/v1/auth/login")
            .peer_addr("127.0.0.1:12345".parse().unwrap())
            .set_json(json!({
                "identifier": $username,
                "password": $password
            }))
            .to_request();
        let resp = test::call_service(&$app, req).await;
        assert_eq!(resp.status(), 200);
        let access_cookie = common::extract_cookie(&resp, "aster_access").unwrap();
        (user_id, access_cookie)
    }};
}

macro_rules! team_multipart_request {
    ($uri:expr, $token:expr, $filename:expr, $content:expr $(,)?) => {{
        let boundary = "----WopiTeamBoundary123";
        let payload = format!(
            "------WopiTeamBoundary123\r\n\
             Content-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\n\
             Content-Type: text/plain\r\n\r\n\
             {content}\r\n\
             ------WopiTeamBoundary123--\r\n",
            filename = $filename,
            content = $content,
        );

        test::TestRequest::post()
            .uri($uri)
            .insert_header(("Cookie", format!("aster_access={}", $token)))
            .insert_header((
                "Content-Type",
                format!("multipart/form-data; boundary={boundary}"),
            ))
            .set_payload(payload)
            .to_request()
    }};
}

macro_rules! open_wopi_session {
    ($app:expr, $token:expr, $uri:expr, $app_key:expr) => {{
        let req = test::TestRequest::post()
            .uri($uri)
            .insert_header(("Cookie", format!("aster_access={}", $token)))
            .set_json(json!({ "app_key": $app_key }))
            .to_request();
        let resp = test::call_service(&$app, req).await;
        assert_eq!(resp.status(), 200);
        test::read_body_json::<serde_json::Value, _>(resp).await
    }};
}

fn test_wopi_app_definition(
    key: &str,
    action_url: &str,
    enabled: bool,
) -> PublicPreviewAppDefinition {
    PublicPreviewAppDefinition {
        key: key.to_string(),
        provider: PreviewAppProvider::Wopi,
        icon: "/static/preview-apps/microsoft-onedrive.svg".to_string(),
        enabled,
        label_i18n_key: None,
        labels: BTreeMap::from([("en".to_string(), "OnlyOffice".to_string())]),
        extensions: vec!["docx".to_string(), "xlsx".to_string(), "pptx".to_string()],
        config: PublicPreviewAppConfig {
            mode: Some(PreviewOpenMode::Iframe),
            action_url: Some(action_url.to_string()),
            ..Default::default()
        },
    }
}

fn apply_test_wopi_registry(
    runtime_config: &RuntimeConfig,
    extra_apps: Vec<PublicPreviewAppDefinition>,
) {
    let mut registry = default_public_preview_apps();
    registry.apps.extend(extra_apps);
    runtime_config.apply(common::system_config_model(
        PREVIEW_APPS_CONFIG_KEY,
        &serde_json::to_string(&registry).unwrap(),
    ));
}

fn configure_test_wopi_runtime(state: &aster_drive::runtime::AppState) {
    state.runtime_config.apply(common::system_config_model(
        PUBLIC_SITE_URL_KEY,
        "https://drive.example.com",
    ));
    apply_test_wopi_registry(
        &state.runtime_config,
        vec![test_wopi_app_definition(
            TEST_WOPI_APP_KEY,
            TEST_WOPI_ACTION_URL,
            true,
        )],
    );
}

fn wopi_file_query(file_id: i64, access_token: &str) -> String {
    format!(
        "/api/v1/wopi/files/{file_id}?access_token={}",
        urlencoding::encode(access_token)
    )
}

fn wopi_contents_query(file_id: i64, access_token: &str) -> String {
    format!(
        "/api/v1/wopi/files/{file_id}/contents?access_token={}",
        urlencoding::encode(access_token)
    )
}

#[actix_web::test]
async fn test_open_wopi_session_persists_token_and_check_file_info_succeeds() {
    let state = common::setup().await;
    configure_test_wopi_runtime(&state);
    let db = state.db.clone();
    let app = create_test_app!(state);

    let (access_cookie, _) = register_and_login!(app);
    let user = user_repo::find_by_username(&db, "testuser")
        .await
        .unwrap()
        .expect("registered user should exist");
    let file_id = upload_test_file_named!(app, access_cookie, "report 1.docx");

    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/files/{file_id}/wopi/open"))
        .insert_header(("Cookie", format!("aster_access={access_cookie}")))
        .set_json(json!({ "app_key": TEST_WOPI_APP_KEY }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: Value = test::read_body_json(resp).await;
    let launch = &body["data"];
    let wopi_access_token = launch["access_token"]
        .as_str()
        .expect("launch session should return access token")
        .to_string();
    let action_url = launch["action_url"]
        .as_str()
        .expect("launch session should return action url");
    let expected_wopi_src_raw = format!("https://drive.example.com/api/v1/wopi/files/{file_id}");
    let expected_wopi_src = urlencoding::encode(&expected_wopi_src_raw);
    assert!(
        action_url.contains(&format!("WOPISrc={expected_wopi_src}")),
        "action_url should include encoded WOPISrc, got {action_url}"
    );
    assert_eq!(launch["mode"], "iframe");
    assert!(
        launch["access_token_ttl"]
            .as_i64()
            .is_some_and(|value| value > 0)
    );

    let token_hash = aster_drive::utils::hash::sha256_hex(wopi_access_token.as_bytes());
    let stored_session = wopi_session_repo::find_by_token_hash(&db, &token_hash)
        .await
        .unwrap()
        .expect("launch session should be persisted");
    assert_eq!(stored_session.actor_user_id, user.id);
    assert_eq!(stored_session.file_id, file_id);
    assert_eq!(stored_session.app_key, TEST_WOPI_APP_KEY);
    assert_eq!(stored_session.session_version, user.session_version);

    let req = test::TestRequest::get()
        .uri(&format!(
            "/api/v1/wopi/files/{file_id}?access_token={}",
            urlencoding::encode(&wopi_access_token)
        ))
        .insert_header(("Origin", TEST_WOPI_ORIGIN))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["BaseFileName"], "report 1.docx");
    assert_eq!(body["OwnerId"], user.id.to_string());
    assert_eq!(body["UserId"], user.id.to_string());
    assert_eq!(body["UserCanNotWriteRelative"], true);
    assert_eq!(body["UserCanRename"], false);
    assert_eq!(body["UserCanWrite"], true);
    assert_eq!(body["ReadOnly"], false);
    assert_eq!(body["SupportsGetLock"], false);
    assert_eq!(body["SupportsLocks"], true);
    assert_eq!(body["SupportsRename"], false);
    assert_eq!(body["SupportsUpdate"], true);
    assert!(
        body["Version"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
    );
}

#[actix_web::test]
async fn test_open_wopi_session_appends_wopisrc_when_action_url_has_no_placeholder() {
    let state = common::setup().await;
    state.runtime_config.apply(common::system_config_model(
        PUBLIC_SITE_URL_KEY,
        "https://drive.example.com",
    ));
    apply_test_wopi_registry(
        &state.runtime_config,
        vec![test_wopi_app_definition(
            TEST_WOPI_ALT_APP_KEY,
            "http://localhost:8080/hosting/wopi/word/edit?lang=zh-CN",
            true,
        )],
    );
    let app = create_test_app!(state);

    let (access_cookie, _) = register_and_login!(app);
    let file_id = upload_test_file_named!(app, access_cookie, "report 1.docx");
    let launch = open_wopi_session!(
        app,
        access_cookie,
        &format!("/api/v1/files/{file_id}/wopi/open"),
        TEST_WOPI_ALT_APP_KEY
    );
    let action_url = launch["data"]["action_url"].as_str().unwrap();

    assert!(action_url.contains("lang=zh-CN"));
    assert!(action_url.contains("WOPISrc="));
    assert!(action_url.contains("%2Fapi%2Fv1%2Fwopi%2Ffiles%2F"));
}

#[actix_web::test]
async fn test_wopi_check_file_info_allows_missing_origin_and_referer() {
    let state = common::setup().await;
    configure_test_wopi_runtime(&state);
    let app = create_test_app!(state);

    let (access_cookie, _) = register_and_login!(app);
    let file_id = upload_test_file_named!(app, access_cookie, "report 1.docx");
    let launch = open_wopi_session!(
        app,
        access_cookie,
        &format!("/api/v1/files/{file_id}/wopi/open"),
        TEST_WOPI_APP_KEY
    );
    let wopi_access_token = launch["data"]["access_token"].as_str().unwrap();

    let req = test::TestRequest::get()
        .uri(&wopi_file_query(file_id, wopi_access_token))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

#[actix_web::test]
async fn test_wopi_check_file_info_allows_trusted_referer_without_origin() {
    let state = common::setup().await;
    configure_test_wopi_runtime(&state);
    let app = create_test_app!(state);

    let (access_cookie, _) = register_and_login!(app);
    let file_id = upload_test_file_named!(app, access_cookie, "report 1.docx");
    let launch = open_wopi_session!(
        app,
        access_cookie,
        &format!("/api/v1/files/{file_id}/wopi/open"),
        TEST_WOPI_APP_KEY
    );
    let wopi_access_token = launch["data"]["access_token"].as_str().unwrap();

    let req = test::TestRequest::get()
        .uri(&wopi_file_query(file_id, wopi_access_token))
        .insert_header((
            "Referer",
            "http://localhost:8080/hosting/wopi/word/edit?WOPISrc=http%3A%2F%2Flocalhost%3A3000%2Fapi%2Fv1%2Fwopi%2Ffiles%2F1",
        ))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

#[actix_web::test]
async fn test_wopi_check_file_info_rejects_token_for_another_file() {
    let state = common::setup().await;
    configure_test_wopi_runtime(&state);
    let app = create_test_app!(state);

    let (access_cookie, _) = register_and_login!(app);
    let file_id = upload_test_file_named!(app, access_cookie, "report 1.docx");
    let other_file_id = upload_test_file_named!(app, access_cookie, "report 2.docx");
    let launch = open_wopi_session!(
        app,
        access_cookie,
        &format!("/api/v1/files/{file_id}/wopi/open"),
        TEST_WOPI_APP_KEY
    );
    let wopi_access_token = launch["data"]["access_token"].as_str().unwrap();

    let req = test::TestRequest::get()
        .uri(&wopi_file_query(other_file_id, wopi_access_token))
        .insert_header(("Origin", TEST_WOPI_ORIGIN))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 404);

    let body = test::read_body(resp).await;
    let message = String::from_utf8(body.to_vec()).unwrap();
    assert!(message.contains("does not match file"));
}

#[actix_web::test]
async fn test_wopi_lock_put_get_and_unlock_lifecycle() {
    let state = common::setup().await;
    configure_test_wopi_runtime(&state);
    let app = create_test_app!(state);

    let (access_cookie, _) = register_and_login!(app);
    let file_id = upload_test_file_named!(app, access_cookie, "report 1.docx");
    let launch = open_wopi_session!(
        app,
        access_cookie,
        &format!("/api/v1/files/{file_id}/wopi/open"),
        TEST_WOPI_APP_KEY
    );
    let wopi_access_token = launch["data"]["access_token"].as_str().unwrap();
    let lock_value = "wopi-lock-1";

    let req = test::TestRequest::post()
        .uri(&wopi_file_query(file_id, wopi_access_token))
        .insert_header(("Origin", TEST_WOPI_ORIGIN))
        .insert_header(("X-WOPI-Override", "LOCK"))
        .insert_header(("X-WOPI-Lock", lock_value))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/files/{file_id}"))
        .insert_header(("Cookie", format!("aster_access={access_cookie}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["is_locked"], true);

    let req = test::TestRequest::post()
        .uri(&wopi_contents_query(file_id, wopi_access_token))
        .insert_header(("Origin", TEST_WOPI_ORIGIN))
        .insert_header(("X-WOPI-Override", "PUT"))
        .insert_header(("X-WOPI-Lock", lock_value))
        .set_payload("edited via wopi")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    assert!(
        resp.headers()
            .get("X-WOPI-ItemVersion")
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| !value.is_empty())
    );

    let req = test::TestRequest::get()
        .uri(&wopi_contents_query(file_id, wopi_access_token))
        .insert_header(("Origin", TEST_WOPI_ORIGIN))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body = test::read_body(resp).await;
    assert_eq!(&body[..], b"edited via wopi");

    let req = test::TestRequest::post()
        .uri(&wopi_file_query(file_id, wopi_access_token))
        .insert_header(("Origin", TEST_WOPI_ORIGIN))
        .insert_header(("X-WOPI-Override", "REFRESH_LOCK"))
        .insert_header(("X-WOPI-Lock", lock_value))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::post()
        .uri(&wopi_file_query(file_id, wopi_access_token))
        .insert_header(("Origin", TEST_WOPI_ORIGIN))
        .insert_header(("X-WOPI-Override", "UNLOCK"))
        .insert_header(("X-WOPI-Lock", lock_value))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/files/{file_id}"))
        .insert_header(("Cookie", format!("aster_access={access_cookie}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["is_locked"], false);
}

#[actix_web::test]
async fn test_wopi_lock_returns_conflict_for_another_wopi_session() {
    let state = common::setup().await;
    configure_test_wopi_runtime(&state);
    let app = create_test_app!(state);

    let (access_cookie, _) = register_and_login!(app);
    let file_id = upload_test_file_named!(app, access_cookie, "report 1.docx");
    let launch_a = open_wopi_session!(
        app,
        access_cookie,
        &format!("/api/v1/files/{file_id}/wopi/open"),
        TEST_WOPI_APP_KEY
    );
    let launch_b = open_wopi_session!(
        app,
        access_cookie,
        &format!("/api/v1/files/{file_id}/wopi/open"),
        TEST_WOPI_APP_KEY
    );
    let token_a = launch_a["data"]["access_token"].as_str().unwrap();
    let token_b = launch_b["data"]["access_token"].as_str().unwrap();

    let req = test::TestRequest::post()
        .uri(&wopi_file_query(file_id, token_a))
        .insert_header(("Origin", TEST_WOPI_ORIGIN))
        .insert_header(("X-WOPI-Override", "LOCK"))
        .insert_header(("X-WOPI-Lock", "lock-a"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::post()
        .uri(&wopi_file_query(file_id, token_b))
        .insert_header(("Origin", TEST_WOPI_ORIGIN))
        .insert_header(("X-WOPI-Override", "LOCK"))
        .insert_header(("X-WOPI-Lock", "lock-b"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 409);
    assert_eq!(
        resp.headers()
            .get("X-WOPI-Lock")
            .and_then(|value| value.to_str().ok()),
        Some("lock-a")
    );
}

#[actix_web::test]
async fn test_wopi_lock_returns_conflict_when_file_is_locked_outside_wopi() {
    let state = common::setup().await;
    configure_test_wopi_runtime(&state);
    let app = create_test_app!(state);

    let (access_cookie, _) = register_and_login!(app);
    let file_id = upload_test_file_named!(app, access_cookie, "report 1.docx");
    let launch = open_wopi_session!(
        app,
        access_cookie,
        &format!("/api/v1/files/{file_id}/wopi/open"),
        TEST_WOPI_APP_KEY
    );
    let wopi_access_token = launch["data"]["access_token"].as_str().unwrap();

    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/files/{file_id}/lock"))
        .insert_header(("Cookie", format!("aster_access={access_cookie}")))
        .set_json(json!({ "locked": true }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::post()
        .uri(&wopi_file_query(file_id, wopi_access_token))
        .insert_header(("Origin", TEST_WOPI_ORIGIN))
        .insert_header(("X-WOPI-Override", "LOCK"))
        .insert_header(("X-WOPI-Lock", "wopi-lock-1"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 409);
    assert!(resp.headers().get("X-WOPI-Lock").is_none());
    assert!(
        resp.headers()
            .get("X-WOPI-LockFailureReason")
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.contains("outside WOPI"))
    );
}

#[actix_web::test]
async fn test_wopi_put_file_returns_conflict_for_lock_mismatch() {
    let state = common::setup().await;
    configure_test_wopi_runtime(&state);
    let app = create_test_app!(state);

    let (access_cookie, _) = register_and_login!(app);
    let file_id = upload_test_file_named!(app, access_cookie, "report 1.docx");
    let launch = open_wopi_session!(
        app,
        access_cookie,
        &format!("/api/v1/files/{file_id}/wopi/open"),
        TEST_WOPI_APP_KEY
    );
    let wopi_access_token = launch["data"]["access_token"].as_str().unwrap();

    let req = test::TestRequest::post()
        .uri(&wopi_file_query(file_id, wopi_access_token))
        .insert_header(("Origin", TEST_WOPI_ORIGIN))
        .insert_header(("X-WOPI-Override", "LOCK"))
        .insert_header(("X-WOPI-Lock", "lock-a"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::post()
        .uri(&wopi_contents_query(file_id, wopi_access_token))
        .insert_header(("Origin", TEST_WOPI_ORIGIN))
        .insert_header(("X-WOPI-Override", "PUT"))
        .insert_header(("X-WOPI-Lock", "lock-b"))
        .set_payload("should conflict")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 409);
    assert_eq!(
        resp.headers()
            .get("X-WOPI-Lock")
            .and_then(|value| value.to_str().ok()),
        Some("lock-a")
    );
    assert!(
        resp.headers()
            .get("X-WOPI-LockFailureReason")
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.contains("mismatch"))
    );
}

#[actix_web::test]
async fn test_wopi_put_file_requires_lock_header_when_wopi_lock_exists() {
    let state = common::setup().await;
    configure_test_wopi_runtime(&state);
    let app = create_test_app!(state);

    let (access_cookie, _) = register_and_login!(app);
    let file_id = upload_test_file_named!(app, access_cookie, "report 1.docx");
    let launch = open_wopi_session!(
        app,
        access_cookie,
        &format!("/api/v1/files/{file_id}/wopi/open"),
        TEST_WOPI_APP_KEY
    );
    let wopi_access_token = launch["data"]["access_token"].as_str().unwrap();

    let req = test::TestRequest::post()
        .uri(&wopi_file_query(file_id, wopi_access_token))
        .insert_header(("Origin", TEST_WOPI_ORIGIN))
        .insert_header(("X-WOPI-Override", "LOCK"))
        .insert_header(("X-WOPI-Lock", "lock-a"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::post()
        .uri(&wopi_contents_query(file_id, wopi_access_token))
        .insert_header(("Origin", TEST_WOPI_ORIGIN))
        .insert_header(("X-WOPI-Override", "PUT"))
        .set_payload("missing lock header")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);

    let body = test::read_body(resp).await;
    let message = String::from_utf8(body.to_vec()).unwrap();
    assert!(message.contains("X-WOPI-Lock header is required"));
}

#[actix_web::test]
async fn test_wopi_refresh_lock_without_active_lock_returns_conflict() {
    let state = common::setup().await;
    configure_test_wopi_runtime(&state);
    let app = create_test_app!(state);

    let (access_cookie, _) = register_and_login!(app);
    let file_id = upload_test_file_named!(app, access_cookie, "report 1.docx");
    let launch = open_wopi_session!(
        app,
        access_cookie,
        &format!("/api/v1/files/{file_id}/wopi/open"),
        TEST_WOPI_APP_KEY
    );
    let wopi_access_token = launch["data"]["access_token"].as_str().unwrap();

    let req = test::TestRequest::post()
        .uri(&wopi_file_query(file_id, wopi_access_token))
        .insert_header(("Origin", TEST_WOPI_ORIGIN))
        .insert_header(("X-WOPI-Override", "REFRESH_LOCK"))
        .insert_header(("X-WOPI-Lock", "lock-a"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 409);
    assert!(
        resp.headers()
            .get("X-WOPI-LockFailureReason")
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.contains("not locked"))
    );
}

#[actix_web::test]
async fn test_wopi_unlock_without_active_lock_returns_conflict() {
    let state = common::setup().await;
    configure_test_wopi_runtime(&state);
    let app = create_test_app!(state);

    let (access_cookie, _) = register_and_login!(app);
    let file_id = upload_test_file_named!(app, access_cookie, "report 1.docx");
    let launch = open_wopi_session!(
        app,
        access_cookie,
        &format!("/api/v1/files/{file_id}/wopi/open"),
        TEST_WOPI_APP_KEY
    );
    let wopi_access_token = launch["data"]["access_token"].as_str().unwrap();

    let req = test::TestRequest::post()
        .uri(&wopi_file_query(file_id, wopi_access_token))
        .insert_header(("Origin", TEST_WOPI_ORIGIN))
        .insert_header(("X-WOPI-Override", "UNLOCK"))
        .insert_header(("X-WOPI-Lock", "lock-a"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 409);
    assert!(resp.headers().get("X-WOPI-Lock").is_none());
    assert!(
        resp.headers()
            .get("X-WOPI-LockFailureReason")
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.contains("not locked"))
    );
}

#[actix_web::test]
async fn test_wopi_unlock_returns_conflict_for_lock_mismatch() {
    let state = common::setup().await;
    configure_test_wopi_runtime(&state);
    let app = create_test_app!(state);

    let (access_cookie, _) = register_and_login!(app);
    let file_id = upload_test_file_named!(app, access_cookie, "report 1.docx");
    let launch = open_wopi_session!(
        app,
        access_cookie,
        &format!("/api/v1/files/{file_id}/wopi/open"),
        TEST_WOPI_APP_KEY
    );
    let wopi_access_token = launch["data"]["access_token"].as_str().unwrap();

    let req = test::TestRequest::post()
        .uri(&wopi_file_query(file_id, wopi_access_token))
        .insert_header(("Origin", TEST_WOPI_ORIGIN))
        .insert_header(("X-WOPI-Override", "LOCK"))
        .insert_header(("X-WOPI-Lock", "lock-a"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::post()
        .uri(&wopi_file_query(file_id, wopi_access_token))
        .insert_header(("Origin", TEST_WOPI_ORIGIN))
        .insert_header(("X-WOPI-Override", "UNLOCK"))
        .insert_header(("X-WOPI-Lock", "lock-b"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 409);
    assert_eq!(
        resp.headers()
            .get("X-WOPI-Lock")
            .and_then(|value| value.to_str().ok()),
        Some("lock-a")
    );
    assert!(
        resp.headers()
            .get("X-WOPI-LockFailureReason")
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.contains("mismatch"))
    );
}

#[actix_web::test]
async fn test_wopi_put_file_contents_rejects_non_put_override() {
    let state = common::setup().await;
    configure_test_wopi_runtime(&state);
    let app = create_test_app!(state);

    let (access_cookie, _) = register_and_login!(app);
    let file_id = upload_test_file_named!(app, access_cookie, "report 1.docx");
    let launch = open_wopi_session!(
        app,
        access_cookie,
        &format!("/api/v1/files/{file_id}/wopi/open"),
        TEST_WOPI_APP_KEY
    );
    let wopi_access_token = launch["data"]["access_token"].as_str().unwrap();

    let req = test::TestRequest::post()
        .uri(&wopi_contents_query(file_id, wopi_access_token))
        .insert_header(("Origin", TEST_WOPI_ORIGIN))
        .insert_header(("X-WOPI-Override", "LOCK"))
        .set_payload("unsupported override")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 501);
}

#[actix_web::test]
async fn test_wopi_put_relative_is_not_implemented_yet() {
    let state = common::setup().await;
    configure_test_wopi_runtime(&state);
    let app = create_test_app!(state);

    let (access_cookie, _) = register_and_login!(app);
    let file_id = upload_test_file_named!(app, access_cookie, "report 1.docx");
    let launch = open_wopi_session!(
        app,
        access_cookie,
        &format!("/api/v1/files/{file_id}/wopi/open"),
        TEST_WOPI_APP_KEY
    );
    let wopi_access_token = launch["data"]["access_token"].as_str().unwrap();

    let req = test::TestRequest::post()
        .uri(&wopi_file_query(file_id, wopi_access_token))
        .insert_header(("Origin", TEST_WOPI_ORIGIN))
        .insert_header(("X-WOPI-Override", "PUT_RELATIVE"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 501);
}

#[actix_web::test]
async fn test_wopi_file_operation_rejects_unknown_override() {
    let state = common::setup().await;
    configure_test_wopi_runtime(&state);
    let app = create_test_app!(state);

    let (access_cookie, _) = register_and_login!(app);
    let file_id = upload_test_file_named!(app, access_cookie, "report 1.docx");
    let launch = open_wopi_session!(
        app,
        access_cookie,
        &format!("/api/v1/files/{file_id}/wopi/open"),
        TEST_WOPI_APP_KEY
    );
    let wopi_access_token = launch["data"]["access_token"].as_str().unwrap();

    let req = test::TestRequest::post()
        .uri(&wopi_file_query(file_id, wopi_access_token))
        .insert_header(("Origin", TEST_WOPI_ORIGIN))
        .insert_header(("X-WOPI-Override", "DELETE"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 501);
}

#[actix_web::test]
async fn test_wopi_check_file_info_rejects_untrusted_origin() {
    let state = common::setup().await;
    configure_test_wopi_runtime(&state);
    let app = create_test_app!(state);

    let (access_cookie, _) = register_and_login!(app);
    let file_id = upload_test_file_named!(app, access_cookie, "report 1.docx");

    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/files/{file_id}/wopi/open"))
        .insert_header(("Cookie", format!("aster_access={access_cookie}")))
        .set_json(json!({ "app_key": TEST_WOPI_APP_KEY }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    let wopi_access_token = body["data"]["access_token"]
        .as_str()
        .expect("launch session should return access token");

    let req = test::TestRequest::get()
        .uri(&format!(
            "/api/v1/wopi/files/{file_id}?access_token={}",
            urlencoding::encode(wopi_access_token)
        ))
        .insert_header(("Origin", "https://evil.example.com"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 401);

    let body = test::read_body(resp).await;
    let message = String::from_utf8(body.to_vec()).unwrap();
    assert!(message.contains("untrusted WOPI request origin"));
}

#[actix_web::test]
async fn test_wopi_check_file_info_rejects_invalid_origin_header() {
    let state = common::setup().await;
    configure_test_wopi_runtime(&state);
    let app = create_test_app!(state);

    let (access_cookie, _) = register_and_login!(app);
    let file_id = upload_test_file_named!(app, access_cookie, "report 1.docx");
    let launch = open_wopi_session!(
        app,
        access_cookie,
        &format!("/api/v1/files/{file_id}/wopi/open"),
        TEST_WOPI_APP_KEY
    );
    let wopi_access_token = launch["data"]["access_token"].as_str().unwrap();

    let req = test::TestRequest::get()
        .uri(&wopi_file_query(file_id, wopi_access_token))
        .insert_header(("Origin", "not-a-valid-origin"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);

    let body = test::read_body(resp).await;
    let message = String::from_utf8(body.to_vec()).unwrap();
    assert!(message.contains("invalid Origin header"));
}

#[actix_web::test]
async fn test_wopi_check_file_info_rejects_invalid_referer_header() {
    let state = common::setup().await;
    configure_test_wopi_runtime(&state);
    let app = create_test_app!(state);

    let (access_cookie, _) = register_and_login!(app);
    let file_id = upload_test_file_named!(app, access_cookie, "report 1.docx");
    let launch = open_wopi_session!(
        app,
        access_cookie,
        &format!("/api/v1/files/{file_id}/wopi/open"),
        TEST_WOPI_APP_KEY
    );
    let wopi_access_token = launch["data"]["access_token"].as_str().unwrap();

    let req = test::TestRequest::get()
        .uri(&wopi_file_query(file_id, wopi_access_token))
        .insert_header(("Referer", "not-a-valid-referer"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);

    let body = test::read_body(resp).await;
    let message = String::from_utf8(body.to_vec()).unwrap();
    assert!(message.contains("invalid Referer header"));
}

#[actix_web::test]
async fn test_wopi_check_file_info_rejects_untrusted_referer() {
    let state = common::setup().await;
    configure_test_wopi_runtime(&state);
    let app = create_test_app!(state);

    let (access_cookie, _) = register_and_login!(app);
    let file_id = upload_test_file_named!(app, access_cookie, "report 1.docx");
    let launch = open_wopi_session!(
        app,
        access_cookie,
        &format!("/api/v1/files/{file_id}/wopi/open"),
        TEST_WOPI_APP_KEY
    );
    let wopi_access_token = launch["data"]["access_token"].as_str().unwrap();

    let req = test::TestRequest::get()
        .uri(&wopi_file_query(file_id, wopi_access_token))
        .insert_header(("Referer", "https://evil.example.com/editor?x=1"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 401);

    let body = test::read_body(resp).await;
    let message = String::from_utf8(body.to_vec()).unwrap();
    assert!(message.contains("untrusted WOPI request referer"));
}

#[actix_web::test]
async fn test_disabled_wopi_app_invalidates_existing_access_token() {
    let state = common::setup().await;
    configure_test_wopi_runtime(&state);
    let db = state.db.clone();
    let runtime_config = state.runtime_config.clone();
    let app = create_test_app!(state);

    let (access_cookie, _) = register_and_login!(app);
    let file_id = upload_test_file_named!(app, access_cookie, "report 1.docx");
    let launch = open_wopi_session!(
        app,
        access_cookie,
        &format!("/api/v1/files/{file_id}/wopi/open"),
        TEST_WOPI_APP_KEY
    );
    let wopi_access_token = launch["data"]["access_token"].as_str().unwrap().to_string();
    let token_hash = aster_drive::utils::hash::sha256_hex(wopi_access_token.as_bytes());

    apply_test_wopi_registry(
        &runtime_config,
        vec![test_wopi_app_definition(
            TEST_WOPI_APP_KEY,
            TEST_WOPI_ACTION_URL,
            false,
        )],
    );

    let req = test::TestRequest::get()
        .uri(&wopi_file_query(file_id, &wopi_access_token))
        .insert_header(("Origin", TEST_WOPI_ORIGIN))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 401);

    let body = test::read_body(resp).await;
    let message = String::from_utf8(body.to_vec()).unwrap();
    assert!(
        message.contains("WOPI app is disabled")
            || message.contains("WOPI app is no longer available")
    );
    assert!(
        wopi_session_repo::find_by_token_hash(&db, &token_hash)
            .await
            .unwrap()
            .is_none(),
        "disabled app should invalidate persisted WOPI sessions"
    );
}

#[actix_web::test]
async fn test_disabled_user_invalidates_wopi_access_token() {
    let state = common::setup().await;
    configure_test_wopi_runtime(&state);
    let db = state.db.clone();
    let mail_sender = state.mail_sender.clone();
    let app = create_test_app!(state);

    let (admin_token, _) = register_and_login!(app);
    let (user_id, user_token) = register_named_user_and_login!(
        app,
        db,
        mail_sender,
        "wopidisabled",
        "wopidisabled@example.com",
        "password123"
    );
    let file_id = upload_test_file_named!(app, user_token, "report 1.docx");
    let launch = open_wopi_session!(
        app,
        user_token,
        &format!("/api/v1/files/{file_id}/wopi/open"),
        TEST_WOPI_APP_KEY
    );
    let wopi_access_token = launch["data"]["access_token"].as_str().unwrap().to_string();
    let token_hash = aster_drive::utils::hash::sha256_hex(wopi_access_token.as_bytes());

    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/admin/users/{user_id}"))
        .insert_header(("Cookie", format!("aster_access={admin_token}")))
        .set_json(json!({ "status": "disabled" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::get()
        .uri(&wopi_file_query(file_id, &wopi_access_token))
        .insert_header(("Origin", TEST_WOPI_ORIGIN))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 401);

    let body = test::read_body(resp).await;
    let message = String::from_utf8(body.to_vec()).unwrap();
    assert!(message.contains("account is disabled"));
    assert!(
        wopi_session_repo::find_by_token_hash(&db, &token_hash)
            .await
            .unwrap()
            .is_none(),
        "disabled account should invalidate persisted WOPI sessions"
    );
}

#[actix_web::test]
async fn test_expired_wopi_access_token_is_rejected_and_removed() {
    let state = common::setup().await;
    configure_test_wopi_runtime(&state);
    let db = state.db.clone();
    let app = create_test_app!(state);

    let (access_cookie, _) = register_and_login!(app);
    let file_id = upload_test_file_named!(app, access_cookie, "report 1.docx");
    let launch = open_wopi_session!(
        app,
        access_cookie,
        &format!("/api/v1/files/{file_id}/wopi/open"),
        TEST_WOPI_APP_KEY
    );
    let wopi_access_token = launch["data"]["access_token"].as_str().unwrap().to_string();
    let token_hash = aster_drive::utils::hash::sha256_hex(wopi_access_token.as_bytes());
    let stored_session = wopi_session_repo::find_by_token_hash(&db, &token_hash)
        .await
        .unwrap()
        .expect("launch session should be persisted");

    let mut expired_session: wopi_session::ActiveModel = stored_session.into();
    expired_session.expires_at = Set(Utc::now() - Duration::minutes(5));
    expired_session.update(&db).await.unwrap();

    let req = test::TestRequest::get()
        .uri(&wopi_file_query(file_id, &wopi_access_token))
        .insert_header(("Origin", TEST_WOPI_ORIGIN))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 401);

    let body = test::read_body(resp).await;
    let message = String::from_utf8(body.to_vec()).unwrap();
    assert!(message.contains("WOPI access token expired"));
    assert!(
        wopi_session_repo::find_by_token_hash(&db, &token_hash)
            .await
            .unwrap()
            .is_none(),
        "expired WOPI session should be cleaned up on access"
    );
}

#[actix_web::test]
async fn test_wopi_lock_rejects_empty_lock_header() {
    let state = common::setup().await;
    configure_test_wopi_runtime(&state);
    let app = create_test_app!(state);

    let (access_cookie, _) = register_and_login!(app);
    let file_id = upload_test_file_named!(app, access_cookie, "report 1.docx");
    let launch = open_wopi_session!(
        app,
        access_cookie,
        &format!("/api/v1/files/{file_id}/wopi/open"),
        TEST_WOPI_APP_KEY
    );
    let wopi_access_token = launch["data"]["access_token"].as_str().unwrap();

    let req = test::TestRequest::post()
        .uri(&wopi_file_query(file_id, wopi_access_token))
        .insert_header(("Origin", TEST_WOPI_ORIGIN))
        .insert_header(("X-WOPI-Override", "LOCK"))
        .insert_header(("X-WOPI-Lock", "   "))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);

    let body = test::read_body(resp).await;
    let message = String::from_utf8(body.to_vec()).unwrap();
    assert!(message.contains("X-WOPI-Lock header must not be empty"));
}

#[actix_web::test]
async fn test_wopi_put_file_returns_conflict_when_file_is_locked_outside_wopi() {
    let state = common::setup().await;
    configure_test_wopi_runtime(&state);
    let app = create_test_app!(state);

    let (access_cookie, _) = register_and_login!(app);
    let file_id = upload_test_file_named!(app, access_cookie, "report 1.docx");
    let launch = open_wopi_session!(
        app,
        access_cookie,
        &format!("/api/v1/files/{file_id}/wopi/open"),
        TEST_WOPI_APP_KEY
    );
    let wopi_access_token = launch["data"]["access_token"].as_str().unwrap();

    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/files/{file_id}/lock"))
        .insert_header(("Cookie", format!("aster_access={access_cookie}")))
        .set_json(json!({ "locked": true }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::post()
        .uri(&wopi_contents_query(file_id, wopi_access_token))
        .insert_header(("Origin", TEST_WOPI_ORIGIN))
        .insert_header(("X-WOPI-Override", "PUT"))
        .insert_header(("X-WOPI-Lock", "lock-a"))
        .set_payload("should conflict")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 409);
    assert!(resp.headers().get("X-WOPI-Lock").is_none());
    assert!(
        resp.headers()
            .get("X-WOPI-LockFailureReason")
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.contains("outside WOPI"))
    );
}

#[actix_web::test]
async fn test_revoked_user_sessions_invalidate_wopi_access_token() {
    let state = common::setup().await;
    configure_test_wopi_runtime(&state);
    let db = state.db.clone();
    let app = create_test_app!(state);

    let (access_cookie, _) = register_and_login!(app);
    let user = user_repo::find_by_username(&db, "testuser")
        .await
        .unwrap()
        .expect("registered user should exist");
    let file_id = upload_test_file_named!(app, access_cookie, "report 1.docx");

    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/files/{file_id}/wopi/open"))
        .insert_header(("Cookie", format!("aster_access={access_cookie}")))
        .set_json(json!({ "app_key": TEST_WOPI_APP_KEY }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    let wopi_access_token = body["data"]["access_token"]
        .as_str()
        .expect("launch session should return access token")
        .to_string();
    let token_hash = aster_drive::utils::hash::sha256_hex(wopi_access_token.as_bytes());

    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/admin/users/{}/sessions/revoke", user.id))
        .insert_header(("Cookie", format!("aster_access={access_cookie}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::get()
        .uri(&format!(
            "/api/v1/wopi/files/{file_id}?access_token={}",
            urlencoding::encode(&wopi_access_token)
        ))
        .insert_header(("Origin", TEST_WOPI_ORIGIN))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 401);

    let body = test::read_body(resp).await;
    let message = String::from_utf8(body.to_vec()).unwrap();
    assert!(message.contains("WOPI session revoked"));
    assert!(
        wopi_session_repo::find_by_token_hash(&db, &token_hash)
            .await
            .unwrap()
            .is_none(),
        "revoked WOPI session should be cleaned up on access"
    );
}

#[actix_web::test]
async fn test_team_file_wopi_open_persists_team_scope_and_allows_check_file_info() {
    let state = common::setup().await;
    configure_test_wopi_runtime(&state);
    let db = state.db.clone();
    let mail_sender = state.mail_sender.clone();
    let app = create_test_app!(state);

    let (owner_id, owner_token) = register_named_user_and_login!(
        app,
        db,
        mail_sender,
        "wopiowner",
        "wopiowner@example.com",
        "password123"
    );
    let (member_id, member_token) = register_named_user_and_login!(
        app,
        db,
        mail_sender,
        "wopimember",
        "wopimember@example.com",
        "password123"
    );

    let req = test::TestRequest::post()
        .uri("/api/v1/teams")
        .insert_header(("Cookie", format!("aster_access={owner_token}")))
        .set_json(json!({ "name": "WOPI Team" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let team_id = body["data"]["id"].as_i64().unwrap();

    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/teams/{team_id}/members"))
        .insert_header(("Cookie", format!("aster_access={owner_token}")))
        .set_json(json!({ "user_id": member_id }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);

    let req = team_multipart_request!(
        &format!("/api/v1/teams/{team_id}/files/upload"),
        &owner_token,
        "team-report.docx",
        "team content",
    );
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let file_id = body["data"]["id"].as_i64().unwrap();

    let launch = open_wopi_session!(
        app,
        member_token,
        &format!("/api/v1/teams/{team_id}/files/{file_id}/wopi/open"),
        TEST_WOPI_APP_KEY
    );
    let wopi_access_token = launch["data"]["access_token"]
        .as_str()
        .expect("launch session should return access token")
        .to_string();
    let token_hash = aster_drive::utils::hash::sha256_hex(wopi_access_token.as_bytes());
    let stored_session = wopi_session_repo::find_by_token_hash(&db, &token_hash)
        .await
        .unwrap()
        .expect("team WOPI session should be persisted");
    assert_eq!(stored_session.actor_user_id, member_id);
    assert_eq!(stored_session.team_id, Some(team_id));
    assert_eq!(stored_session.file_id, file_id);

    let req = test::TestRequest::get()
        .uri(&wopi_file_query(file_id, &wopi_access_token))
        .insert_header(("Origin", TEST_WOPI_ORIGIN))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["BaseFileName"], "team-report.docx");
    assert_eq!(body["OwnerId"], owner_id.to_string());
    assert_eq!(body["UserId"], member_id.to_string());
}

#[actix_web::test]
async fn test_team_wopi_access_token_is_rejected_after_member_removal() {
    let state = common::setup().await;
    configure_test_wopi_runtime(&state);
    let db = state.db.clone();
    let mail_sender = state.mail_sender.clone();
    let app = create_test_app!(state);

    let (_, owner_token) = register_named_user_and_login!(
        app,
        db,
        mail_sender,
        "wopiowner3",
        "wopiowner3@example.com",
        "password123"
    );
    let (member_id, member_token) = register_named_user_and_login!(
        app,
        db,
        mail_sender,
        "wopimember3",
        "wopimember3@example.com",
        "password123"
    );

    let req = test::TestRequest::post()
        .uri("/api/v1/teams")
        .insert_header(("Cookie", format!("aster_access={owner_token}")))
        .set_json(json!({ "name": "WOPI Team 3" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let team_id = body["data"]["id"].as_i64().unwrap();

    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/teams/{team_id}/members"))
        .insert_header(("Cookie", format!("aster_access={owner_token}")))
        .set_json(json!({ "user_id": member_id }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);

    let req = team_multipart_request!(
        &format!("/api/v1/teams/{team_id}/files/upload"),
        &owner_token,
        "team-report.docx",
        "team content",
    );
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let file_id = body["data"]["id"].as_i64().unwrap();

    let launch = open_wopi_session!(
        app,
        member_token,
        &format!("/api/v1/teams/{team_id}/files/{file_id}/wopi/open"),
        TEST_WOPI_APP_KEY
    );
    let wopi_access_token = launch["data"]["access_token"].as_str().unwrap().to_string();

    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/teams/{team_id}/members/{member_id}"))
        .insert_header(("Cookie", format!("aster_access={owner_token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::get()
        .uri(&wopi_file_query(file_id, &wopi_access_token))
        .insert_header(("Origin", TEST_WOPI_ORIGIN))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 401);
}

#[actix_web::test]
async fn test_team_file_wopi_open_rejects_non_member() {
    let state = common::setup().await;
    configure_test_wopi_runtime(&state);
    let db = state.db.clone();
    let mail_sender = state.mail_sender.clone();
    let app = create_test_app!(state);

    let (_, owner_token) = register_named_user_and_login!(
        app,
        db,
        mail_sender,
        "wopiowner2",
        "wopiowner2@example.com",
        "password123"
    );
    let (_, outsider_token) = register_named_user_and_login!(
        app,
        db,
        mail_sender,
        "wopioutsider",
        "wopioutsider@example.com",
        "password123"
    );

    let req = test::TestRequest::post()
        .uri("/api/v1/teams")
        .insert_header(("Cookie", format!("aster_access={owner_token}")))
        .set_json(json!({ "name": "WOPI Team 2" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let team_id = body["data"]["id"].as_i64().unwrap();

    let req = team_multipart_request!(
        &format!("/api/v1/teams/{team_id}/files/upload"),
        &owner_token,
        "team-report.docx",
        "team content",
    );
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let file_id = body["data"]["id"].as_i64().unwrap();

    let req = test::TestRequest::post()
        .uri(&format!(
            "/api/v1/teams/{team_id}/files/{file_id}/wopi/open"
        ))
        .insert_header(("Cookie", format!("aster_access={outsider_token}")))
        .set_json(json!({ "app_key": TEST_WOPI_APP_KEY }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 403);
}
