use actix_web::{HttpRequest, HttpResponse, web};
use rust_embed::Embed;
use std::path::PathBuf;

#[derive(Embed)]
#[folder = "frontend-panel/dist/"]
struct FrontendAssets;

/// 运行时可覆盖的前端目录
const CUSTOM_FRONTEND_DIR: &str = "./frontend-panel/dist";

pub struct FrontendService;

impl FrontendService {
    /// 优先从自定义目录加载，fallback 到嵌入资源
    async fn load_file(file_path: &str) -> Option<Vec<u8>> {
        if file_path.contains("..") {
            return None;
        }

        let custom_path = PathBuf::from(CUSTOM_FRONTEND_DIR).join(file_path);
        if let Ok(data) = tokio::fs::read(&custom_path).await {
            tracing::trace!("serving from custom dir: {file_path}");
            return Some(data);
        }

        FrontendAssets::get(file_path).map(|c| c.data.into_owned())
    }

    /// 服务 index.html，替换配置占位符
    async fn serve_index() -> HttpResponse {
        let html = match Self::load_file("index.html").await {
            Some(data) => String::from_utf8_lossy(&data).into_owned(),
            None => include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/frontend-panel/dist/index.html"
            ))
            .to_string(),
        };

        let processed = html.replace("%ASTERDRIVE_VERSION%", env!("CARGO_PKG_VERSION"));

        HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(processed)
    }

    pub async fn handle_index(_req: HttpRequest) -> HttpResponse {
        Self::serve_index().await
    }

    pub async fn handle_assets(req: HttpRequest) -> HttpResponse {
        let path = req.match_info().query("path");
        let asset_path = format!("assets/{path}");
        let content_type = Self::get_content_type(path);

        match Self::load_file(&asset_path).await {
            Some(data) => HttpResponse::Ok().content_type(content_type).body(data),
            None => HttpResponse::NotFound().body("File not found"),
        }
    }

    pub async fn handle_static(req: HttpRequest) -> HttpResponse {
        let path = req.match_info().query("path");
        let asset_path = format!("static/{path}");
        let content_type = Self::get_content_type(path);

        match Self::load_file(&asset_path).await {
            Some(data) => HttpResponse::Ok().content_type(content_type).body(data),
            None => HttpResponse::NotFound().body("File not found"),
        }
    }

    pub async fn handle_favicon(_req: HttpRequest) -> HttpResponse {
        match Self::load_file("favicon.svg").await {
            Some(data) => HttpResponse::Ok().content_type("image/svg+xml").body(data),
            None => HttpResponse::Ok()
                .content_type("image/svg+xml")
                .body(Vec::new()),
        }
    }

    pub async fn handle_spa_fallback(_req: HttpRequest) -> HttpResponse {
        Self::serve_index().await
    }

    pub async fn handle_pwa_file(req: HttpRequest) -> HttpResponse {
        let filename = req
            .uri()
            .path()
            .trim_start_matches('/');
        let content_type = Self::get_content_type(filename);
        match Self::load_file(filename).await {
            Some(data) => HttpResponse::Ok().content_type(content_type).body(data),
            None => HttpResponse::NotFound().body("File not found"),
        }
    }

    fn get_content_type(path: &str) -> &'static str {
        match path.rsplit('.').next() {
            Some("css") => "text/css",
            Some("js" | "mjs") => "application/javascript",
            Some("json") => "application/json",
            Some("webmanifest") => "application/manifest+json",
            Some("png") => "image/png",
            Some("jpg" | "jpeg") => "image/jpeg",
            Some("gif") => "image/gif",
            Some("svg") => "image/svg+xml",
            Some("ico") => "image/x-icon",
            Some("woff") => "font/woff",
            Some("woff2") => "font/woff2",
            Some("ttf") => "font/ttf",
            _ => "application/octet-stream",
        }
    }
}

/// 前端路由，挂在 `/` 下，必须最后注册
pub fn routes() -> actix_web::Scope {
    web::scope("")
        .route("/", web::get().to(FrontendService::handle_index))
        .route(
            "/assets/{path:.*}",
            web::get().to(FrontendService::handle_assets),
        )
        .route(
            "/static/{path:.*}",
            web::get().to(FrontendService::handle_static),
        )
        .route(
            "/favicon.svg",
            web::get().to(FrontendService::handle_favicon),
        )
        // PWA 文件（sw.js, workbox-*.js, manifest.webmanifest）
        .route(
            "/sw.js",
            web::get().to(FrontendService::handle_pwa_file),
        )
        .route(
            "/manifest.webmanifest",
            web::get().to(FrontendService::handle_pwa_file),
        )
        .route(
            "/{filename:workbox-[^/]*}",
            web::get().to(FrontendService::handle_pwa_file),
        )
        // SPA fallback（最后）
        .route(
            "/{path:.*}",
            web::get().to(FrontendService::handle_spa_fallback),
        )
}
