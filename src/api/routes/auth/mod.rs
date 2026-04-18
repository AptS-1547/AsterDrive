//! 认证 API 路由聚合入口。

pub use crate::api::dto::auth::*;
use crate::api::middleware::rate_limit;
use crate::api::request_auth::access_token;
use crate::config::RateLimitConfig;
use crate::config::site_url;
use crate::runtime::AppState;
use crate::services::{auth_service, storage_change_service};
use actix_governor::Governor;
use actix_web::http::header;
use actix_web::middleware::Condition;
use actix_web::{HttpRequest, HttpResponse, web};
use bytes::Bytes;
use rand::RngExt;

pub use self::profile::{
    get_self_avatar, patch_preferences, patch_profile, put_avatar_source, request_email_change,
    resend_email_change, upload_avatar,
};
pub use self::public::{
    check, confirm_contact_verification, confirm_password_reset, register, request_password_reset,
    resend_register_activation, setup,
};
pub use self::session::{get_storage_events, login, logout, me, put_password, refresh};
pub use crate::services::profile_service::{AvatarInfo, UserProfileInfo};
pub use crate::services::user_service::{MeResponse, UpdatePreferencesReq, UserInfo};
pub use crate::types::{
    AvatarSource, BrowserOpenMode, ColorPreset, Language, PrefViewMode, ThemeMode, UserPreferences,
};

const AUTH_MAIL_RESPONSE_FLOOR_MS: u64 = 350;
const AUTH_MAIL_RESPONSE_JITTER_MS: u64 = 125;

pub mod cookies;
pub mod profile;
pub mod public;
pub mod session;

pub fn routes(rl: &RateLimitConfig) -> impl actix_web::dev::HttpServiceFactory + use<> {
    let auth_limiter = rate_limit::build_governor(&rl.auth, &rl.trusted_proxies);
    // 已认证端点（/auth/me、/auth/preferences 等）用 api tier，
    // 避免匿名大量请求 /login 耗尽同一个桶并把真实用户锁在 /me 之外。
    let api_limiter = rate_limit::build_governor(&rl.api, &rl.trusted_proxies);

    // 公开路由 + 认证路由分别注册到 /auth 路径下
    web::scope("/auth")
        .service(
            web::scope("")
                .wrap(Condition::new(rl.enabled, Governor::new(&auth_limiter)))
                .route("/check", web::post().to(check))
                .route("/register", web::post().to(register))
                .route(
                    "/register/resend",
                    web::post().to(resend_register_activation),
                )
                .route("/setup", web::post().to(setup))
                .route(
                    "/contact-verification/confirm",
                    web::get().to(confirm_contact_verification),
                )
                .route(
                    "/password/reset/request",
                    web::post().to(request_password_reset),
                )
                .route(
                    "/password/reset/confirm",
                    web::post().to(confirm_password_reset),
                )
                .route("/login", web::post().to(login))
                .route("/refresh", web::post().to(refresh))
                .route("/logout", web::post().to(logout)),
        )
        .service(
            web::scope("")
                .wrap(crate::api::middleware::auth::JwtAuth)
                .wrap(Condition::new(rl.enabled, Governor::new(&api_limiter)))
                .route("/me", web::get().to(me))
                .route("/password", web::put().to(put_password))
                .route("/email/change", web::post().to(request_email_change))
                .route("/email/change/resend", web::post().to(resend_email_change))
                .route("/preferences", web::patch().to(patch_preferences))
                .route("/profile", web::patch().to(patch_profile))
                .route("/profile/avatar/upload", web::post().to(upload_avatar))
                .route("/profile/avatar/source", web::put().to(put_avatar_source))
                .route("/events/storage", web::get().to(get_storage_events))
                .route("/profile/avatar/{size}", web::get().to(get_self_avatar)),
        )
}

async fn apply_auth_mail_response_floor(started_at: tokio::time::Instant) {
    let mut rng = rand::rng();
    let jitter_ms = rng.random_range(0..=AUTH_MAIL_RESPONSE_JITTER_MS);
    let target = std::time::Duration::from_millis(AUTH_MAIL_RESPONSE_FLOOR_MS + jitter_ms);
    let elapsed = started_at.elapsed();
    if elapsed < target {
        tokio::time::sleep(target - elapsed).await;
    }
}

#[derive(Clone, Copy)]
enum ContactVerificationRedirectStatus {
    EmailChanged,
    Expired,
    Invalid,
    Missing,
    RegisterActivated,
}

impl ContactVerificationRedirectStatus {
    fn as_query_value(self) -> &'static str {
        match self {
            Self::EmailChanged => "email-changed",
            Self::Expired => "expired",
            Self::Invalid => "invalid",
            Self::Missing => "missing",
            Self::RegisterActivated => "register-activated",
        }
    }
}

async fn request_has_active_access_session(state: &AppState, req: &HttpRequest) -> bool {
    let Some(token) = access_token(req) else {
        return false;
    };

    auth_service::authenticate_access_token(state, &token)
        .await
        .is_ok()
}

fn contact_verification_redirect_url(
    state: &AppState,
    path: &str,
    status: ContactVerificationRedirectStatus,
    email: Option<&str>,
) -> String {
    let mut redirect_path = format!("{path}?contact_verification={}", status.as_query_value());

    if let Some(email) = email {
        redirect_path.push_str("&email=");
        redirect_path.push_str(&urlencoding::encode(email));
    }

    site_url::public_app_url_or_path(&state.runtime_config, &redirect_path)
}

fn contact_verification_redirect_response(
    state: &AppState,
    path: &str,
    status: ContactVerificationRedirectStatus,
    email: Option<&str>,
) -> HttpResponse {
    HttpResponse::Found()
        .append_header((
            header::LOCATION,
            contact_verification_redirect_url(state, path, status, email),
        ))
        .finish()
}

fn storage_event_frame(event: &storage_change_service::StorageChangeEvent) -> Option<Bytes> {
    serde_json::to_string(event)
        .map(|json| Bytes::from(format!("data: {json}\n\n")))
        .map_err(|e| tracing::warn!("failed to serialize storage change event: {e}"))
        .ok()
}
