pub mod auth;
pub mod db_lock_system;
pub mod deltav;
pub mod dir_entry;
pub mod file;
pub mod fs;
pub mod metadata;
pub mod path_resolver;

use actix_web::web;
use dav_server::actix::{DavRequest, DavResponse};
use dav_server::{DavConfig, DavHandler};
use sea_orm::DatabaseConnection;

use crate::config::WebDavConfig;
use crate::runtime::AppState;

/// WebDAV 共享状态（单例）
pub struct WebDavState {
    pub handler: DavHandler,
    pub prefix: String,
    pub db: DatabaseConnection,
}

/// WebDAV handler — 所有 HTTP 方法都路由到这里，由 dav-server 内部分派
pub async fn webdav_handler(
    dav_req: DavRequest,
    state: web::Data<AppState>,
    webdav: web::Data<WebDavState>,
) -> DavResponse {
    // 1. 检查运行时开关 (system_config: webdav_enabled)
    let enabled = state.runtime_config.get_bool_or("webdav_enabled", true);

    if !enabled {
        return http::Response::builder()
            .status(503)
            .body(dav_server::body::Body::from("WebDAV is disabled"))
            .unwrap()
            .into();
    }

    // 2. 认证
    let auth_result = match auth::authenticate_webdav(dav_req.request.headers(), &state).await {
        Ok(r) => r,
        Err(_) => {
            return http::Response::builder()
                .status(401)
                .header("WWW-Authenticate", "Basic realm=\"AsterDrive WebDAV\"")
                .body(dav_server::body::Body::from("Unauthorized"))
                .unwrap()
                .into();
        }
    };

    // 3. DeltaV 方法拦截（dav-server 不支持 RFC3253）
    let method = dav_req.request.method().to_string();
    match method.as_str() {
        "REPORT" => {
            // 消费请求，分离 URI 和 body
            let (parts, body) = dav_req.request.into_parts();
            let body_bytes = collect_body(body).await;
            return deltav::handle_report(
                &parts.uri,
                &body_bytes,
                &state.db,
                &auth_result,
                &webdav.prefix,
            )
            .await
            .into();
        }
        "VERSION-CONTROL" => {
            return deltav::handle_version_control(
                dav_req.request.uri(),
                &state.db,
                &auth_result,
                &webdav.prefix,
            )
            .await
            .into();
        }
        _ => {}
    }

    // 4. 创建 per-user 文件系统（可能限制到指定文件夹）
    let dav_fs = fs::AsterDavFs::new(
        state.db.clone(),
        state.driver_registry.clone(),
        state.runtime_config.clone(),
        state.policy_snapshot.clone(),
        state.config.clone(),
        state.cache.clone(),
        state.mail_sender.clone(),
        state.thumbnail_tx.clone(),
        state.storage_change_tx.clone(),
        auth_result.user_id,
        auth_result.root_folder_id,
    );

    // 5. Per-request 锁系统（需要 user_id 做 path → entity 解析）
    let lock_system = db_lock_system::DbLockSystem::new(
        webdav.db.clone(),
        auth_result.user_id,
        auth_result.root_folder_id,
    );

    // 6. 构建 per-request 配置
    let config = DavConfig::new()
        .filesystem(Box::new(dav_fs))
        .locksystem(lock_system)
        .strip_prefix(&webdav.prefix);

    // 7. 交给 dav-server 处理
    let response: DavResponse = webdav
        .handler
        .handle_with(config, dav_req.request)
        .await
        .into();

    // 8. OPTIONS 响应追加 DeltaV 版本控制标记
    if method == "OPTIONS" {
        return append_deltav_header(response);
    }

    response
}

/// 从 http_body::Body 中收集全部字节
async fn collect_body<B>(body: B) -> Vec<u8>
where
    B: http_body::Body<Data = bytes::Bytes>,
{
    let mut body = Box::pin(body);
    let mut data = Vec::new();
    while let Some(Ok(frame)) = std::future::poll_fn(|cx| body.as_mut().poll_frame(cx)).await {
        if let Ok(chunk) = frame.into_data() {
            data.extend_from_slice(&chunk);
        }
    }
    data
}

/// 在 OPTIONS 响应的 DAV 头中追加 version-control 合规标记
fn append_deltav_header(response: DavResponse) -> DavResponse {
    let DavResponse(mut resp) = response;
    if let Some(dav_value) = resp.headers().get("DAV").cloned()
        && let Ok(s) = dav_value.to_str()
    {
        let new_value = format!("{s}, version-control");
        if let Ok(hv) = http::HeaderValue::from_str(&new_value) {
            resp.headers_mut().insert("DAV", hv);
        }
    }
    DavResponse(resp)
}

/// 注册 WebDAV 路由（需要 db 来创建 per-request DbLockSystem）
pub fn configure(
    cfg: &mut web::ServiceConfig,
    webdav_config: &WebDavConfig,
    db: &DatabaseConnection,
) {
    let webdav_state = web::Data::new(WebDavState {
        handler: DavHandler::builder().build_handler(),
        prefix: webdav_config.prefix.clone(),
        db: db.clone(),
    });

    cfg.app_data(webdav_state).service(
        web::scope(&webdav_config.prefix)
            .app_data(web::PayloadConfig::new(webdav_config.payload_limit))
            .default_service(web::to(webdav_handler)),
    );
}
