mod audit;
mod microsoft;
mod provider;
#[cfg(test)]
mod tests;

use chrono::{Duration, Utc};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ConnectionTrait, TransactionTrait};
use serde::{Deserialize, Serialize};
use std::fmt;

use crate::db::repository::{
    policy_repo, storage_policy_authorization_flow_repo, storage_policy_credential_repo,
};
use crate::entities::{storage_policy_authorization_flow, storage_policy_credential};
use crate::errors::{AsterError, MapAsterErr, Result};
use crate::runtime::SharedRuntimeState;
use crate::services::audit_service::{AuditContext, AuditRequestInfo};
use crate::storage::drivers::onedrive::{MicrosoftGraphClient, MicrosoftGraphClientConfig};
use crate::types::{
    StorageAuthorizationFlowStatus, StorageCredentialKind, StorageCredentialProvider,
    StorageCredentialStatus, parse_storage_policy_options,
};
use crate::utils::id;

use super::{
    FLOW_TTL_SECS, MicrosoftGraphApplicationConfigInput, MicrosoftGraphAuthorizationContext,
    MicrosoftGraphAuthorizationInput, StoragePolicyCredentialInfo, crypto,
    normalize_optional_string, normalize_required_string, normalize_scopes,
    resolve_onedrive_location, scopes_to_json,
};
use audit::{
    OAUTH_AUDIT_EVENT_AUTHORIZATION_COMPLETED, OAUTH_AUDIT_EVENT_AUTHORIZATION_FAILED,
    OAUTH_AUDIT_EVENT_AUTHORIZATION_STARTED, OAUTH_AUDIT_RESULT_FAILED, OAUTH_AUDIT_RESULT_SUCCESS,
    StorageCredentialOauthAuditDetails, log_storage_credential_oauth_audit,
};
use microsoft::{
    MicrosoftGraphFlowContext, build_pkce_challenge, build_pkce_verifier,
    decrypt_stored_client_secret, exchange_microsoft_graph_code, flow_client_secret_aad,
    metadata_cloud, metadata_string, microsoft_authorization_url, microsoft_graph_flow_cloud,
    microsoft_graph_flow_tenant, parse_metadata,
};

pub(crate) use microsoft::{StorageCredentialMetadataInput, storage_credential_metadata};
pub(crate) use provider::{
    MicrosoftGraphCleanupTokenSnapshot, build_microsoft_graph_cleanup_token_provider,
    build_microsoft_graph_credential_token_provider,
};

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(utoipa::ToSchema))]
pub struct StorageAuthorizationStartInput {
    pub provider: StorageCredentialProvider,
    pub microsoft_graph: Option<MicrosoftGraphAuthorizationInput>,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(utoipa::ToSchema))]
pub struct StorageAuthorizationStartResponse {
    pub authorization_url: String,
    pub expires_in: u64,
    pub provider: StorageCredentialProvider,
    pub microsoft_graph: Option<MicrosoftGraphAuthorizationContext>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(
    all(debug_assertions, feature = "openapi"),
    derive(utoipa::IntoParams, utoipa::ToSchema)
)]
pub struct StorageAuthorizationCallbackQuery {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
    pub error_description: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(utoipa::ToSchema))]
pub struct StorageAuthorizationCallbackOutcome {
    pub credential: StoragePolicyCredentialInfo,
}

pub(crate) async fn upsert_microsoft_graph_application_config<C: ConnectionTrait>(
    db: &C,
    encryption_key: &str,
    policy_id: i64,
    input: MicrosoftGraphApplicationConfigInput,
) -> Result<Option<storage_policy_credential::Model>> {
    let existing = storage_policy_credential_repo::find_by_policy_provider_kind(
        db,
        policy_id,
        StorageCredentialProvider::MicrosoftGraph,
        StorageCredentialKind::OauthDelegated,
    )
    .await?;
    let existing_metadata = existing
        .as_ref()
        .and_then(|credential| parse_metadata(&credential.metadata));
    if existing.is_none()
        && input.cloud.is_none()
        && normalize_optional_string(input.tenant.clone()).is_none()
        && normalize_optional_string(input.client_id.clone()).is_none()
        && normalize_optional_string(input.client_secret.clone()).is_none()
        && input
            .scopes
            .as_ref()
            .is_none_or(|scopes| scopes.iter().all(|scope| scope.trim().is_empty()))
    {
        return Ok(None);
    }

    let cloud = input
        .cloud
        .or_else(|| existing_metadata.as_ref().and_then(metadata_cloud))
        .unwrap_or_default();
    let tenant = normalize_optional_string(input.tenant)
        .or_else(|| {
            existing
                .as_ref()
                .and_then(|credential| credential.tenant_id.clone())
        })
        .unwrap_or_else(|| "common".to_string());
    let client_id = normalize_optional_string(input.client_id).or_else(|| {
        existing_metadata
            .as_ref()
            .and_then(|metadata| metadata_string(metadata, "client_id"))
    });
    let client_secret = normalize_optional_string(input.client_secret);
    let existing_client_secret_ciphertext = existing_metadata
        .as_ref()
        .and_then(|metadata| metadata_string(metadata, "client_secret_ciphertext"));
    let scopes = match input.scopes {
        Some(scopes) => normalize_scopes(Some(scopes)),
        None => existing
            .as_ref()
            .map(|credential| super::parse_scopes_json(&credential.scopes))
            .filter(|scopes| !scopes.is_empty())
            .unwrap_or_else(|| normalize_scopes(None)),
    };
    let metadata = microsoft_graph_application_metadata(
        existing_metadata.as_ref(),
        encryption_key,
        policy_id,
        cloud,
        client_id.as_deref(),
        client_secret.as_deref(),
        existing_client_secret_ciphertext.as_deref(),
    )?;
    let now = Utc::now();

    if let Some(credential) = existing {
        let mut active: storage_policy_credential::ActiveModel = credential.into();
        active.tenant_id = Set(Some(tenant));
        active.scopes = Set(scopes_to_json(&scopes)?);
        active.metadata = Set(metadata);
        active.updated_at = Set(now);
        return active.update(db).await.map(Some).map_err(AsterError::from);
    }

    storage_policy_credential_repo::upsert_by_policy_provider_kind(
        db,
        storage_policy_credential::ActiveModel {
            policy_id: Set(policy_id),
            provider: Set(StorageCredentialProvider::MicrosoftGraph),
            credential_kind: Set(StorageCredentialKind::OauthDelegated),
            account_label: Set(None),
            subject: Set(None),
            tenant_id: Set(Some(tenant)),
            scopes: Set(scopes_to_json(&scopes)?),
            access_token_ciphertext: Set(None),
            refresh_token_ciphertext: Set(None),
            metadata: Set(metadata),
            status: Set(StorageCredentialStatus::Invalid),
            status_reason: Set(Some("authorization_required".to_string())),
            expires_at: Set(None),
            authorized_at: Set(None),
            last_refreshed_at: Set(None),
            last_validated_at: Set(None),
            ..Default::default()
        },
        now,
    )
    .await
    .map(Some)
}

fn microsoft_graph_application_metadata(
    existing_metadata: Option<&serde_json::Value>,
    encryption_key: &str,
    policy_id: i64,
    cloud: crate::types::MicrosoftGraphCloud,
    client_id: Option<&str>,
    client_secret: Option<&str>,
    existing_client_secret_ciphertext: Option<&str>,
) -> Result<String> {
    let mut metadata = existing_metadata
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    metadata["cloud"] = serde_json::json!(cloud);
    metadata["graph_base_url"] = serde_json::Value::String(cloud.graph_base_url().to_string());
    if let Some(client_id) = client_id {
        metadata["client_id"] = serde_json::Value::String(client_id.to_string());
    }
    if let Some(client_secret) = client_secret {
        let ciphertext =
            microsoft::encrypt_stored_client_secret(encryption_key, policy_id, client_secret)?;
        metadata["client_secret_configured"] = serde_json::Value::Bool(true);
        metadata["client_secret_ciphertext"] = serde_json::Value::String(ciphertext);
    } else if let Some(ciphertext) = existing_client_secret_ciphertext {
        metadata["client_secret_configured"] = serde_json::Value::Bool(true);
        metadata["client_secret_ciphertext"] = serde_json::Value::String(ciphertext.to_string());
    } else {
        metadata["client_secret_configured"] = serde_json::Value::Bool(false);
        metadata
            .as_object_mut()
            .map(|object| object.remove("client_secret_ciphertext"));
    }
    serde_json::to_string(&metadata).map_aster_err_ctx(
        "failed to serialize Microsoft Graph application metadata",
        AsterError::internal_error,
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StorageAuthorizationFailureReason {
    InvalidState,
    ProviderError,
    TokenExchangeFailed,
    DriveResolutionFailed,
    InvalidRequest,
    ServerError,
    UnsupportedProvider,
}

impl StorageAuthorizationFailureReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidState => "invalid_state",
            Self::ProviderError => "provider_error",
            Self::TokenExchangeFailed => "token_exchange_failed",
            Self::DriveResolutionFailed => "drive_resolution_failed",
            Self::InvalidRequest => "invalid_request",
            Self::ServerError => "server_error",
            Self::UnsupportedProvider => "unsupported_provider",
        }
    }
}

#[derive(Debug)]
pub struct StorageAuthorizationCallbackError {
    reason: StorageAuthorizationFailureReason,
    source: AsterError,
}

impl StorageAuthorizationCallbackError {
    fn new(reason: StorageAuthorizationFailureReason, source: AsterError) -> Self {
        Self { reason, source }
    }

    pub const fn reason(&self) -> StorageAuthorizationFailureReason {
        self.reason
    }

    pub fn source(&self) -> &AsterError {
        &self.source
    }
}

impl fmt::Display for StorageAuthorizationCallbackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.reason.as_str(), self.source)
    }
}

impl std::error::Error for StorageAuthorizationCallbackError {}

pub async fn start_authorization(
    state: &impl SharedRuntimeState,
    req: &actix_web::HttpRequest,
    policy_id: i64,
    created_by_user_id: i64,
    input: StorageAuthorizationStartInput,
) -> Result<StorageAuthorizationStartResponse> {
    let policy = policy_repo::find_by_id(state.writer_db(), policy_id).await?;
    crate::storage::connectors::ensure_storage_authorization_supported(
        policy.driver_type,
        input.provider,
    )?;
    match input.provider {
        StorageCredentialProvider::MicrosoftGraph => {
            start_microsoft_graph_authorization(
                state,
                req,
                created_by_user_id,
                policy,
                input.microsoft_graph,
            )
            .await
        }
        StorageCredentialProvider::GoogleDrive => Err(AsterError::unsupported_driver(
            "Google Drive storage credential authorization is not implemented yet",
        )),
    }
}

async fn start_microsoft_graph_authorization(
    state: &impl SharedRuntimeState,
    req: &actix_web::HttpRequest,
    created_by_user_id: i64,
    policy: crate::entities::storage_policy::Model,
    input: Option<MicrosoftGraphAuthorizationInput>,
) -> Result<StorageAuthorizationStartResponse> {
    let input = input.unwrap_or_default();
    let policy_id = policy.id;
    let existing_credential = storage_policy_credential_repo::find_by_policy_provider_kind(
        state.writer_db(),
        policy_id,
        StorageCredentialProvider::MicrosoftGraph,
        StorageCredentialKind::OauthDelegated,
    )
    .await?;
    let existing_metadata = existing_credential
        .as_ref()
        .and_then(|credential| parse_metadata(&credential.metadata));
    let options = parse_storage_policy_options(policy.options.as_ref());
    let cloud = input
        .cloud
        .or_else(|| existing_metadata.as_ref().and_then(metadata_cloud))
        .or(options.onedrive_cloud)
        .unwrap_or_default();
    let tenant = normalize_optional_string(input.tenant)
        .or_else(|| {
            existing_credential
                .as_ref()
                .and_then(|credential| credential.tenant_id.clone())
        })
        .or_else(|| options.onedrive_tenant.clone())
        .unwrap_or_else(|| "common".to_string());
    let client_id = match normalize_optional_string(input.client_id)
        .or_else(|| {
            existing_metadata
                .as_ref()
                .and_then(|metadata| metadata_string(metadata, "client_id"))
        })
        .or_else(|| normalize_optional_string(Some(policy.access_key.clone())))
    {
        Some(client_id) => normalize_required_string(&client_id, "client_id", 512)?,
        None => return Err(AsterError::validation_error("client_id is required")),
    };
    let client_secret = match normalize_optional_string(input.client_secret) {
        Some(client_secret) => Some(client_secret),
        None => existing_metadata
            .as_ref()
            .and_then(|metadata| metadata_string(metadata, "client_secret_ciphertext"))
            .map(|ciphertext| {
                decrypt_stored_client_secret(
                    &state.config().auth.storage_credential_secret_key,
                    policy_id,
                    &ciphertext,
                )
            })
            .transpose()?
            .or_else(|| normalize_optional_string(Some(policy.secret_key.clone()))),
    };
    let client_secret = client_secret
        .map(|client_secret| normalize_required_string(&client_secret, "client_secret", 2048))
        .transpose()?
        .ok_or_else(|| {
            // AsterDrive stores OneDrive as a server-side backend. Treat the Microsoft app
            // as a confidential client so background refresh cannot silently fall back to
            // public-client OAuth semantics.
            AsterError::validation_error(
                "client_secret is required for Microsoft Graph storage authorization",
            )
        })?;
    let default_scopes = super::default_microsoft_graph_scopes_for_onedrive_options(&options);
    let scopes = match input.scopes {
        Some(scopes) => super::normalize_scopes_with_default(Some(scopes), default_scopes),
        None => existing_credential
            .as_ref()
            .map(|credential| super::parse_scopes_json(&credential.scopes))
            .filter(|scopes| !scopes.is_empty())
            .unwrap_or_else(|| super::normalize_scopes_with_default(None, default_scopes)),
    };
    let redirect_uri = callback_redirect_uri(state, req)?;
    let state_value = format!("storage_oauth_{}", id::new_short_token());
    let pkce_verifier = build_pkce_verifier();
    let pkce_challenge = build_pkce_challenge(&pkce_verifier);
    let authorization_url = microsoft_authorization_url(
        cloud,
        &tenant,
        &client_id,
        &redirect_uri,
        &scopes,
        &state_value,
        &pkce_challenge,
    )?;
    let state_hash = crypto::token_hash(&state_value);
    let client_secret_ciphertext = Some(crypto::encrypt_token(
        &state.config().auth.storage_credential_secret_key,
        flow_client_secret_aad(policy_id, &state_hash).as_bytes(),
        &client_secret,
    )?);
    let context = MicrosoftGraphFlowContext {
        cloud,
        tenant: tenant.clone(),
        client_id: client_id.clone(),
        client_secret_ciphertext,
        scopes: scopes.clone(),
    };
    let now = Utc::now();
    let ttl = crate::utils::numbers::u64_to_i64(FLOW_TTL_SECS, "storage authorization flow ttl")?;
    storage_policy_authorization_flow_repo::cancel_pending_for_policy(
        state.writer_db(),
        policy_id,
        now,
    )
    .await?;
    storage_policy_authorization_flow_repo::create(
        state.writer_db(),
        storage_policy_authorization_flow::ActiveModel {
            provider: Set(StorageCredentialProvider::MicrosoftGraph),
            policy_id: Set(Some(policy_id)),
            created_by_user_id: Set(created_by_user_id),
            state_hash: Set(state_hash),
            pkce_verifier: Set(Some(pkce_verifier)),
            redirect_uri: Set(redirect_uri),
            scopes: Set(scopes_to_json(&scopes)?),
            context: Set(serde_json::to_string(&context).map_aster_err_ctx(
                "failed to serialize Microsoft Graph authorization context",
                AsterError::internal_error,
            )?),
            status: Set(StorageAuthorizationFlowStatus::Pending),
            created_at: Set(now),
            expires_at: Set(now + Duration::seconds(ttl)),
            consumed_at: Set(None),
            ..Default::default()
        },
    )
    .await?;
    log_storage_credential_oauth_audit(
        state,
        &AuditRequestInfo::from_request(req).to_context(created_by_user_id),
        StorageCredentialOauthAuditDetails {
            event: OAUTH_AUDIT_EVENT_AUTHORIZATION_STARTED,
            result: OAUTH_AUDIT_RESULT_SUCCESS,
            policy_id: Some(policy_id),
            cloud: Some(cloud),
            tenant: Some(&tenant),
            client_secret_configured: Some(true),
            ..Default::default()
        },
    )
    .await;

    Ok(StorageAuthorizationStartResponse {
        authorization_url,
        expires_in: FLOW_TTL_SECS,
        provider: StorageCredentialProvider::MicrosoftGraph,
        microsoft_graph: Some(MicrosoftGraphAuthorizationContext {
            cloud,
            tenant,
            client_id,
            client_secret_configured: true,
            scopes,
        }),
    })
}

pub async fn finish_authorization_callback(
    state: &impl SharedRuntimeState,
    query: &StorageAuthorizationCallbackQuery,
) -> std::result::Result<StorageAuthorizationCallbackOutcome, StorageAuthorizationCallbackError> {
    if let Some(error) = query.error.as_deref() {
        let description = query
            .error_description
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(error);
        log_storage_credential_oauth_audit(
            state,
            &AuditContext::system(),
            StorageCredentialOauthAuditDetails {
                event: OAUTH_AUDIT_EVENT_AUTHORIZATION_FAILED,
                result: OAUTH_AUDIT_RESULT_FAILED,
                reason: Some(StorageAuthorizationFailureReason::ProviderError.as_str()),
                ..Default::default()
            },
        )
        .await;
        return Err(StorageAuthorizationCallbackError::new(
            StorageAuthorizationFailureReason::ProviderError,
            AsterError::auth_invalid_credentials(format!(
                "storage credential provider returned error: {description}"
            )),
        ));
    }
    let code = match query.code.as_deref() {
        Some(code) => code,
        None => {
            log_storage_credential_oauth_audit(
                state,
                &AuditContext::system(),
                StorageCredentialOauthAuditDetails {
                    event: OAUTH_AUDIT_EVENT_AUTHORIZATION_FAILED,
                    result: OAUTH_AUDIT_RESULT_FAILED,
                    reason: Some(StorageAuthorizationFailureReason::InvalidRequest.as_str()),
                    ..Default::default()
                },
            )
            .await;
            return Err(StorageAuthorizationCallbackError::new(
                StorageAuthorizationFailureReason::InvalidRequest,
                AsterError::auth_invalid_credentials("storage credential callback missing code"),
            ));
        }
    };
    let state_value = match query.state.as_deref() {
        Some(state_value) => state_value,
        None => {
            log_storage_credential_oauth_audit(
                state,
                &AuditContext::system(),
                StorageCredentialOauthAuditDetails {
                    event: OAUTH_AUDIT_EVENT_AUTHORIZATION_FAILED,
                    result: OAUTH_AUDIT_RESULT_FAILED,
                    reason: Some(StorageAuthorizationFailureReason::InvalidRequest.as_str()),
                    ..Default::default()
                },
            )
            .await;
            return Err(StorageAuthorizationCallbackError::new(
                StorageAuthorizationFailureReason::InvalidRequest,
                AsterError::auth_invalid_credentials("storage credential callback missing state"),
            ));
        }
    };

    let txn = state
        .writer_db()
        .begin()
        .await
        .map_err(|error| storage_authorization_callback_server_error(error.into()))?;
    let now = Utc::now();
    let flow = match storage_policy_authorization_flow_repo::consume_by_state_hash(
        &txn,
        &crypto::token_hash(state_value),
        now,
    )
    .await
    .map_err(storage_authorization_callback_server_error)?
    {
        Some(flow) => flow,
        None => {
            let _ = txn.rollback().await;
            log_storage_credential_oauth_audit(
                state,
                &AuditContext::system(),
                StorageCredentialOauthAuditDetails {
                    event: OAUTH_AUDIT_EVENT_AUTHORIZATION_FAILED,
                    result: OAUTH_AUDIT_RESULT_FAILED,
                    reason: Some(StorageAuthorizationFailureReason::InvalidState.as_str()),
                    ..Default::default()
                },
            )
            .await;
            return Err(StorageAuthorizationCallbackError::new(
                StorageAuthorizationFailureReason::InvalidState,
                AsterError::auth_invalid_credentials(
                    "storage credential state is invalid or expired",
                ),
            ));
        }
    };
    let flow_policy_id = flow.policy_id;
    let flow_user_id = flow.created_by_user_id;
    let flow_cloud = microsoft_graph_flow_cloud(&flow);
    let flow_tenant = microsoft_graph_flow_tenant(&flow);
    let policy_id = match flow.policy_id {
        Some(policy_id) => policy_id,
        None => {
            let _ = txn.rollback().await;
            return Err(storage_authorization_callback_server_error(
                AsterError::database_operation("storage authorization flow missing policy_id"),
            ));
        }
    };
    let policy = match policy_repo::find_by_id(&txn, policy_id)
        .await
        .map_err(storage_authorization_callback_server_error)
    {
        Ok(policy) => policy,
        Err(error) => {
            let _ = txn.rollback().await;
            return Err(error);
        }
    };
    if let Err(error) = crate::storage::connectors::ensure_storage_authorization_supported(
        policy.driver_type,
        flow.provider,
    )
    .map_err(|error| {
        StorageAuthorizationCallbackError::new(
            StorageAuthorizationFailureReason::UnsupportedProvider,
            error,
        )
    }) {
        let _ = txn.rollback().await;
        log_storage_credential_oauth_audit(
            state,
            &AuditContext {
                user_id: flow_user_id,
                ip_address: None,
                user_agent: None,
            },
            StorageCredentialOauthAuditDetails {
                event: OAUTH_AUDIT_EVENT_AUTHORIZATION_FAILED,
                result: OAUTH_AUDIT_RESULT_FAILED,
                policy_id: flow_policy_id,
                cloud: flow_cloud,
                tenant: flow_tenant.as_deref(),
                reason: Some(error.reason().as_str()),
                ..Default::default()
            },
        )
        .await;
        return Err(error);
    }
    let policy_options = parse_storage_policy_options(policy.options.as_ref());
    // Keep Microsoft Graph token exchange and drive resolution outside the DB
    // transaction; provider latency must not hold SQLite/MySQL/Postgres locks.
    txn.commit()
        .await
        .map_err(|error| storage_authorization_callback_server_error(error.into()))?;
    let now = Utc::now();
    let credential_result = finish_authorization_provider_callback(
        &state.config().auth.storage_credential_secret_key,
        &flow,
        &policy_options,
        code,
        now,
    )
    .await;
    let credential = match credential_result {
        Ok(credential) => {
            let txn = state
                .writer_db()
                .begin()
                .await
                .map_err(|error| storage_authorization_callback_server_error(error.into()))?;
            let credential = match storage_policy_credential_repo::upsert_by_policy_provider_kind(
                &txn, credential, now,
            )
            .await
            .map_err(storage_authorization_callback_server_error)
            {
                Ok(credential) => credential,
                Err(error) => {
                    let _ = txn.rollback().await;
                    return Err(error);
                }
            };
            txn.commit()
                .await
                .map_err(|error| storage_authorization_callback_server_error(error.into()))?;
            credential
        }
        Err(error) => {
            let reason = error.reason().as_str();
            log_storage_credential_oauth_audit(
                state,
                &AuditContext {
                    user_id: flow_user_id,
                    ip_address: None,
                    user_agent: None,
                },
                StorageCredentialOauthAuditDetails {
                    event: OAUTH_AUDIT_EVENT_AUTHORIZATION_FAILED,
                    result: OAUTH_AUDIT_RESULT_FAILED,
                    policy_id: flow_policy_id,
                    cloud: flow_cloud,
                    tenant: flow_tenant.as_deref(),
                    reason: Some(reason),
                    ..Default::default()
                },
            )
            .await;
            return Err(error);
        }
    };
    state
        .driver_registry()
        .reload_storage_policy_credentials(state.writer_db(), state.config().as_ref())
        .await
        .map_err(storage_authorization_callback_server_error)?;
    log_storage_credential_oauth_audit(
        state,
        &AuditContext {
            user_id: flow_user_id,
            ip_address: None,
            user_agent: None,
        },
        StorageCredentialOauthAuditDetails {
            event: OAUTH_AUDIT_EVENT_AUTHORIZATION_COMPLETED,
            result: OAUTH_AUDIT_RESULT_SUCCESS,
            policy_id: flow_policy_id,
            cloud: flow_cloud,
            tenant: flow_tenant.as_deref(),
            ..Default::default()
        },
    )
    .await;
    Ok(StorageAuthorizationCallbackOutcome {
        credential: credential.into(),
    })
}

fn storage_authorization_callback_server_error(
    error: AsterError,
) -> StorageAuthorizationCallbackError {
    StorageAuthorizationCallbackError::new(StorageAuthorizationFailureReason::ServerError, error)
}

async fn finish_microsoft_graph_callback(
    encryption_key: &str,
    flow: &storage_policy_authorization_flow::Model,
    options: &crate::types::StoragePolicyOptions,
    code: &str,
    now: chrono::DateTime<Utc>,
) -> std::result::Result<storage_policy_credential::ActiveModel, StorageAuthorizationCallbackError>
{
    let policy_id = flow.policy_id.ok_or_else(|| {
        storage_authorization_callback_server_error(AsterError::database_operation(
            "storage authorization flow missing policy_id",
        ))
    })?;
    let context =
        serde_json::from_str::<MicrosoftGraphFlowContext>(&flow.context).map_err(|err| {
            storage_authorization_callback_server_error(AsterError::database_operation(format!(
                "invalid Microsoft Graph authorization context: {err}"
            )))
        })?;
    let pkce_verifier = flow.pkce_verifier.as_deref().ok_or_else(|| {
        storage_authorization_callback_server_error(AsterError::database_operation(
            "storage authorization flow missing PKCE verifier",
        ))
    })?;
    let client_secret = match context.client_secret_ciphertext.as_deref() {
        Some(ciphertext) => crypto::decrypt_token(
            encryption_key,
            flow_client_secret_aad(policy_id, &flow.state_hash).as_bytes(),
            ciphertext,
        )
        .map_err(storage_authorization_callback_server_error)?,
        None => {
            return Err(StorageAuthorizationCallbackError::new(
                StorageAuthorizationFailureReason::InvalidRequest,
                AsterError::validation_error(
                    "client_secret is required for Microsoft Graph storage authorization",
                ),
            ));
        }
    };
    let token = exchange_microsoft_graph_code(
        &context,
        Some(client_secret.as_str()),
        code,
        &flow.redirect_uri,
        pkce_verifier,
    )
    .await
    .map_err(|error| {
        StorageAuthorizationCallbackError::new(
            StorageAuthorizationFailureReason::TokenExchangeFailed,
            error,
        )
    })?;
    let graph_client = MicrosoftGraphClient::new(MicrosoftGraphClientConfig::new(
        context.cloud.graph_base_url(),
        token.access_token.clone(),
    ))
    .map_err(storage_authorization_callback_server_error)?;
    let location = resolve_onedrive_location(&graph_client, options)
        .await
        .map_err(|error| {
            StorageAuthorizationCallbackError::new(
                StorageAuthorizationFailureReason::DriveResolutionFailed,
                error,
            )
        })?;
    let root_item = location.root_item;
    let expires_at = token
        .expires_in
        .and_then(|seconds| (seconds > 0).then(|| now + Duration::seconds(seconds)));
    let granted_scopes = token
        .scope
        .as_deref()
        .map(|scope| {
            normalize_scopes(Some(
                scope.split_whitespace().map(ToOwned::to_owned).collect(),
            ))
        })
        .filter(|scopes| !scopes.is_empty())
        .unwrap_or_else(|| context.scopes.clone());
    let access_aad = crypto::token_aad(
        policy_id,
        StorageCredentialProvider::MicrosoftGraph.as_str(),
        "access",
    );
    let refresh_aad = crypto::token_aad(
        policy_id,
        StorageCredentialProvider::MicrosoftGraph.as_str(),
        "refresh",
    );
    let access_token_ciphertext =
        crypto::encrypt_token(encryption_key, access_aad.as_bytes(), &token.access_token)
            .map_err(storage_authorization_callback_server_error)?;
    let refresh_token_ciphertext = match token.refresh_token.as_deref() {
        Some(refresh_token) if !refresh_token.trim().is_empty() => Some(
            crypto::encrypt_token(encryption_key, refresh_aad.as_bytes(), refresh_token)
                .map_err(storage_authorization_callback_server_error)?,
        ),
        _ => None,
    };
    Ok(storage_policy_credential::ActiveModel {
        policy_id: Set(policy_id),
        provider: Set(StorageCredentialProvider::MicrosoftGraph),
        credential_kind: Set(StorageCredentialKind::OauthDelegated),
        account_label: Set(root_item.name.clone()),
        subject: Set(Some(root_item.id.clone())),
        tenant_id: Set(Some(context.tenant.clone())),
        scopes: Set(
            scopes_to_json(&granted_scopes).map_err(storage_authorization_callback_server_error)?
        ),
        access_token_ciphertext: Set(Some(access_token_ciphertext)),
        refresh_token_ciphertext: Set(refresh_token_ciphertext),
        metadata: Set(storage_credential_metadata(StorageCredentialMetadataInput {
            encryption_key,
            policy_id,
            cloud: context.cloud,
            client_id: Some(&context.client_id),
            client_secret: Some(client_secret.as_str()),
            client_secret_ciphertext: None,
            drive_id: &location.drive_id,
            root_item_id: &root_item.id,
            root_item_name: root_item.name.as_deref(),
            id_token: token.id_token.as_deref(),
        })
        .map_err(storage_authorization_callback_server_error)?),
        status: Set(StorageCredentialStatus::Authorized),
        status_reason: Set(None),
        expires_at: Set(expires_at),
        authorized_at: Set(Some(now)),
        last_refreshed_at: Set(None),
        last_validated_at: Set(None),
        ..Default::default()
    })
}

async fn finish_authorization_provider_callback(
    encryption_key: &str,
    flow: &storage_policy_authorization_flow::Model,
    options: &crate::types::StoragePolicyOptions,
    code: &str,
    now: chrono::DateTime<Utc>,
) -> std::result::Result<storage_policy_credential::ActiveModel, StorageAuthorizationCallbackError>
{
    // Provider protocol handling stays in storage_credential_service; the
    // connector layer only decides whether the policy is allowed to use it.
    match flow.provider {
        StorageCredentialProvider::MicrosoftGraph => {
            finish_microsoft_graph_callback(encryption_key, flow, options, code, now).await
        }
        StorageCredentialProvider::GoogleDrive => Err(StorageAuthorizationCallbackError::new(
            StorageAuthorizationFailureReason::UnsupportedProvider,
            AsterError::unsupported_driver(
                "Google Drive storage credential authorization is not implemented yet",
            ),
        )),
    }
}

fn callback_redirect_uri(
    state: &impl SharedRuntimeState,
    req: &actix_web::HttpRequest,
) -> Result<String> {
    let conn = req.connection_info();
    let uri = crate::config::site_url::public_app_url_for_request(
        state.runtime_config(),
        "/api/v1/admin/policies/storage-authorization/callback",
        conn.scheme(),
        conn.host(),
    )
    .ok_or_else(|| {
        AsterError::validation_error(
            "cannot build storage credential callback redirect URI; configure public_site_url",
        )
    })?;
    if uri.starts_with('/') {
        return Err(AsterError::validation_error(
            "storage credential callback redirect URI must be absolute; configure public_site_url",
        ));
    }
    Ok(uri)
}
