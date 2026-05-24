mod common;

use actix_web::{body::MessageBody, http::StatusCode, test};
use aster_drive::services::mfa_service::totp;
use chrono::{Duration, Utc};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, Set};
use serde_json::Value;

async fn register_user<S, B, E>(app: &S, username: &str, email: &str, password: &str)
where
    S: actix_web::dev::Service<
            actix_http::Request,
            Response = actix_web::dev::ServiceResponse<B>,
            Error = E,
        >,
    B: MessageBody,
    E: std::fmt::Debug,
{
    let req = test::TestRequest::post()
        .uri("/api/v1/auth/register")
        .set_json(serde_json::json!({
            "username": username,
            "email": email,
            "password": password
        }))
        .to_request();
    let resp = test::call_service(app, req).await;
    assert_eq!(resp.status(), StatusCode::CREATED);
}

async fn login_raw<S, B, E>(
    app: &S,
    identifier: &str,
    password: &str,
) -> actix_web::dev::ServiceResponse<B>
where
    S: actix_web::dev::Service<
            actix_http::Request,
            Response = actix_web::dev::ServiceResponse<B>,
            Error = E,
        >,
    B: MessageBody,
    E: std::fmt::Debug,
{
    let req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .set_json(serde_json::json!({
            "identifier": identifier,
            "password": password
        }))
        .to_request();
    test::call_service(app, req).await
}

async fn enable_totp<S, B, E>(app: &S, access: &str) -> (i64, String, Vec<String>)
where
    S: actix_web::dev::Service<
            actix_http::Request,
            Response = actix_web::dev::ServiceResponse<B>,
            Error = E,
        >,
    B: MessageBody,
    E: std::fmt::Debug,
{
    let req = test::TestRequest::post()
        .uri("/api/v1/auth/mfa/totp/setup/start")
        .insert_header(("Cookie", common::access_cookie_header(access)))
        .insert_header(common::csrf_header_for(access))
        .to_request();
    let resp = test::call_service(app, req).await;
    let status = resp.status();
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(status, StatusCode::OK, "{body:#?}");
    let flow_token = body["data"]["flow_token"].as_str().unwrap().to_string();
    let secret = body["data"]["secret"].as_str().unwrap().to_string();
    let code = totp_code(&secret);

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/mfa/totp/setup/finish")
        .insert_header(("Cookie", common::access_cookie_header(access)))
        .insert_header(common::csrf_header_for(access))
        .set_json(serde_json::json!({
            "flow_token": flow_token,
            "code": code,
            "name": "Phone"
        }))
        .to_request();
    let resp = test::call_service(app, req).await;
    let status = resp.status();
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(status, StatusCode::OK, "{body:#?}");
    let factor_id = body["data"]["factor"]["id"].as_i64().unwrap();
    let recovery_codes = body["data"]["recovery_codes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap().to_string())
        .collect();
    (factor_id, secret, recovery_codes)
}

async fn verify_mfa<S, B, E>(
    app: &S,
    flow_token: &str,
    method: &str,
    code: &str,
) -> actix_web::dev::ServiceResponse<B>
where
    S: actix_web::dev::Service<
            actix_http::Request,
            Response = actix_web::dev::ServiceResponse<B>,
            Error = E,
        >,
    E: std::fmt::Debug,
{
    let req = test::TestRequest::post()
        .uri("/api/v1/auth/mfa/challenge/verify")
        .set_json(serde_json::json!({
            "flow_token": flow_token,
            "method": method,
            "code": code
        }))
        .to_request();
    test::call_service(app, req).await
}

fn totp_code(secret: &str) -> String {
    let secret_bytes = totp::decode_secret(secret).unwrap();
    totp::code_for_time(&secret_bytes, chrono::Utc::now()).unwrap()
}

#[tokio::test]
async fn test_password_login_without_mfa_still_sets_cookies() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    register_user(&app, "plainuser", "plain@example.com", "password123").await;

    let resp = login_raw(&app, "plainuser", "password123").await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(common::extract_cookie(&resp, "aster_access").is_some());
    assert!(common::extract_cookie(&resp, "aster_refresh").is_some());
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["status"], "authenticated");
}

#[tokio::test]
async fn test_password_login_requires_mfa_without_setting_cookies_then_totp_completes() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    register_user(&app, "mfauser", "mfa@example.com", "password123").await;
    let (access, _) = login_user!(app, "mfauser", "password123");
    let (_factor_id, secret, _recovery_codes) = enable_totp(&app, &access).await;

    let resp = login_raw(&app, "mfauser", "password123").await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(common::extract_cookie(&resp, "aster_access").is_none());
    assert!(common::extract_cookie(&resp, "aster_refresh").is_none());
    assert!(common::extract_cookie(&resp, "aster_csrf").is_none());
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["status"], "mfa_required");
    let flow_token = body["data"]["flow_token"].as_str().unwrap().to_string();
    assert_eq!(body["data"]["methods"][0], "totp");

    let code = totp_code(&secret);
    let resp = verify_mfa(&app, &flow_token, "totp", &code).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(common::extract_cookie(&resp, "aster_access").is_some());
    assert!(common::extract_cookie(&resp, "aster_refresh").is_some());
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["status"], "authenticated");
}

#[tokio::test]
async fn test_mfa_wrong_totp_attempts_eventually_consume_flow() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    register_user(&app, "attempts", "attempts@example.com", "password123").await;
    let (access, _) = login_user!(app, "attempts", "password123");
    let _ = enable_totp(&app, &access).await;

    let resp = login_raw(&app, "attempts", "password123").await;
    let body: Value = test::read_body_json(resp).await;
    let flow_token = body["data"]["flow_token"].as_str().unwrap().to_string();

    for index in 0..5 {
        let resp = verify_mfa(&app, &flow_token, "totp", "000000").await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        let body: Value = test::read_body_json(resp).await;
        if index < 4 {
            assert_eq!(body["error"]["subcode"], "auth.mfa_code_invalid");
        } else {
            assert_eq!(body["error"]["subcode"], "auth.mfa_attempts_exceeded");
        }
    }
}

#[tokio::test]
async fn test_expired_mfa_flow_cannot_login() {
    let state = common::setup().await;
    let db = state.writer_db().clone();
    let app = create_test_app!(state);
    register_user(&app, "expiredmfa", "expiredmfa@example.com", "password123").await;
    let (access, _) = login_user!(app, "expiredmfa", "password123");
    let _ = enable_totp(&app, &access).await;

    let resp = login_raw(&app, "expiredmfa", "password123").await;
    let body: Value = test::read_body_json(resp).await;
    let flow_token = body["data"]["flow_token"].as_str().unwrap().to_string();
    let flow_hash = aster_drive::utils::hash::sha256_hex(flow_token.as_bytes());
    let flow = aster_drive::entities::mfa_login_flow::Entity::find()
        .filter(aster_drive::entities::mfa_login_flow::Column::FlowTokenHash.eq(flow_hash))
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    let mut active = flow.into_active_model();
    active.expires_at = Set(Utc::now() - Duration::seconds(1));
    active.update(&db).await.unwrap();

    let resp = verify_mfa(&app, &flow_token, "totp", "123456").await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert!(common::extract_cookie(&resp, "aster_access").is_none());
    assert!(common::extract_cookie(&resp, "aster_refresh").is_none());
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["error"]["subcode"], "auth.mfa_flow_expired");
}

#[tokio::test]
async fn test_recovery_code_can_be_used_once() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    register_user(&app, "recovery", "recovery@example.com", "password123").await;
    let (access, _) = login_user!(app, "recovery", "password123");
    let (_factor_id, _secret, recovery_codes) = enable_totp(&app, &access).await;
    let recovery_code = recovery_codes.first().unwrap().clone();

    let resp = login_raw(&app, "recovery", "password123").await;
    let body: Value = test::read_body_json(resp).await;
    let flow_token = body["data"]["flow_token"].as_str().unwrap().to_string();
    let resp = verify_mfa(&app, &flow_token, "recovery_code", &recovery_code).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let resp = login_raw(&app, "recovery", "password123").await;
    let body: Value = test::read_body_json(resp).await;
    let second_flow = body["data"]["flow_token"].as_str().unwrap().to_string();
    let resp = verify_mfa(&app, &second_flow, "recovery_code", &recovery_code).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_totp_method_with_recovery_code_returns_mfa_code_invalid() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    register_user(
        &app,
        "wrongmethod",
        "wrongmethod@example.com",
        "password123",
    )
    .await;
    let (access, _) = login_user!(app, "wrongmethod", "password123");
    let (_factor_id, _secret, recovery_codes) = enable_totp(&app, &access).await;
    let recovery_code = recovery_codes.first().unwrap().clone();

    let resp = login_raw(&app, "wrongmethod", "password123").await;
    let body: Value = test::read_body_json(resp).await;
    let flow_token = body["data"]["flow_token"].as_str().unwrap().to_string();
    let resp = verify_mfa(&app, &flow_token, "totp", &recovery_code).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["error"]["subcode"], "auth.mfa_code_invalid");
    assert_ne!(body["msg"], "TOTP code must be a 6 digit number");
}

#[tokio::test]
async fn test_regenerate_recovery_codes_accepts_recovery_code_without_password() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    register_user(&app, "regen", "regen@example.com", "password123").await;
    let (access, _) = login_user!(app, "regen", "password123");
    let (_factor_id, _secret, recovery_codes) = enable_totp(&app, &access).await;
    let recovery_code = recovery_codes.first().unwrap().clone();

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/mfa/recovery-codes/regenerate")
        .insert_header(("Cookie", common::access_cookie_header(&access)))
        .insert_header(common::csrf_header_for(&access))
        .set_json(serde_json::json!({ "code": recovery_code }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let status = resp.status();
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(status, StatusCode::OK, "{body:#?}");
    assert_eq!(body["data"].as_array().unwrap().len(), 10);
}

#[tokio::test]
async fn test_regenerate_recovery_codes_wrong_code_returns_mfa_subcode() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    register_user(&app, "regenbad", "regenbad@example.com", "password123").await;
    let (access, _) = login_user!(app, "regenbad", "password123");
    let _ = enable_totp(&app, &access).await;

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/mfa/recovery-codes/regenerate")
        .insert_header(("Cookie", common::access_cookie_header(&access)))
        .insert_header(common::csrf_header_for(&access))
        .set_json(serde_json::json!({ "code": "000000" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let status = resp.status();
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body:#?}");
    assert_eq!(body["error"]["internal_code"], "E010");
    assert_eq!(body["error"]["subcode"], "auth.mfa_code_invalid");
}

#[tokio::test]
async fn test_delete_mfa_factor_accepts_totp_without_password() {
    let state = common::setup().await;
    let db = state.writer_db().clone();
    let app = create_test_app!(state);
    register_user(&app, "disablemfa", "disablemfa@example.com", "password123").await;
    let (access, _) = login_user!(app, "disablemfa", "password123");
    let (factor_id, secret, _recovery_codes) = enable_totp(&app, &access).await;

    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/auth/mfa/factors/{factor_id}"))
        .insert_header(("Cookie", common::access_cookie_header(&access)))
        .insert_header(common::csrf_header_for(&access))
        .set_json(serde_json::json!({ "code": totp_code(&secret) }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let status = resp.status();
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(status, StatusCode::OK, "{body:#?}");

    let factors = aster_drive::entities::mfa_factor::Entity::find()
        .filter(aster_drive::entities::mfa_factor::Column::Id.eq(factor_id))
        .all(&db)
        .await
        .unwrap();
    assert!(factors.is_empty());
}

#[tokio::test]
async fn test_old_mfa_flow_cannot_login_after_password_reset() {
    let state = common::setup().await;
    let db = state.writer_db().clone();
    let app = create_test_app!(state);
    let (admin_access, _) = register_and_login!(app);
    let stale_user_id = admin_create_user!(
        app,
        &admin_access,
        "stale2",
        "stale2@example.com",
        "password123"
    );
    let (access, _) = login_user!(app, "stale2", "password123");
    let (_factor_id, secret, _codes) = enable_totp(&app, &access).await;

    let resp = login_raw(&app, "stale2", "password123").await;
    let body: Value = test::read_body_json(resp).await;
    let flow_token = body["data"]["flow_token"].as_str().unwrap().to_string();

    let req = test::TestRequest::put()
        .uri(&format!("/api/v1/admin/users/{stale_user_id}/password"))
        .insert_header(("Cookie", common::access_cookie_header(&admin_access)))
        .insert_header(common::csrf_header_for(&admin_access))
        .set_json(serde_json::json!({ "password": "newpassword123" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let code = totp_code(&secret);
    let resp = verify_mfa(&app, &flow_token, "totp", &code).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["error"]["subcode"], "auth.mfa_flow_invalid");

    let active_sessions = aster_drive::entities::auth_session::Entity::find()
        .filter(aster_drive::entities::auth_session::Column::UserId.eq(stale_user_id))
        .all(&db)
        .await
        .unwrap();
    assert!(active_sessions.is_empty());
}

#[tokio::test]
async fn test_disabled_user_cannot_exchange_existing_mfa_flow() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (admin_access, _) = register_and_login!(app);
    let user_id = admin_create_user!(
        app,
        &admin_access,
        "disabledmfa",
        "disabledmfa@example.com",
        "password123"
    );
    let (access, _) = login_user!(app, "disabledmfa", "password123");
    let (_factor_id, secret, _codes) = enable_totp(&app, &access).await;

    let resp = login_raw(&app, "disabledmfa", "password123").await;
    let body: Value = test::read_body_json(resp).await;
    let flow_token = body["data"]["flow_token"].as_str().unwrap().to_string();

    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/admin/users/{user_id}"))
        .insert_header(("Cookie", common::access_cookie_header(&admin_access)))
        .insert_header(common::csrf_header_for(&admin_access))
        .set_json(serde_json::json!({ "status": "disabled" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let code = totp_code(&secret);
    let resp = verify_mfa(&app, &flow_token, "totp", &code).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    assert!(common::extract_cookie(&resp, "aster_access").is_none());
    assert!(common::extract_cookie(&resp, "aster_refresh").is_none());
}

#[tokio::test]
async fn test_admin_reset_mfa_clears_factors_and_revokes_sessions() {
    let state = common::setup().await;
    let db = state.writer_db().clone();
    let app = create_test_app!(state);
    let (admin_access, _) = register_and_login!(app);
    let user_id = admin_create_user!(
        app,
        &admin_access,
        "resetmfa",
        "resetmfa@example.com",
        "password123"
    );
    let (access, _) = login_user!(app, "resetmfa", "password123");
    let _ = enable_totp(&app, &access).await;

    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/admin/users/{user_id}/mfa"))
        .insert_header(("Cookie", common::access_cookie_header(&admin_access)))
        .insert_header(common::csrf_header_for(&admin_access))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let factors = aster_drive::entities::mfa_factor::Entity::find()
        .filter(aster_drive::entities::mfa_factor::Column::UserId.eq(user_id))
        .all(&db)
        .await
        .unwrap();
    assert!(factors.is_empty());

    let resp = login_raw(&app, "resetmfa", "password123").await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(common::extract_cookie(&resp, "aster_access").is_some());
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["status"], "authenticated");
}
