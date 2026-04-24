use crate::config::auth_runtime;
use crate::config::branding;
use crate::config::site_url;
use crate::runtime::PrimaryAppState;
use crate::services::preview_app_service;
use crate::types::parse_storage_policy_options;
use serde::Serialize;
use std::collections::BTreeSet;
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::ToSchema;

#[derive(Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct PublicBranding {
    pub title: String,
    pub description: String,
    pub favicon_url: String,
    pub wordmark_dark_url: String,
    pub wordmark_light_url: String,
    pub site_url: Option<String>,
    pub allow_user_registration: bool,
}

pub fn get_public_branding(state: &PrimaryAppState) -> PublicBranding {
    let auth_policy = auth_runtime::RuntimeAuthPolicy::from_runtime_config(&state.runtime_config);
    PublicBranding {
        title: branding::title_or_default(&state.runtime_config),
        description: branding::description_or_default(&state.runtime_config),
        favicon_url: branding::favicon_url_or_default(&state.runtime_config),
        wordmark_dark_url: branding::wordmark_dark_url_or_default(&state.runtime_config),
        wordmark_light_url: branding::wordmark_light_url_or_default(&state.runtime_config),
        site_url: site_url::public_site_url(&state.runtime_config),
        allow_user_registration: auth_policy.allow_user_registration,
    }
}

pub fn get_public_preview_apps(
    state: &PrimaryAppState,
) -> preview_app_service::PublicPreviewAppsConfig {
    preview_app_service::get_public_preview_apps(state)
}

pub fn get_public_thumbnail_support(
    state: &PrimaryAppState,
) -> crate::config::media_processing::PublicThumbnailSupport {
    let mut support =
        crate::config::media_processing::public_thumbnail_support(&state.runtime_config);
    let mut extensions = support.extensions.iter().cloned().collect::<BTreeSet<_>>();

    for policy in state.policy_snapshot.all_policies() {
        let options = parse_storage_policy_options(policy.options.as_ref());
        if !options.uses_storage_native_thumbnail() || options.thumbnail_extensions.is_empty() {
            continue;
        }

        match state.driver_registry.get_driver(&policy) {
            Ok(driver) if driver.as_native_thumbnail().is_some() => {
                extensions.extend(options.thumbnail_extensions);
            }
            Ok(_) => {}
            Err(error) => {
                tracing::debug!(
                    policy_id = policy.id,
                    "skip storage-native thumbnail public support for policy: {error}"
                );
            }
        }
    }

    support.extensions = extensions.into_iter().collect();
    support
}
