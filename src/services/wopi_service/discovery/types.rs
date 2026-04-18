use chrono::{DateTime, Utc};

use crate::services::preview_app_service;
use crate::services::wopi_service::proof::WopiProofKeySet;

#[derive(Debug, Clone)]
pub(crate) struct WopiAppConfig {
    pub(crate) action: String,
    pub(crate) action_url: Option<String>,
    pub(crate) discovery_url: Option<String>,
    pub(crate) form_fields: std::collections::BTreeMap<String, String>,
    pub(crate) mode: preview_app_service::PreviewOpenMode,
}

#[derive(Debug, Clone)]
pub(super) struct WopiDiscoveryAction {
    pub(super) action: String,
    pub(super) app_icon_url: Option<String>,
    pub(super) app_name: Option<String>,
    pub(super) ext: Option<String>,
    pub(super) mime: Option<String>,
    pub(super) urlsrc: String,
}

#[derive(Debug, Clone)]
pub(crate) struct WopiDiscovery {
    pub(super) actions: Vec<WopiDiscoveryAction>,
    pub(super) proof_keys: Option<WopiProofKeySet>,
}

#[derive(Debug, Clone)]
pub(super) struct CachedWopiDiscovery {
    pub(super) discovery: WopiDiscovery,
    pub(super) cached_at: DateTime<Utc>,
}
