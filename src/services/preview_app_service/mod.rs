mod defaults;
mod normalize;
#[cfg(test)]
mod tests;
mod types;

pub use defaults::{default_public_preview_apps, default_public_preview_apps_json};
pub use normalize::{get_public_preview_apps, normalize_public_preview_apps_config_value};
pub use types::{
    PREVIEW_APPS_CONFIG_KEY, PreviewAppProvider, PreviewOpenMode, PublicPreviewAppConfig,
    PublicPreviewAppDefinition, PublicPreviewAppsConfig,
};

const PREVIEW_APPS_VERSION: i32 = 2;
const BUILTIN_TABLE_PREVIEW_APP_KEY: &str = "builtin.table";
const DEFAULT_TABLE_PREVIEW_DELIMITER: &str = "auto";
const PREVIEW_APP_ICON_AUDIO: &str = "/static/preview-apps/audio.svg";
const PREVIEW_APP_ICON_CODE: &str = "/static/preview-apps/code.svg";
const PREVIEW_APP_ICON_FILE: &str = "/static/preview-apps/file.svg";
const PREVIEW_APP_ICON_GOOGLE_DRIVE: &str = "/static/preview-apps/google-drive.svg";
const PREVIEW_APP_ICON_IMAGE: &str = "/static/preview-apps/image.svg";
const PREVIEW_APP_ICON_JSON: &str = "/static/preview-apps/json.svg";
const PREVIEW_APP_ICON_MARKDOWN: &str = "/static/preview-apps/markdown.svg";
const PREVIEW_APP_ICON_MICROSOFT_ONEDRIVE: &str = "/static/preview-apps/microsoft-onedrive.svg";
const PREVIEW_APP_ICON_PDF: &str = "/static/preview-apps/pdf.svg";
const PREVIEW_APP_ICON_TABLE: &str = "/static/preview-apps/table.svg";
const PREVIEW_APP_ICON_VIDEO: &str = "/static/preview-apps/video.svg";

const REQUIRED_BUILTIN_PREVIEW_APP_KEYS: &[&str] = &[
    "builtin.image",
    "builtin.video",
    "builtin.audio",
    "builtin.pdf",
    "builtin.markdown",
    BUILTIN_TABLE_PREVIEW_APP_KEY,
    "builtin.formatted",
    "builtin.code",
    "builtin.try_text",
];

const fn default_preview_apps_version() -> i32 {
    PREVIEW_APPS_VERSION
}

const fn default_true() -> bool {
    true
}

fn is_table_preview_app_key(key: &str) -> bool {
    key.trim() == BUILTIN_TABLE_PREVIEW_APP_KEY
}

fn is_required_builtin_preview_app_key(key: &str) -> bool {
    REQUIRED_BUILTIN_PREVIEW_APP_KEYS.contains(&key)
}
