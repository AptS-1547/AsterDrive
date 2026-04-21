use super::common::api_not_found;
use super::routes;
use actix_web::web;

pub fn configure_primary(cfg: &mut web::ServiceConfig, db: &sea_orm::DatabaseConnection) {
    let rl = crate::config::try_get_config()
        .map(|c| c.rate_limit.clone())
        .unwrap_or_default();

    cfg.service(
        web::scope("/api/v1")
            .service(routes::auth::routes(&rl))
            .service(routes::files::routes(&rl))
            .service(routes::folders::routes(&rl))
            .service(routes::admin::routes(&rl))
            .service(routes::shares::routes(&rl))
            .service(routes::share_public::routes(&rl))
            .service(routes::webdav_accounts::routes(&rl))
            .service(routes::trash::routes(&rl))
            .service(routes::properties::routes(&rl))
            .service(routes::batch::routes(&rl))
            .service(routes::search::routes(&rl))
            .service(routes::tasks::routes(&rl))
            .service(routes::teams::routes(&rl))
            .service(routes::public::routes())
            .service(routes::wopi::routes())
            .default_service(web::to(api_not_found)),
    )
    .service(routes::health::primary_routes())
    .service(routes::share_public::direct_routes(&rl));

    #[cfg(all(debug_assertions, feature = "openapi"))]
    configure_openapi(cfg);

    if let Some(config) = crate::config::try_get_config() {
        crate::webdav::configure(cfg, &config.webdav, db);
    }

    cfg.service(routes::frontend::routes());
}

#[cfg(all(debug_assertions, feature = "openapi"))]
fn configure_openapi(cfg: &mut web::ServiceConfig) {
    use super::openapi;
    use actix_web::HttpResponse;
    use utoipa::OpenApi;
    use utoipa_swagger_ui::SwaggerUi;

    let spec = openapi::ApiDoc::openapi();
    let spec_clone = spec.clone();
    cfg.service(web::scope("/api-docs").route(
        "/openapi.json",
        web::get().to(move || {
            let s = spec_clone.clone();
            async move { HttpResponse::Ok().json(s) }
        }),
    ));
    cfg.service(SwaggerUi::new("/swagger-ui/{_:.*}").url("/api-docs/openapi.json", spec));
}
