#[macro_use]
mod common;

use actix_web::body::MessageBody;
use actix_web::cookie::SameSite;
use actix_web::test;
use serde_json::Value;
use std::io::Cursor;
use std::time::Duration;

macro_rules! login_user_with_credentials {
    ($app:expr, $identifier:expr, $password:expr) => {{
        let req = test::TestRequest::post()
            .uri("/api/v1/auth/login")
            .peer_addr("127.0.0.1:12345".parse().unwrap())
            .set_json(serde_json::json!({
                "identifier": $identifier,
                "password": $password
            }))
            .to_request();
        let resp = test::call_service(&$app, req).await;
        assert_eq!(resp.status(), 200);
        common::extract_cookie(&resp, "aster_access").unwrap()
    }};
}

macro_rules! admin_create_user_with_credentials {
    ($app:expr, $admin_token:expr, $username:expr, $email:expr, $password:expr) => {{
        let req = test::TestRequest::post()
            .uri("/api/v1/admin/users")
            .insert_header(("Cookie", format!("aster_access={}", $admin_token)))
            .peer_addr("127.0.0.1:12345".parse().unwrap())
            .set_json(serde_json::json!({
                "username": $username,
                "email": $email,
                "password": $password
            }))
            .to_request();
        let resp = test::call_service(&$app, req).await;
        assert_eq!(resp.status(), 201);
        let body: Value = test::read_body_json(resp).await;
        body["data"]["id"].as_i64().unwrap()
    }};
}

macro_rules! team_upload_request {
    ($team_id:expr, $token:expr, $filename:expr, $content:expr $(,)?) => {{
        let boundary = "----TeamStorageEventBoundary";
        let payload = format!(
            "------TeamStorageEventBoundary\r\n\
             Content-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\n\
             Content-Type: text/plain\r\n\r\n\
             {content}\r\n\
             ------TeamStorageEventBoundary--\r\n",
            filename = $filename,
            content = $content,
        );

        test::TestRequest::post()
            .uri(&format!("/api/v1/teams/{}/files/upload", $team_id))
            .insert_header(("Cookie", format!("aster_access={}", $token)))
            .insert_header((
                "Content-Type",
                format!("multipart/form-data; boundary={boundary}"),
            ))
            .set_payload(payload)
            .to_request()
    }};
}

fn avatar_upload_payload() -> (String, Vec<u8>) {
    let boundary = "----AsterAvatarBoundary".to_string();
    let image = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        8,
        8,
        image::Rgba([255, 120, 0, 255]),
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

fn extract_verification_token(
    message: &aster_drive::services::mail_service::MailMessage,
) -> String {
    common::extract_token_from_mail_message(
        message,
        "/api/v1/auth/contact-verification/confirm?token=",
    )
    .expect("verification link missing from mail body")
}

fn extract_password_reset_token(
    message: &aster_drive::services::mail_service::MailMessage,
) -> String {
    common::extract_token_from_mail_message(message, "/reset-password?token=")
        .expect("password reset link missing from mail body")
}

async fn read_next_sse_json<B>(body: &mut B) -> Value
where
    B: MessageBody + Unpin,
    B::Error: std::fmt::Debug,
{
    for _ in 0..4 {
        let frame = tokio::time::timeout(
            Duration::from_secs(2),
            std::future::poll_fn(|cx| std::pin::Pin::new(&mut *body).poll_next(cx)),
        )
        .await
        .expect("timed out waiting for SSE frame")
        .expect("SSE stream ended unexpectedly")
        .expect("SSE body chunk should not fail");

        let text = std::str::from_utf8(&frame).expect("SSE frame should be utf-8");
        for chunk in text.split("\n\n") {
            if let Some(json) = chunk.strip_prefix("data: ") {
                return serde_json::from_str(json).expect("SSE data should be valid JSON");
            }
        }
    }

    panic!("did not receive SSE data frame");
}

async fn read_next_sse_json_with_timeout<B>(body: &mut B, timeout: Duration) -> Option<Value>
where
    B: MessageBody + Unpin,
    B::Error: std::fmt::Debug,
{
    let deadline = tokio::time::Instant::now() + timeout;

    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return None;
        }

        let frame = match tokio::time::timeout(
            remaining,
            std::future::poll_fn(|cx| std::pin::Pin::new(&mut *body).poll_next(cx)),
        )
        .await
        {
            Ok(frame) => frame
                .expect("SSE stream ended unexpectedly")
                .expect("SSE body chunk should not fail"),
            Err(_) => return None,
        };

        let text = std::str::from_utf8(&frame).expect("SSE frame should be utf-8");
        for chunk in text.split("\n\n") {
            if let Some(json) = chunk.strip_prefix("data: ") {
                return Some(serde_json::from_str(json).expect("SSE data should be valid JSON"));
            }
        }
    }
}

#[actix_web::test]
async fn test_register_and_login() {
    let state = common::setup().await;
    let app = create_test_app!(state);

    // 注册
    let req = test::TestRequest::post()
        .uri("/api/v1/auth/register")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "username": "alice",
            "email": "alice@example.com",
            "password": "secret123"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["code"], 0);
    assert_eq!(body["data"]["username"], "alice");
    // password_hash 不应该暴露
    assert!(body["data"]["password_hash"].is_null());

    // 重复注册应失败
    let req = test::TestRequest::post()
        .uri("/api/v1/auth/register")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "username": "alice",
            "email": "alice2@example.com",
            "password": "secret123"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);

    // 登录
    let req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "identifier": "alice",
            "password": "secret123"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let access = common::extract_cookie(&resp, "aster_access");
    let refresh = common::extract_cookie(&resp, "aster_refresh");
    let access_cookie_path = resp
        .response()
        .cookies()
        .find(|cookie| cookie.name() == "aster_access")
        .expect("access cookie missing")
        .path()
        .map(str::to_string);
    let access_cookie_same_site = resp
        .response()
        .cookies()
        .find(|cookie| cookie.name() == "aster_access")
        .expect("access cookie missing")
        .same_site();
    let refresh_cookie_path = resp
        .response()
        .cookies()
        .find(|cookie| cookie.name() == "aster_refresh")
        .expect("refresh cookie missing")
        .path()
        .map(str::to_string);
    let refresh_cookie_same_site = resp
        .response()
        .cookies()
        .find(|cookie| cookie.name() == "aster_refresh")
        .expect("refresh cookie missing")
        .same_site();
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["code"], 0);
    assert_eq!(body["data"]["expires_in"], 900);
    // tokens 在 cookie 里
    assert!(access.is_some());
    assert!(refresh.is_some());
    assert_eq!(access_cookie_path.as_deref(), Some("/"));
    assert_eq!(access_cookie_same_site, Some(SameSite::Lax));
    assert_eq!(refresh_cookie_path.as_deref(), Some("/api/v1/auth/refresh"));
    assert_eq!(refresh_cookie_same_site, Some(SameSite::Lax));

    // 错误密码
    let req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "identifier": "alice",
            "password": "wrongpassword"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 401);
}

#[actix_web::test]
async fn test_setup_still_works_when_public_registration_is_disabled() {
    let state = common::setup().await;
    state.runtime_config.apply(common::system_config_model(
        aster_drive::config::auth_runtime::AUTH_ALLOW_USER_REGISTRATION_KEY,
        "false",
    ));
    let app = create_test_app!(state);

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/setup")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "username": "adminuser",
            "email": "admin@example.com",
            "password": "secret123"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
}

#[actix_web::test]
async fn test_check_reports_public_registration_flag() {
    let state = common::setup().await;
    state.runtime_config.apply(common::system_config_model(
        aster_drive::config::auth_runtime::AUTH_ALLOW_USER_REGISTRATION_KEY,
        "false",
    ));
    let app = create_test_app!(state);

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/setup")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "username": "adminuser",
            "email": "admin@example.com",
            "password": "secret123"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/check")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["has_users"], true);
    assert_eq!(body["data"]["allow_user_registration"], false);
}

#[actix_web::test]
async fn test_register_is_blocked_when_public_registration_is_disabled() {
    let state = common::setup().await;
    state.runtime_config.apply(common::system_config_model(
        aster_drive::config::auth_runtime::AUTH_ALLOW_USER_REGISTRATION_KEY,
        "false",
    ));
    let app = create_test_app!(state);

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/setup")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "username": "adminuser",
            "email": "admin@example.com",
            "password": "secret123"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/register")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "username": "blockeduser",
            "email": "blocked@example.com",
            "password": "secret123"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 403);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["msg"], "new user registration is disabled");
}

#[actix_web::test]
async fn test_register_requires_activation_until_confirmed() {
    let state = common::setup().await;
    let db = state.db.clone();
    let mail_sender = state.mail_sender.clone();
    let runtime_config = state.runtime_config.clone();
    let app = create_test_app!(state);

    let _ = register_and_login!(app);

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/register")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "username": "pendinguser",
            "email": "pendinguser@example.com",
            "password": "password123"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    common::flush_mail_outbox_with(&db, &runtime_config, &mail_sender).await;

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "identifier": "pendinguser",
            "password": "password123"
        }))
        .to_request();
    assert_service_status!(app, req, 403);

    let memory_sender = aster_drive::services::mail_service::memory_sender_ref(&mail_sender)
        .expect("memory mail sender should be available in tests");
    let token = extract_verification_token(
        &memory_sender
            .last_message()
            .expect("activation email should be sent"),
    );

    let req = test::TestRequest::get()
        .uri(&format!(
            "/api/v1/auth/contact-verification/confirm?token={}",
            urlencoding::encode(&token)
        ))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 302);
    common::flush_mail_outbox_with(&db, &runtime_config, &mail_sender).await;
    let location = resp
        .headers()
        .get("Location")
        .and_then(|value| value.to_str().ok())
        .expect("contact verification redirect location missing");
    assert_eq!(location, "/login?contact_verification=register-activated");

    let (_access, _refresh) = login_user!(app, "pendinguser", "password123");
}

#[actix_web::test]
async fn test_register_skips_activation_when_register_activation_is_disabled() {
    let state = common::setup().await;
    let db = state.db.clone();
    let mail_sender = state.mail_sender.clone();
    let runtime_config = state.runtime_config.clone();
    state.runtime_config.apply(common::system_config_model(
        aster_drive::config::auth_runtime::AUTH_REGISTER_ACTIVATION_ENABLED_KEY,
        "false",
    ));
    let app = create_test_app!(state);

    let _ = register_and_login!(app);

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/register")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "username": "directuser",
            "email": "directuser@example.com",
            "password": "password123"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["email_verified"], true);

    common::flush_mail_outbox_with(&db, &runtime_config, &mail_sender).await;
    let memory_sender = aster_drive::services::mail_service::memory_sender_ref(&mail_sender)
        .expect("memory mail sender should be available in tests");
    assert!(memory_sender.messages().is_empty());

    let (_access, _refresh) = login_user!(app, "directuser", "password123");
}

#[actix_web::test]
async fn test_register_resend_is_generic_for_unknown_identifier_and_cooldown() {
    let state = common::setup().await;
    let db = state.db.clone();
    let mail_sender = state.mail_sender.clone();
    let runtime_config = state.runtime_config.clone();
    let app = create_test_app!(state);

    let _ = register_and_login!(app);

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/register")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "username": "pendinguser",
            "email": "pendinguser@example.com",
            "password": "password123"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    common::flush_mail_outbox_with(&db, &runtime_config, &mail_sender).await;

    let resend = |identifier: &str| {
        test::TestRequest::post()
            .uri("/api/v1/auth/register/resend")
            .peer_addr("127.0.0.1:12345".parse().unwrap())
            .set_json(serde_json::json!({ "identifier": identifier }))
            .to_request()
    };

    let resp = test::call_service(&app, resend("missing@example.com")).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(
        body["data"]["message"],
        "If the account can be reactivated, an activation email will be sent"
    );

    let resp = test::call_service(&app, resend("pendinguser")).await;
    assert_eq!(resp.status(), 200);
    common::flush_mail_outbox_with(&db, &runtime_config, &mail_sender).await;

    let memory_sender = aster_drive::services::mail_service::memory_sender_ref(&mail_sender)
        .expect("memory mail sender should be available in tests");
    assert_eq!(memory_sender.messages().len(), 1);
}

#[actix_web::test]
async fn test_email_change_confirmation_redirects_and_notifies_previous_email() {
    let state = common::setup().await;
    let db = state.db.clone();
    let mail_sender = state.mail_sender.clone();
    let runtime_config = state.runtime_config.clone();
    let app = create_test_app!(state);

    let (access, _refresh) = register_and_login!(app);

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/email/change")
        .insert_header(("Cookie", format!("aster_access={access}")))
        .set_json(serde_json::json!({
            "new_email": "updated@example.com"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    common::flush_mail_outbox_with(&db, &runtime_config, &mail_sender).await;

    let memory_sender = aster_drive::services::mail_service::memory_sender_ref(&mail_sender)
        .expect("memory mail sender should be available in tests");
    let token = extract_verification_token(
        &memory_sender
            .last_message()
            .expect("email change confirmation should be sent"),
    );

    let req = test::TestRequest::get()
        .uri(&format!(
            "/api/v1/auth/contact-verification/confirm?token={}",
            urlencoding::encode(&token)
        ))
        .insert_header(("Cookie", format!("aster_access={access}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 302);
    common::flush_mail_outbox_with(&db, &runtime_config, &mail_sender).await;
    let location = resp
        .headers()
        .get("Location")
        .and_then(|value| value.to_str().ok())
        .expect("contact verification redirect location missing");
    assert_eq!(
        location,
        "/settings/security?contact_verification=email-changed&email=updated%40example.com"
    );

    let messages = memory_sender.messages();
    let previous_email_notice = messages
        .last()
        .expect("email change notice should be sent to previous address");
    assert_eq!(previous_email_notice.to.address, "test@example.com");
    assert_eq!(
        previous_email_notice.subject,
        aster_drive::config::mail::default_template_subject(
            aster_drive::types::MailTemplateCode::ContactChangeNotice,
        )
    );
    assert!(
        previous_email_notice
            .text_body
            .contains("updated@example.com")
    );
}

#[actix_web::test]
async fn test_email_change_resend_returns_generic_success_during_cooldown() {
    let state = common::setup().await;
    let db = state.db.clone();
    let mail_sender = state.mail_sender.clone();
    let runtime_config = state.runtime_config.clone();
    let app = create_test_app!(state);

    let (access, _refresh) = register_and_login!(app);

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/email/change")
        .insert_header(("Cookie", format!("aster_access={access}")))
        .set_json(serde_json::json!({
            "new_email": "updated@example.com"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    common::flush_mail_outbox_with(&db, &runtime_config, &mail_sender).await;

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/email/change/resend")
        .insert_header(("Cookie", format!("aster_access={access}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(
        body["data"]["message"],
        "If an email change is pending, a confirmation email will be sent"
    );
    common::flush_mail_outbox_with(&db, &runtime_config, &mail_sender).await;

    let memory_sender = aster_drive::services::mail_service::memory_sender_ref(&mail_sender)
        .expect("memory mail sender should be available in tests");
    assert_eq!(memory_sender.messages().len(), 1);
}

#[actix_web::test]
async fn test_password_reset_request_is_generic_for_unknown_email() {
    let state = common::setup().await;
    let mail_sender = state.mail_sender.clone();
    let app = create_test_app!(state);

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/password/reset/request")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "email": "missing@example.com"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["code"], 0);

    let memory_sender = aster_drive::services::mail_service::memory_sender_ref(&mail_sender)
        .expect("memory mail sender should be available in tests");
    assert!(memory_sender.messages().is_empty());
}

#[actix_web::test]
async fn test_password_reset_rotates_session_and_sends_notice_and_records_audit_logs() {
    use aster_drive::entities::audit_log;
    use sea_orm::EntityTrait;

    let state = common::setup().await;
    let db = state.db.clone();
    let mail_sender = state.mail_sender.clone();
    let runtime_config = state.runtime_config.clone();
    let app = create_test_app!(state);
    let (access, refresh) = register_and_login!(app);

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/password/reset/request")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "email": "test@example.com"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    common::flush_mail_outbox_with(&db, &runtime_config, &mail_sender).await;

    let memory_sender = aster_drive::services::mail_service::memory_sender_ref(&mail_sender)
        .expect("memory mail sender should be available in tests");
    let token = extract_password_reset_token(
        &memory_sender
            .last_message()
            .expect("password reset email should be sent"),
    );

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/password/reset/confirm")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "token": token,
            "new_password": "newsecret456"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    common::flush_mail_outbox_with(&db, &runtime_config, &mail_sender).await;

    let messages = memory_sender.messages();
    let password_reset_notice = messages
        .last()
        .expect("password reset notice should be sent after confirmation");
    assert_eq!(password_reset_notice.to.address, "test@example.com");
    assert_eq!(
        password_reset_notice.subject,
        aster_drive::config::mail::default_template_subject(
            aster_drive::types::MailTemplateCode::PasswordResetNotice,
        )
    );
    assert!(
        password_reset_notice
            .text_body
            .contains("Your password was reset")
    );
    assert!(
        password_reset_notice
            .text_body
            .contains("If you did not reset your password")
    );

    let req = test::TestRequest::get()
        .uri("/api/v1/auth/me")
        .insert_header(("Cookie", format!("aster_access={access}")))
        .to_request();
    assert_service_status!(app, req, 401);

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/refresh")
        .insert_header(("Cookie", format!("aster_refresh={refresh}")))
        .to_request();
    assert_service_status!(app, req, 401);

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "identifier": "testuser",
            "password": "password123"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 401);

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "identifier": "testuser",
            "password": "newsecret456"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let actions: Vec<String> = audit_log::Entity::find()
        .all(&db)
        .await
        .unwrap()
        .into_iter()
        .map(|entry| entry.action)
        .collect();
    assert!(actions.contains(&"user_request_password_reset".to_string()));
    assert!(actions.contains(&"user_confirm_password_reset".to_string()));
}

#[actix_web::test]
async fn test_password_reset_confirm_rejects_reused_token() {
    let state = common::setup().await;
    let db = state.db.clone();
    let mail_sender = state.mail_sender.clone();
    let runtime_config = state.runtime_config.clone();
    let app = create_test_app!(state);
    let _ = register_and_login!(app);

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/password/reset/request")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "email": "test@example.com"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    common::flush_mail_outbox_with(&db, &runtime_config, &mail_sender).await;

    let memory_sender = aster_drive::services::mail_service::memory_sender_ref(&mail_sender)
        .expect("memory mail sender should be available in tests");
    let token = extract_password_reset_token(
        &memory_sender
            .last_message()
            .expect("password reset email should be sent"),
    );

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/password/reset/confirm")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "token": token,
            "new_password": "newsecret456"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/password/reset/confirm")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "token": token,
            "new_password": "anothersecret789"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["code"], 2005);
}

#[actix_web::test]
async fn test_password_reset_confirm_rejects_expired_token() {
    use aster_drive::entities::contact_verification_token;
    use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, Set};

    let state = common::setup().await;
    let db = state.db.clone();
    let mail_sender = state.mail_sender.clone();
    let runtime_config = state.runtime_config.clone();
    let app = create_test_app!(state);
    let _ = register_and_login!(app);

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/password/reset/request")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "email": "test@example.com"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    common::flush_mail_outbox_with(&db, &runtime_config, &mail_sender).await;

    let memory_sender = aster_drive::services::mail_service::memory_sender_ref(&mail_sender)
        .expect("memory mail sender should be available in tests");
    let token = extract_password_reset_token(
        &memory_sender
            .last_message()
            .expect("password reset email should be sent"),
    );

    let token_hash = aster_drive::utils::hash::sha256_hex(token.as_bytes());
    let record = contact_verification_token::Entity::find()
        .filter(contact_verification_token::Column::TokenHash.eq(token_hash))
        .one(&db)
        .await
        .unwrap()
        .expect("password reset token record should exist");
    let mut active = record.into_active_model();
    active.expires_at = Set(chrono::Utc::now() - chrono::Duration::seconds(1));
    active.update(&db).await.unwrap();

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/password/reset/confirm")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "token": token,
            "new_password": "newsecret456"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 410);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["code"], 2006);
}

#[actix_web::test]
async fn test_password_reset_request_cooldown_returns_generic_success() {
    let state = common::setup().await;
    let db = state.db.clone();
    let mail_sender = state.mail_sender.clone();
    let runtime_config = state.runtime_config.clone();
    let app = create_test_app!(state);
    let _ = register_and_login!(app);

    let request = || {
        test::TestRequest::post()
            .uri("/api/v1/auth/password/reset/request")
            .peer_addr("127.0.0.1:12345".parse().unwrap())
            .set_json(serde_json::json!({
                "email": "test@example.com"
            }))
            .to_request()
    };

    let resp = test::call_service(&app, request()).await;
    assert_eq!(resp.status(), 200);

    let resp = test::call_service(&app, request()).await;
    assert_eq!(resp.status(), 200);
    common::flush_mail_outbox_with(&db, &runtime_config, &mail_sender).await;

    let memory_sender = aster_drive::services::mail_service::memory_sender_ref(&mail_sender)
        .expect("memory mail sender should be available in tests");
    assert_eq!(memory_sender.messages().len(), 1);
}

#[actix_web::test]
async fn test_contact_verification_tokens_allow_only_one_unconsumed_token_per_purpose() {
    use aster_drive::db::repository::{contact_verification_token_repo, user_repo};
    use aster_drive::entities::contact_verification_token;
    use aster_drive::types::{VerificationChannel, VerificationPurpose};
    use sea_orm::Set;

    let state = common::setup().await;
    let db = state.db.clone();
    let app = create_test_app!(state);
    let _ = register_and_login!(app);

    let user = user_repo::find_by_email(&db, "test@example.com")
        .await
        .unwrap()
        .expect("test user should exist");
    let now = chrono::Utc::now();

    contact_verification_token_repo::create(
        &db,
        contact_verification_token::ActiveModel {
            user_id: Set(user.id),
            channel: Set(VerificationChannel::Email),
            purpose: Set(VerificationPurpose::PasswordReset),
            target: Set(user.email.clone()),
            token_hash: Set("token-hash-1".to_string()),
            expires_at: Set(now + chrono::Duration::minutes(10)),
            consumed_at: Set(None),
            created_at: Set(now),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let duplicate = contact_verification_token_repo::create(
        &db,
        contact_verification_token::ActiveModel {
            user_id: Set(user.id),
            channel: Set(VerificationChannel::Email),
            purpose: Set(VerificationPurpose::PasswordReset),
            target: Set(user.email.clone()),
            token_hash: Set("token-hash-2".to_string()),
            expires_at: Set(now + chrono::Duration::minutes(20)),
            consumed_at: Set(None),
            created_at: Set(now + chrono::Duration::seconds(1)),
            ..Default::default()
        },
    )
    .await;
    assert!(duplicate.is_err());

    let first = contact_verification_token_repo::find_latest_active_for_user(
        &db,
        user.id,
        VerificationChannel::Email,
        VerificationPurpose::PasswordReset,
    )
    .await
    .unwrap()
    .expect("first token should still be active");
    contact_verification_token_repo::mark_consumed(&db, first)
        .await
        .unwrap();

    contact_verification_token_repo::create(
        &db,
        contact_verification_token::ActiveModel {
            user_id: Set(user.id),
            channel: Set(VerificationChannel::Email),
            purpose: Set(VerificationPurpose::PasswordReset),
            target: Set(user.email),
            token_hash: Set("token-hash-3".to_string()),
            expires_at: Set(now + chrono::Duration::minutes(30)),
            consumed_at: Set(None),
            created_at: Set(now + chrono::Duration::seconds(2)),
            ..Default::default()
        },
    )
    .await
    .unwrap();
}

#[actix_web::test]
async fn test_token_refresh() {
    let state = common::setup().await;
    let app = create_test_app!(state);

    let (_access, refresh) = register_and_login!(app);

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/refresh")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .insert_header(("Cookie", format!("aster_refresh={refresh}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let access = common::extract_cookie(&resp, "aster_access");
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["code"], 0);
    assert_eq!(body["data"]["expires_in"], 900);
    assert!(access.is_some());
}

#[actix_web::test]
async fn test_login_uses_runtime_auth_policy() {
    let state = common::setup().await;
    state.runtime_config.apply(common::system_config_model(
        aster_drive::config::auth_runtime::AUTH_COOKIE_SECURE_KEY,
        "true",
    ));
    state.runtime_config.apply(common::system_config_model(
        aster_drive::config::auth_runtime::AUTH_ACCESS_TOKEN_TTL_SECS_KEY,
        "120",
    ));
    state.runtime_config.apply(common::system_config_model(
        aster_drive::config::auth_runtime::AUTH_REFRESH_TOKEN_TTL_SECS_KEY,
        "3600",
    ));
    let app = create_test_app!(state);

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/register")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "username": "runtimeauth",
            "email": "runtimeauth@example.com",
            "password": "secret123"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "identifier": "runtimeauth",
            "password": "secret123"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let access_cookie_max_age = resp
        .response()
        .cookies()
        .find(|cookie| cookie.name() == "aster_access")
        .expect("access cookie missing")
        .max_age()
        .map(|duration| duration.whole_seconds());
    let refresh_cookie_max_age = resp
        .response()
        .cookies()
        .find(|cookie| cookie.name() == "aster_refresh")
        .expect("refresh cookie missing")
        .max_age()
        .map(|duration| duration.whole_seconds());
    let access_cookie_secure = resp
        .response()
        .cookies()
        .find(|cookie| cookie.name() == "aster_access")
        .expect("access cookie missing")
        .secure();
    let refresh_cookie_secure = resp
        .response()
        .cookies()
        .find(|cookie| cookie.name() == "aster_refresh")
        .expect("refresh cookie missing")
        .secure();
    let body: Value = test::read_body_json(resp).await;

    assert_eq!(body["data"]["expires_in"], 120);
    assert_eq!(access_cookie_max_age, Some(120));
    assert_eq!(refresh_cookie_max_age, Some(3600));
    assert_eq!(access_cookie_secure, Some(true));
    assert_eq!(refresh_cookie_secure, Some(true));
}

#[actix_web::test]
async fn test_refresh_token_cannot_access_protected_routes() {
    let state = common::setup().await;
    let app = create_test_app!(state);

    let (_access, refresh) = register_and_login!(app);

    let req = test::TestRequest::get()
        .uri("/api/v1/auth/me")
        .insert_header(("Cookie", format!("aster_access={refresh}")))
        .to_request();
    assert_service_status!(app, req, 401);
}

#[actix_web::test]
async fn test_storage_events_stream_receives_file_change_frames() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let req = test::TestRequest::get()
        .uri("/api/v1/auth/events/storage")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let content_type = resp
        .headers()
        .get("Content-Type")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    assert_eq!(content_type, "text/event-stream");

    let (_req, resp) = resp.into_parts();
    let mut body = resp.into_body();

    let _file_id = upload_test_file!(app, token);

    let event = read_next_sse_json(&mut body).await;
    assert_eq!(event["kind"], "file.created");
    assert_eq!(event["workspace"]["kind"], "personal");
    assert!(
        event["file_ids"]
            .as_array()
            .is_some_and(|ids| ids.len() == 1)
    );
    assert!(
        event["folder_ids"]
            .as_array()
            .is_some_and(|ids| ids.is_empty())
    );
    assert_eq!(event["root_affected"], true);
}

#[actix_web::test]
async fn test_storage_events_stream_receives_team_file_change_frames_for_member() {
    let state = common::setup().await;
    let app = create_test_app!(state);

    let (owner_token, _) = register_and_login!(app);
    let member_id = admin_create_user_with_credentials!(
        app,
        owner_token,
        "teammember",
        "teammember@example.com",
        "password123"
    );
    let member_token = login_user_with_credentials!(app, "teammember", "password123");

    let req = test::TestRequest::post()
        .uri("/api/v1/teams")
        .insert_header(("Cookie", format!("aster_access={owner_token}")))
        .set_json(serde_json::json!({ "name": "Storage Events Team" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let team_id = body["data"]["id"].as_i64().unwrap();

    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/teams/{team_id}/members"))
        .insert_header(("Cookie", format!("aster_access={owner_token}")))
        .set_json(serde_json::json!({ "user_id": member_id }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);

    let req = test::TestRequest::get()
        .uri("/api/v1/auth/events/storage")
        .insert_header(("Cookie", format!("aster_access={member_token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let (_req, resp) = resp.into_parts();
    let mut body = resp.into_body();

    let req = team_upload_request!(team_id, &owner_token, "team-event.txt", "team event");
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);

    let event = read_next_sse_json(&mut body).await;
    assert_eq!(event["kind"], "file.created");
    assert_eq!(event["workspace"]["kind"], "team");
    assert_eq!(event["workspace"]["team_id"].as_i64(), Some(team_id));
    assert!(
        event["file_ids"]
            .as_array()
            .is_some_and(|ids| ids.len() == 1)
    );
    assert_eq!(event["root_affected"], true);
}

#[actix_web::test]
async fn test_storage_events_stream_hides_team_frames_from_non_members() {
    let state = common::setup().await;
    let app = create_test_app!(state);

    let (owner_token, _) = register_and_login!(app);
    let _outsider_id = admin_create_user_with_credentials!(
        app,
        owner_token,
        "teamoutsider",
        "teamoutsider@example.com",
        "password123"
    );
    let outsider_token = login_user_with_credentials!(app, "teamoutsider", "password123");

    let req = test::TestRequest::post()
        .uri("/api/v1/teams")
        .insert_header(("Cookie", format!("aster_access={owner_token}")))
        .set_json(serde_json::json!({ "name": "Hidden Team Events" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let team_id = body["data"]["id"].as_i64().unwrap();

    let req = test::TestRequest::get()
        .uri("/api/v1/auth/events/storage")
        .insert_header(("Cookie", format!("aster_access={outsider_token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let (_req, resp) = resp.into_parts();
    let mut body = resp.into_body();

    let req = team_upload_request!(team_id, &owner_token, "hidden.txt", "hidden event");
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);

    let hidden_event = read_next_sse_json_with_timeout(&mut body, Duration::from_millis(500)).await;
    assert!(
        hidden_event.is_none(),
        "non-member should not receive team storage event: {hidden_event:?}"
    );

    let _file_id = upload_test_file_named!(app, outsider_token, "outsider-visible.txt");
    let event = read_next_sse_json(&mut body).await;
    assert_eq!(event["kind"], "file.created");
    assert_eq!(event["workspace"]["kind"], "personal");
    assert_eq!(event["root_affected"], true);
}

#[actix_web::test]
async fn test_logout_clears_cookies_without_revoking_existing_tokens() {
    let state = common::setup().await;
    let app = create_test_app!(state);

    let (access, refresh) = register_and_login!(app);

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/logout")
        .insert_header((
            "Cookie",
            format!("aster_access={access}; aster_refresh={refresh}"),
        ))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let cleared_access_path = resp
        .response()
        .cookies()
        .find(|cookie| cookie.name() == "aster_access")
        .expect("cleared access cookie missing")
        .path()
        .map(str::to_string);
    let cleared_refresh_path = resp
        .response()
        .cookies()
        .find(|cookie| cookie.name() == "aster_refresh")
        .expect("cleared refresh cookie missing")
        .path()
        .map(str::to_string);
    assert_eq!(
        common::extract_cookie(&resp, "aster_access").as_deref(),
        Some("")
    );
    assert_eq!(
        common::extract_cookie(&resp, "aster_refresh").as_deref(),
        Some("")
    );
    assert_eq!(cleared_access_path.as_deref(), Some("/"));
    assert_eq!(
        cleared_refresh_path.as_deref(),
        Some("/api/v1/auth/refresh")
    );

    let req = test::TestRequest::get()
        .uri("/api/v1/auth/me")
        .insert_header(("Cookie", format!("aster_access={access}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/refresh")
        .insert_header(("Cookie", format!("aster_refresh={refresh}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

#[actix_web::test]
async fn test_auth_me() {
    let state = common::setup().await;
    let app = create_test_app!(state);

    let (token, _) = register_and_login!(app);

    let req = test::TestRequest::get()
        .uri("/api/v1/auth/me")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["username"], "testuser");
    assert!(body["data"]["access_token_expires_at"].as_i64().unwrap() > 0);
    assert!(body["data"]["password_hash"].is_null());
    assert!(body["data"]["profile"]["display_name"].is_null());
    assert_eq!(body["data"]["profile"]["avatar"]["source"], "none");
}

/// 注册时自动分配新用户默认策略组
#[actix_web::test]
async fn test_register_auto_assigns_policy() {
    use aster_drive::db::repository::policy_group_repo;

    let state = common::setup().await;
    let expected_default_id = policy_group_repo::find_default_group(&state.db)
        .await
        .unwrap()
        .expect("default policy group should exist")
        .id;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    // 获取用户 ID
    let req = test::TestRequest::get()
        .uri("/api/v1/auth/me")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(
        body["data"]["policy_group_id"].as_i64().unwrap(),
        expected_default_id
    );
}

#[actix_web::test]
async fn test_unauthorized_access() {
    let state = common::setup().await;
    let app = create_test_app!(state);

    // 没 token 访问受保护端点
    let req = test::TestRequest::get().uri("/api/v1/folders").to_request();
    let result = test::try_call_service(&app, req).await;
    match result {
        Ok(resp) => assert_eq!(resp.status(), 401),
        Err(err) => {
            let resp = err.error_response();
            assert_eq!(resp.status(), 401);
        }
    }

    // 假 token
    let req = test::TestRequest::get()
        .uri("/api/v1/folders")
        .insert_header(("Authorization", "Bearer fake.token.here"))
        .to_request();
    assert_service_status!(app, req, 401);
}

/// 用户状态缓存：正常认证 → 连续请求不应查 DB（通过 MemoryCache 验证）
#[actix_web::test]
async fn test_user_status_cached_in_auth_middleware() {
    // 用 MemoryCache 替代默认 NoopCache
    let cache_config = aster_drive::config::CacheConfig {
        enabled: true,
        backend: "memory".to_string(),
        default_ttl: 60,
        ..Default::default()
    };
    let cache = aster_drive::cache::create_cache(&cache_config).await;

    let base = common::setup().await;
    let state = aster_drive::runtime::AppState {
        db: base.db,
        driver_registry: base.driver_registry,
        runtime_config: base.runtime_config,
        policy_snapshot: base.policy_snapshot,
        config: base.config,
        cache,
        mail_sender: base.mail_sender,
        thumbnail_tx: base.thumbnail_tx,
        storage_change_tx: base.storage_change_tx,
    };
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    // 第一次请求（cache miss → 查 DB → 写缓存）
    let req = test::TestRequest::get()
        .uri("/api/v1/folders")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // 第二次请求（cache hit → 不查 DB）—— 功能正确即可
    let req = test::TestRequest::get()
        .uri("/api/v1/folders")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

/// admin 禁用用户后，缓存立即失效，后续请求被拒
#[actix_web::test]
async fn test_disable_user_invalidates_status_cache() {
    let cache_config = aster_drive::config::CacheConfig {
        enabled: true,
        backend: "memory".to_string(),
        default_ttl: 60,
        ..Default::default()
    };
    let cache = aster_drive::cache::create_cache(&cache_config).await;

    let base = common::setup().await;
    let state = aster_drive::runtime::AppState {
        db: base.db,
        driver_registry: base.driver_registry,
        runtime_config: base.runtime_config,
        policy_snapshot: base.policy_snapshot,
        config: base.config,
        cache,
        mail_sender: base.mail_sender,
        thumbnail_tx: base.thumbnail_tx,
        storage_change_tx: base.storage_change_tx,
    };
    let app = create_test_app!(state);
    let (admin_token, _) = register_and_login!(app);

    let bob_id = admin_create_user_with_credentials!(
        app,
        admin_token,
        "bobuser",
        "bob@example.com",
        "password456"
    );

    // bob 登录
    let req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "identifier": "bobuser",
            "password": "password456"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let bob_token = common::extract_cookie(&resp, "aster_access").unwrap();
    let bob_refresh = common::extract_cookie(&resp, "aster_refresh").unwrap();

    // bob 正常访问（写入缓存）
    let req = test::TestRequest::get()
        .uri("/api/v1/folders")
        .insert_header(("Cookie", format!("aster_access={bob_token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // admin 禁用 bob
    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/admin/users/{bob_id}"))
        .insert_header(("Cookie", format!("aster_access={admin_token}")))
        .set_json(serde_json::json!({ "status": "disabled" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // bob 再次访问——应被拒（缓存已失效）
    let req = test::TestRequest::get()
        .uri("/api/v1/folders")
        .insert_header(("Cookie", format!("aster_access={bob_token}")))
        .to_request();
    assert_service_status!(app, req, 403, "disabled user should get 403");

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/refresh")
        .insert_header(("Cookie", format!("aster_refresh={bob_refresh}")))
        .to_request();
    assert_service_status!(app, req, 403, "disabled user refresh should get 403");

    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/admin/users/{bob_id}"))
        .insert_header(("Cookie", format!("aster_access={admin_token}")))
        .set_json(serde_json::json!({ "status": "active" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::get()
        .uri("/api/v1/auth/me")
        .insert_header(("Cookie", format!("aster_access={bob_token}")))
        .to_request();
    assert_service_status!(app, req, 401, "old token should stay revoked");

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/refresh")
        .insert_header(("Cookie", format!("aster_refresh={bob_refresh}")))
        .to_request();
    assert_service_status!(app, req, 401, "old refresh token should stay revoked");
}

// ── Preferences endpoint tests ──

/// Set preferences via PATCH, then verify they are returned by GET /me.
#[actix_web::test]
async fn test_patch_preferences_set_and_get() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    // Patch all fields
    let req = test::TestRequest::patch()
        .uri("/api/v1/auth/preferences")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "theme_mode": "dark",
            "color_preset": "green",
            "view_mode": "grid",
            "browser_open_mode": "double_click",
            "sort_by": "size",
            "sort_order": "desc",
            "language": "zh",
            "storage_event_stream_enabled": false
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["code"], 0);
    assert_eq!(body["data"]["theme_mode"], "dark");
    assert_eq!(body["data"]["color_preset"], "green");
    assert_eq!(body["data"]["view_mode"], "grid");
    assert_eq!(body["data"]["browser_open_mode"], "double_click");
    assert_eq!(body["data"]["sort_by"], "size");
    assert_eq!(body["data"]["sort_order"], "desc");
    assert_eq!(body["data"]["language"], "zh");
    assert_eq!(body["data"]["storage_event_stream_enabled"], false);

    // Verify via GET /me
    let req = test::TestRequest::get()
        .uri("/api/v1/auth/me")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["preferences"]["theme_mode"], "dark");
    assert_eq!(body["data"]["preferences"]["view_mode"], "grid");
    assert_eq!(
        body["data"]["preferences"]["browser_open_mode"],
        "double_click"
    );
    assert_eq!(body["data"]["preferences"]["language"], "zh");
    assert_eq!(
        body["data"]["preferences"]["storage_event_stream_enabled"],
        false
    );
}

/// Partial PATCH only updates specified fields; others remain unchanged.
#[actix_web::test]
async fn test_patch_preferences_partial_update() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    // Set initial preferences
    let req = test::TestRequest::patch()
        .uri("/api/v1/auth/preferences")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "theme_mode": "dark",
            "view_mode": "grid",
            "browser_open_mode": "double_click"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // Partial update: only change sort_by
    let req = test::TestRequest::patch()
        .uri("/api/v1/auth/preferences")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "sort_by": "size"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    // Previously set fields should be preserved
    assert_eq!(body["data"]["theme_mode"], "dark");
    assert_eq!(body["data"]["view_mode"], "grid");
    assert_eq!(body["data"]["browser_open_mode"], "double_click");
    // Newly set field
    assert_eq!(body["data"]["sort_by"], "size");
}

/// Invalid enum values should be rejected with a 400 error.
#[actix_web::test]
async fn test_patch_preferences_invalid_enum_value() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let req = test::TestRequest::patch()
        .uri("/api/v1/auth/preferences")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "theme_mode": "invalid_value"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400, "invalid enum value should return 400");

    // sort_order with invalid value
    let req = test::TestRequest::patch()
        .uri("/api/v1/auth/preferences")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "sort_order": "sideways"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400, "invalid sort_order should return 400");
}

/// PATCH with empty body should succeed (no-op, returns current prefs).
#[actix_web::test]
async fn test_patch_preferences_empty_body() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    // Empty body — should succeed with no changes
    let req = test::TestRequest::patch()
        .uri("/api/v1/auth/preferences")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    // All fields should be null for a fresh user
    assert!(body["data"]["theme_mode"].is_null());
    assert!(body["data"]["color_preset"].is_null());
    assert!(body["data"]["browser_open_mode"].is_null());
    assert!(body["data"]["language"].is_null());
    assert!(body["data"]["storage_event_stream_enabled"].is_null());

    // Verify via GET /me — fresh user has no stored config so preferences is null
    let req = test::TestRequest::get()
        .uri("/api/v1/auth/me")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert!(body["data"]["preferences"].is_null());
}

/// sort_by = "type" uses a special snake_case rename; verify it round-trips correctly.
#[actix_web::test]
async fn test_patch_preferences_sort_by_type() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let req = test::TestRequest::patch()
        .uri("/api/v1/auth/preferences")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({ "sort_by": "type" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["sort_by"], "type");
}

#[actix_web::test]
async fn test_patch_profile_display_name_round_trip_and_clear() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let req = test::TestRequest::patch()
        .uri("/api/v1/auth/profile")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "display_name": "  Test User  "
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["display_name"], "Test User");
    assert_eq!(body["data"]["avatar"]["source"], "none");

    let (boundary, payload) = avatar_upload_payload();
    let req = test::TestRequest::post()
        .uri("/api/v1/auth/profile/avatar/upload")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .insert_header((
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        ))
        .set_payload(payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["display_name"], "Test User");
    assert_eq!(body["data"]["avatar"]["source"], "upload");

    let req = test::TestRequest::patch()
        .uri("/api/v1/auth/profile")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "display_name": "   "
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert!(body["data"]["display_name"].is_null());
    assert_eq!(body["data"]["avatar"]["source"], "upload");

    let req = test::TestRequest::get()
        .uri("/api/v1/auth/me")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert!(body["data"]["profile"]["display_name"].is_null());
    assert_eq!(body["data"]["profile"]["avatar"]["source"], "upload");
}

#[actix_web::test]
async fn test_change_password_rotates_session_and_updates_login_secret() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, refresh) = register_and_login!(app);

    let req = test::TestRequest::put()
        .uri("/api/v1/auth/password")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "current_password": "password123",
            "new_password": "newsecret456"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let rotated_access = common::extract_cookie(&resp, "aster_access").unwrap();
    let rotated_refresh = common::extract_cookie(&resp, "aster_refresh").unwrap();
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["code"], 0);
    assert_eq!(body["data"]["expires_in"], 900);

    let req = test::TestRequest::get()
        .uri("/api/v1/auth/me")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    assert_service_status!(app, req, 401);

    let req = test::TestRequest::get()
        .uri("/api/v1/auth/me")
        .insert_header(("Cookie", format!("aster_access={rotated_access}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/refresh")
        .insert_header(("Cookie", format!("aster_refresh={refresh}")))
        .to_request();
    assert_service_status!(app, req, 401);

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/refresh")
        .insert_header(("Cookie", format!("aster_refresh={rotated_refresh}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "identifier": "testuser",
            "password": "password123"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 401);

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "identifier": "testuser",
            "password": "newsecret456"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

#[actix_web::test]
async fn test_change_password_rejects_wrong_current_password() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let req = test::TestRequest::put()
        .uri("/api/v1/auth/password")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "current_password": "wrongpassword",
            "new_password": "newsecret456"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 401);

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "identifier": "testuser",
            "password": "password123"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

#[actix_web::test]
async fn test_patch_profile_rejects_overlong_display_name() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let req = test::TestRequest::patch()
        .uri("/api/v1/auth/profile")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "display_name": "a".repeat(65)
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);

    let req = test::TestRequest::get()
        .uri("/api/v1/auth/me")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert!(body["data"]["profile"]["display_name"].is_null());
}

#[actix_web::test]
async fn test_display_name_survives_avatar_source_switches() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let req = test::TestRequest::patch()
        .uri("/api/v1/auth/profile")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "display_name": "Avatar User"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let (boundary, payload) = avatar_upload_payload();
    let req = test::TestRequest::post()
        .uri("/api/v1/auth/profile/avatar/upload")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .insert_header((
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        ))
        .set_payload(payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["display_name"], "Avatar User");
    assert_eq!(body["data"]["avatar"]["source"], "upload");

    for source in ["gravatar", "none"] {
        let req = test::TestRequest::put()
            .uri("/api/v1/auth/profile/avatar/source")
            .insert_header(("Cookie", format!("aster_access={token}")))
            .set_json(serde_json::json!({ "source": source }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
        let body: Value = test::read_body_json(resp).await;
        assert_eq!(body["data"]["display_name"], "Avatar User");
        assert_eq!(body["data"]["avatar"]["source"], source);
    }
}

#[actix_web::test]
async fn test_avatar_upload_and_source_switch() {
    let state = common::setup().await;
    let avatar_base_path = state
        .runtime_config
        .get(aster_drive::config::avatar::AVATAR_DIR_KEY)
        .expect("avatar_dir should exist");
    let shared_policy_base_path = aster_drive::db::repository::policy_repo::find_default(&state.db)
        .await
        .unwrap()
        .expect("default policy should exist")
        .base_path;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let req = test::TestRequest::get()
        .uri("/api/v1/auth/me")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    let user_id = body["data"]["id"].as_i64().unwrap();

    let (boundary, payload) = avatar_upload_payload();
    let req = test::TestRequest::post()
        .uri("/api/v1/auth/profile/avatar/upload")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .insert_header((
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        ))
        .set_payload(payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["avatar"]["source"], "upload");
    assert_eq!(body["data"]["avatar"]["version"], 1);
    assert_eq!(
        body["data"]["avatar"]["url_512"],
        "/auth/profile/avatar/512?v=1"
    );
    let avatar_user_dir =
        std::path::PathBuf::from(&avatar_base_path).join(format!("user/{user_id}"));
    let avatar_v1_dir = avatar_user_dir.join("v1");
    let avatar_v1_512 = avatar_v1_dir.join("512.webp");
    let avatar_v1_1024 = avatar_v1_dir.join("1024.webp");
    assert!(avatar_v1_512.exists());
    assert!(avatar_v1_1024.exists());
    assert!(
        !std::path::PathBuf::from(&shared_policy_base_path)
            .join(format!("user/{user_id}/v1/512.webp"))
            .exists()
    );
    assert!(
        !std::path::PathBuf::from(&shared_policy_base_path)
            .join(format!("user/{user_id}/v1/1024.webp"))
            .exists()
    );

    let req = test::TestRequest::get()
        .uri("/api/v1/auth/profile/avatar/512")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.headers().get("content-type").unwrap(), "image/webp");

    let req = test::TestRequest::put()
        .uri("/api/v1/auth/profile/avatar/source")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "source": "gravatar"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["avatar"]["source"], "gravatar");
    assert_eq!(body["data"]["avatar"]["version"], 2);
    assert!(
        body["data"]["avatar"]["url_512"]
            .as_str()
            .unwrap()
            .contains("gravatar.com/avatar/")
    );
    assert!(!avatar_v1_512.exists());
    assert!(!avatar_v1_1024.exists());
    assert!(!avatar_v1_dir.exists());
    assert!(!avatar_user_dir.exists());

    let req = test::TestRequest::get()
        .uri("/api/v1/auth/profile/avatar/512")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 404);
}

#[actix_web::test]
async fn test_avatar_reupload_replaces_previous_objects() {
    let state = common::setup().await;
    let avatar_base_path = state
        .runtime_config
        .get(aster_drive::config::avatar::AVATAR_DIR_KEY)
        .expect("avatar_dir should exist");
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let req = test::TestRequest::get()
        .uri("/api/v1/auth/me")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    let user_id = body["data"]["id"].as_i64().unwrap();

    let (boundary, payload) = avatar_upload_payload();
    let req = test::TestRequest::post()
        .uri("/api/v1/auth/profile/avatar/upload")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .insert_header((
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        ))
        .set_payload(payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let avatar_user_dir =
        std::path::PathBuf::from(&avatar_base_path).join(format!("user/{user_id}"));
    let avatar_v1_dir = avatar_user_dir.join("v1");
    let avatar_v1_512 = avatar_v1_dir.join("512.webp");
    let avatar_v1_1024 = avatar_v1_dir.join("1024.webp");
    assert!(avatar_v1_512.exists());
    assert!(avatar_v1_1024.exists());

    let (boundary, payload) = avatar_upload_payload();
    let req = test::TestRequest::post()
        .uri("/api/v1/auth/profile/avatar/upload")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .insert_header((
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        ))
        .set_payload(payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["avatar"]["source"], "upload");
    assert_eq!(body["data"]["avatar"]["version"], 2);
    assert_eq!(
        body["data"]["avatar"]["url_512"],
        "/auth/profile/avatar/512?v=2"
    );

    let avatar_v2_dir = avatar_user_dir.join("v2");
    let avatar_v2_512 = avatar_v2_dir.join("512.webp");
    let avatar_v2_1024 = avatar_v2_dir.join("1024.webp");
    assert!(!avatar_v1_512.exists());
    assert!(!avatar_v1_1024.exists());
    assert!(!avatar_v1_dir.exists());
    assert!(avatar_user_dir.exists());
    assert!(avatar_v2_dir.exists());
    assert!(avatar_v2_512.exists());
    assert!(avatar_v2_1024.exists());
}

#[actix_web::test]
async fn test_avatar_switch_to_none_deletes_uploaded_objects() {
    let state = common::setup().await;
    let avatar_base_path = state
        .runtime_config
        .get(aster_drive::config::avatar::AVATAR_DIR_KEY)
        .expect("avatar_dir should exist");
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let req = test::TestRequest::get()
        .uri("/api/v1/auth/me")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    let user_id = body["data"]["id"].as_i64().unwrap();

    let (boundary, payload) = avatar_upload_payload();
    let req = test::TestRequest::post()
        .uri("/api/v1/auth/profile/avatar/upload")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .insert_header((
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        ))
        .set_payload(payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let avatar_user_dir =
        std::path::PathBuf::from(&avatar_base_path).join(format!("user/{user_id}"));
    let avatar_v1_dir = avatar_user_dir.join("v1");
    let avatar_v1_512 = avatar_v1_dir.join("512.webp");
    let avatar_v1_1024 = avatar_v1_dir.join("1024.webp");
    assert!(avatar_v1_512.exists());
    assert!(avatar_v1_1024.exists());

    let req = test::TestRequest::put()
        .uri("/api/v1/auth/profile/avatar/source")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "source": "none"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["avatar"]["source"], "none");
    assert_eq!(body["data"]["avatar"]["version"], 2);
    assert!(body["data"]["avatar"]["url_512"].is_null());

    assert!(!avatar_v1_512.exists());
    assert!(!avatar_v1_1024.exists());
    assert!(!avatar_v1_dir.exists());
    assert!(!avatar_user_dir.exists());

    let req = test::TestRequest::get()
        .uri("/api/v1/auth/profile/avatar/512")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 404);
}

/// Unauthenticated requests to PATCH /preferences should be rejected.
#[actix_web::test]
async fn test_patch_preferences_unauthenticated() {
    let state = common::setup().await;
    let app = create_test_app!(state);

    let req = test::TestRequest::patch()
        .uri("/api/v1/auth/preferences")
        .set_json(serde_json::json!({
            "theme_mode": "dark"
        }))
        .to_request();
    let result = test::try_call_service(&app, req).await;
    match result {
        Ok(resp) => assert_eq!(resp.status(), 401),
        Err(err) => assert_eq!(err.error_response().status(), 401),
    }
}
