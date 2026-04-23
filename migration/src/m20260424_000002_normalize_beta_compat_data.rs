//! 数据库迁移：统一清理 beta 前移除的兼容数据。

use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::{Map as JsonMap, Value as JsonValue};
use std::io::Cursor;

#[derive(DeriveMigrationName)]
pub struct Migration;

const PREVIEW_APPS_CONFIG_KEY: &str = "frontend_preview_apps_json";
const PREVIEW_APP_DISCOVERY_GENERATED_SEGMENT: &str = "__wopi_discovery__";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum LockOwnerInfo {
    Wopi { app_key: String, lock: String },
    Webdav { xml: String },
    Text { value: String },
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        normalize_remote_upload_strategy(manager).await?;
        normalize_resource_lock_owner_info(manager).await?;
        normalize_preview_apps_config(manager).await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}

async fn normalize_remote_upload_strategy(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let db = manager.get_connection();
    let backend = db.get_database_backend();
    let rows = db
        .query_all_raw(Statement::from_string(
            backend,
            "SELECT id, options FROM storage_policies".to_string(),
        ))
        .await?;

    for row in rows {
        let id = row
            .try_get_by_index::<i64>(0)
            .map_err(|error| DbErr::Migration(format!("read storage_policies.id: {error}")))?;
        let raw = row.try_get_by_index::<String>(1).map_err(|error| {
            DbErr::Migration(format!("read storage_policies.options for #{id}: {error}"))
        })?;

        let Some(normalized) = normalize_remote_upload_strategy_value(&raw)? else {
            continue;
        };

        let value_bind = bind_param(backend, 1);
        let id_bind = bind_param(backend, 2);
        db.execute_raw(Statement::from_sql_and_values(
            backend,
            format!("UPDATE storage_policies SET options = {value_bind} WHERE id = {id_bind}"),
            vec![normalized.into(), id.into()],
        ))
        .await?;
    }

    Ok(())
}

async fn normalize_resource_lock_owner_info(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let db = manager.get_connection();
    let backend = db.get_database_backend();
    let rows = db
        .query_all_raw(Statement::from_string(
            backend,
            "SELECT id, owner_info FROM resource_locks WHERE owner_info IS NOT NULL".to_string(),
        ))
        .await?;

    for row in rows {
        let id = row
            .try_get_by_index::<i64>(0)
            .map_err(|error| DbErr::Migration(format!("read resource_locks.id: {error}")))?;
        let raw = row.try_get_by_index::<String>(1).map_err(|error| {
            DbErr::Migration(format!("read resource_locks.owner_info for #{id}: {error}"))
        })?;
        let normalized = normalize_lock_owner_info_value(&raw)?;

        if normalized == raw {
            continue;
        }

        let owner_info_bind = bind_param(backend, 1);
        let id_bind = bind_param(backend, 2);
        db.execute_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "UPDATE resource_locks SET owner_info = {owner_info_bind} WHERE id = {id_bind}"
            ),
            vec![normalized.into(), id.into()],
        ))
        .await?;
    }

    Ok(())
}

async fn normalize_preview_apps_config(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let db = manager.get_connection();
    let backend = db.get_database_backend();
    let key_ident = quote_ident(backend, "key");
    let key_bind = bind_param(backend, 1);
    let rows = db
        .query_all_raw(Statement::from_sql_and_values(
            backend,
            format!("SELECT id, value FROM system_config WHERE {key_ident} = {key_bind}"),
            vec![PREVIEW_APPS_CONFIG_KEY.into()],
        ))
        .await?;

    for row in rows {
        let id = row
            .try_get_by_index::<i64>(0)
            .map_err(|error| DbErr::Migration(format!("read system_config.id: {error}")))?;
        let raw = row.try_get_by_index::<String>(1).map_err(|error| {
            DbErr::Migration(format!(
                "read system_config.value for preview apps config #{id}: {error}"
            ))
        })?;
        let Some(normalized) = normalize_preview_apps_config_value(&raw)? else {
            continue;
        };

        let value_bind = bind_param(backend, 1);
        let id_bind = bind_param(backend, 2);
        db.execute_raw(Statement::from_sql_and_values(
            backend,
            format!("UPDATE system_config SET value = {value_bind} WHERE id = {id_bind}"),
            vec![normalized.into(), id.into()],
        ))
        .await?;
    }

    Ok(())
}

fn normalize_remote_upload_strategy_value(raw: &str) -> Result<Option<String>, DbErr> {
    let Ok(mut value) = serde_json::from_str::<JsonValue>(raw) else {
        return Ok(None);
    };
    let Some(object) = value.as_object_mut() else {
        return Ok(None);
    };
    let Some(strategy) = object.get_mut("remote_upload_strategy") else {
        return Ok(None);
    };
    if strategy.as_str() != Some("chunked") {
        return Ok(None);
    }

    *strategy = JsonValue::String("presigned".to_string());
    serde_json::to_string(&value).map(Some).map_err(|error| {
        DbErr::Migration(format!(
            "serialize normalized storage policy options: {error}"
        ))
    })
}

fn normalize_lock_owner_info_value(raw: &str) -> Result<String, DbErr> {
    if let Ok(payload) = serde_json::from_str::<LockOwnerInfo>(raw) {
        return serde_json::to_string(&payload).map_err(|error| {
            DbErr::Migration(format!(
                "serialize canonical resource lock owner JSON: {error}"
            ))
        });
    }

    let payload = if xmltree::Element::parse(Cursor::new(raw.as_bytes())).is_ok() {
        LockOwnerInfo::Webdav {
            xml: raw.to_string(),
        }
    } else {
        LockOwnerInfo::Text {
            value: raw.to_string(),
        }
    };

    serde_json::to_string(&payload).map_err(|error| {
        DbErr::Migration(format!(
            "serialize normalized resource lock owner JSON: {error}"
        ))
    })
}

fn normalize_preview_apps_config_value(raw: &str) -> Result<Option<String>, DbErr> {
    let Ok(mut value) = serde_json::from_str::<JsonValue>(raw) else {
        return Ok(None);
    };
    let Some(apps) = value.get_mut("apps").and_then(JsonValue::as_array_mut) else {
        return Ok(None);
    };

    let mut changed = false;
    let original_len = apps.len();
    apps.retain(|app| !is_legacy_wopi_seed_app(app));
    changed |= apps.len() != original_len;

    for app in apps.iter_mut() {
        changed |= normalize_preview_app_labels(app);
    }

    if !changed || apps.is_empty() {
        return Ok(None);
    }

    serde_json::to_string(&value).map(Some).map_err(|error| {
        DbErr::Migration(format!("serialize normalized preview apps config: {error}"))
    })
}

fn normalize_preview_app_labels(app: &mut JsonValue) -> bool {
    let Some(object) = app.as_object_mut() else {
        return false;
    };

    let had_legacy_label_key = object.contains_key("label_i18n_key");
    let legacy_label_key = match object.remove("label_i18n_key") {
        Some(JsonValue::String(value)) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        _ => None,
    };
    let mut changed = had_legacy_label_key;
    let app_key = object
        .get("key")
        .and_then(JsonValue::as_str)
        .map(str::to_string);

    let labels_value = object
        .entry("labels".to_string())
        .or_insert_with(|| JsonValue::Object(JsonMap::new()));
    if !labels_value.is_object() {
        *labels_value = JsonValue::Object(JsonMap::new());
        changed = true;
    }

    let Some(labels) = labels_value.as_object_mut() else {
        return changed;
    };
    let has_labels = labels
        .values()
        .any(|value| value.as_str().is_some_and(|label| !label.trim().is_empty()));
    if has_labels {
        return changed;
    }

    let Some(label_values) =
        legacy_preview_app_labels(app_key.as_deref(), legacy_label_key.as_deref()).or_else(|| {
            legacy_label_key
                .as_deref()
                .map(|value| [("en", value), ("zh", value)])
        })
    else {
        return changed;
    };

    labels.clear();
    for (locale, label) in label_values {
        labels.insert(locale.to_string(), JsonValue::String(label.to_string()));
    }
    true
}

fn legacy_preview_app_labels<'a>(
    app_key: Option<&str>,
    legacy_label_key: Option<&str>,
) -> Option<[(&'static str, &'static str); 2]> {
    app_key
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .and_then(legacy_preview_app_labels_for_value)
        .or_else(|| {
            legacy_label_key
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .and_then(legacy_preview_app_labels_for_value)
        })
}

fn legacy_preview_app_labels_for_value(value: &str) -> Option<[(&'static str, &'static str); 2]> {
    match value {
        "builtin.audio" | "open_with_audio" => Some([("en", "Audio preview"), ("zh", "音频预览")]),
        "builtin.code" | "open_with_code" => Some([("en", "Source view"), ("zh", "源码视图")]),
        "builtin.formatted" | "open_with_formatted" => {
            Some([("en", "Formatted view"), ("zh", "格式化视图")])
        }
        "builtin.image" | "open_with_image" => Some([("en", "Image preview"), ("zh", "图片预览")]),
        "builtin.markdown" | "open_with_markdown" => {
            Some([("en", "Markdown preview"), ("zh", "Markdown 预览")])
        }
        "builtin.office_google" | "open_with_office_google" => {
            Some([("en", "Google Viewer"), ("zh", "Google 预览器")])
        }
        "builtin.office_microsoft" | "open_with_office_microsoft" => {
            Some([("en", "Microsoft Viewer"), ("zh", "Microsoft 预览器")])
        }
        "builtin.pdf" | "open_with_pdf" => Some([("en", "PDF preview"), ("zh", "PDF 预览")]),
        "builtin.table" | "open_with_table" => Some([("en", "Table preview"), ("zh", "表格预览")]),
        "builtin.try_text" | "open_with_try_text" => {
            Some([("en", "Open as text"), ("zh", "以文本方式打开")])
        }
        "builtin.video" | "open_with_video" => Some([("en", "Video preview"), ("zh", "视频预览")]),
        _ => None,
    }
}

fn is_legacy_wopi_seed_app(app: &JsonValue) -> bool {
    let Some(object) = app.as_object() else {
        return false;
    };

    let key = object
        .get("key")
        .and_then(JsonValue::as_str)
        .map(str::trim)
        .unwrap_or_default();
    let provider = object
        .get("provider")
        .and_then(JsonValue::as_str)
        .map(str::trim)
        .unwrap_or_default();
    let extensions_empty = object
        .get("extensions")
        .and_then(JsonValue::as_array)
        .map_or(true, Vec::is_empty);
    let Some(config) = object.get("config").and_then(JsonValue::as_object) else {
        return false;
    };

    provider.eq_ignore_ascii_case("wopi")
        && !key.contains(PREVIEW_APP_DISCOVERY_GENERATED_SEGMENT)
        && key.starts_with("custom.wopi.")
        && extensions_empty
        && config
            .get("discovery_url")
            .and_then(JsonValue::as_str)
            .is_some_and(|value| !value.trim().is_empty())
        && !config
            .get("action")
            .and_then(JsonValue::as_str)
            .is_some_and(|value| !value.trim().is_empty())
        && !config
            .get("action_url")
            .and_then(JsonValue::as_str)
            .is_some_and(|value| !value.trim().is_empty())
        && !config
            .get("action_url_template")
            .and_then(JsonValue::as_str)
            .is_some_and(|value| !value.trim().is_empty())
}

fn quote_ident(backend: DbBackend, ident: &str) -> String {
    match backend {
        DbBackend::MySql => format!("`{}`", ident.replace('`', "``")),
        DbBackend::Postgres | DbBackend::Sqlite => {
            format!("\"{}\"", ident.replace('"', "\"\""))
        }
        _ => format!("\"{}\"", ident.replace('"', "\"\"")),
    }
}

fn bind_param(backend: DbBackend, index: usize) -> String {
    match backend {
        DbBackend::Postgres => format!("${index}"),
        _ => "?".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_preview_apps_config_value;
    use serde_json::{Value, json};

    #[test]
    fn normalizes_preview_app_legacy_label_keys_into_labels() {
        let raw = json!({
            "version": 2,
            "apps": [
                {
                    "key": "builtin.image",
                    "provider": "builtin",
                    "icon": "/static/preview-apps/image.svg",
                    "label_i18n_key": "open_with_image"
                }
            ]
        })
        .to_string();

        let normalized = normalize_preview_apps_config_value(&raw)
            .expect("normalize preview apps config")
            .expect("normalized preview apps config");
        let value: Value = serde_json::from_str(&normalized).expect("parse normalized config");
        let app = &value["apps"][0];

        assert!(app.get("label_i18n_key").is_none());
        assert_eq!(app["labels"]["en"], "Image preview");
        assert_eq!(app["labels"]["zh"], "图片预览");
    }

    #[test]
    fn preserves_existing_labels_while_removing_legacy_label_key() {
        let raw = json!({
            "version": 2,
            "apps": [
                {
                    "key": "custom.viewer",
                    "provider": "url_template",
                    "icon": "/static/preview-apps/file.svg",
                    "label_i18n_key": "legacy_viewer",
                    "labels": {
                        "en": "Viewer"
                    },
                    "config": {
                        "mode": "iframe",
                        "url_template": "https://viewer.example.com/?src={{file_preview_url}}"
                    }
                }
            ]
        })
        .to_string();

        let normalized = normalize_preview_apps_config_value(&raw)
            .expect("normalize preview apps config")
            .expect("normalized preview apps config");
        let value: Value = serde_json::from_str(&normalized).expect("parse normalized config");
        let app = &value["apps"][0];

        assert!(app.get("label_i18n_key").is_none());
        assert_eq!(app["labels"]["en"], "Viewer");
    }

    #[test]
    fn falls_back_to_legacy_label_key_text_for_unknown_preview_app_labels() {
        let raw = json!({
            "version": 2,
            "apps": [
                {
                    "key": "custom.viewer",
                    "provider": "url_template",
                    "icon": "/static/preview-apps/file.svg",
                    "label_i18n_key": "custom_viewer_label",
                    "config": {
                        "mode": "iframe",
                        "url_template": "https://viewer.example.com/?src={{file_preview_url}}"
                    }
                }
            ]
        })
        .to_string();

        let normalized = normalize_preview_apps_config_value(&raw)
            .expect("normalize preview apps config")
            .expect("normalized preview apps config");
        let value: Value = serde_json::from_str(&normalized).expect("parse normalized config");
        let app = &value["apps"][0];

        assert_eq!(app["labels"]["en"], "custom_viewer_label");
        assert_eq!(app["labels"]["zh"], "custom_viewer_label");
    }

    #[test]
    fn removes_legacy_seed_apps_while_keeping_remaining_preview_apps() {
        let raw = json!({
            "version": 2,
            "apps": [
                {
                    "key": "custom.wopi.word",
                    "provider": "wopi",
                    "icon": "/static/preview-apps/file.svg",
                    "labels": {
                        "en": "Word"
                    },
                    "config": {
                        "discovery_url": "https://office.example.com/hosting/discovery"
                    }
                },
                {
                    "key": "builtin.image",
                    "provider": "builtin",
                    "icon": "/static/preview-apps/image.svg",
                    "labels": {
                        "en": "Image preview"
                    }
                }
            ]
        })
        .to_string();

        let normalized = normalize_preview_apps_config_value(&raw)
            .expect("normalize preview apps config")
            .expect("normalized preview apps config");
        let value: Value = serde_json::from_str(&normalized).expect("parse normalized config");
        let apps = value["apps"].as_array().expect("preview apps array");

        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0]["key"], "builtin.image");
    }
}
