use std::fs;
use aster_drive::api::openapi::ApiDoc;
use utoipa::OpenApi;

#[test]
fn generate_openapi() {
    let doc = ApiDoc::openapi();
    let json = serde_json::to_string_pretty(&doc).unwrap();
    fs::create_dir_all("./frontend-panel/generated").expect("Unable to create directory");
    fs::write("./frontend-panel/generated/openapi.json", json).expect("Unable to write OpenAPI spec");
}
