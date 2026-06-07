#![cfg(all(debug_assertions, feature = "openapi"))]
//! OpenAPI 生成测试。

use aster_drive::api::api_error_code::ApiErrorCode;
use aster_drive::api::openapi::ApiDoc;
use std::fs;
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::OpenApi;

#[test]
fn generate_openapi() {
    let doc = ApiDoc::openapi();
    let json = serde_json::to_string_pretty(&doc).unwrap();
    fs::create_dir_all("./frontend-panel/generated").expect("Unable to create directory");
    fs::write("./frontend-panel/generated/openapi.json", json)
        .expect("Unable to write OpenAPI spec");
}

#[test]
fn api_error_code_openapi_schema_uses_wire_values() {
    let value = serde_json::to_value(ApiDoc::openapi()).unwrap();
    let schema = &value["components"]["schemas"]["ApiErrorCode"];
    let values = schema["enum"]
        .as_array()
        .expect("ApiErrorCode schema should have enum values")
        .iter()
        .map(|value| {
            value
                .as_str()
                .expect("ApiErrorCode enum value should be string")
        })
        .collect::<Vec<_>>();

    assert_eq!(schema["type"], "string");
    assert_eq!(values.len(), ApiErrorCode::ALL.len());
    for code in ApiErrorCode::ALL {
        assert!(
            values.contains(&code.as_str()),
            "OpenAPI schema missing {}",
            code.as_str()
        );
    }
    assert!(!values.contains(&"AuthFailed"));
    assert!(!values.contains(&"StorageTransient"));
    assert!(!values.contains(&"remote.dynamic"));
}

#[test]
fn api_error_code_openapi_schema_has_unique_values() {
    let value = serde_json::to_value(ApiDoc::openapi()).unwrap();
    let values = value["components"]["schemas"]["ApiErrorCode"]["enum"]
        .as_array()
        .expect("ApiErrorCode schema should have enum values");
    let mut seen = std::collections::HashSet::new();

    for value in values {
        let value = value
            .as_str()
            .expect("ApiErrorCode enum value should be string");
        assert!(seen.insert(value), "duplicate ApiErrorCode value {value}");
    }
}

#[test]
fn api_error_info_openapi_exposes_retryable_only() {
    let value = serde_json::to_value(ApiDoc::openapi()).unwrap();
    let info = &value["components"]["schemas"]["ApiErrorInfo"];
    let properties = info["properties"]
        .as_object()
        .expect("ApiErrorInfo should have properties");

    assert!(properties.contains_key("retryable"));
    assert!(!properties.contains_key("code"));
    assert!(!properties.contains_key("internal_code"));
    assert!(!properties.contains_key("subcode"));
    assert!(!properties.contains_key("api_code"));
}

#[test]
fn api_response_openapi_code_references_api_error_code_schema() {
    let value = serde_json::to_value(ApiDoc::openapi()).unwrap();
    let schemas = value["components"]["schemas"]
        .as_object()
        .expect("components schemas should be object");
    let responses = schemas
        .iter()
        .filter(|(name, _)| name.starts_with("ApiResponse_"));
    let mut checked = 0;

    for (name, schema) in responses {
        let code = &schema["properties"]["code"];
        assert_eq!(
            code["$ref"],
            serde_json::json!("#/components/schemas/ApiErrorCode"),
            "{name} should reference ApiErrorCode for code"
        );
        assert!(
            code.get("enum").is_none(),
            "{name} code should not inline enum values"
        );
        checked += 1;
    }

    assert!(checked > 0, "at least one ApiResponse schema should exist");
}
