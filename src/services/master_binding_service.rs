//! 服务模块：`master_binding_service`。

use crate::db::repository::{master_binding_repo, policy_repo};
use crate::entities::{master_binding, storage_policy};
use crate::errors::{AsterError, Result, precondition_failed_with_subcode};
use crate::runtime::FollowerRuntimeState;
use crate::storage::remote_protocol::{
    INTERNAL_AUTH_ACCESS_KEY_HEADER, INTERNAL_AUTH_NONCE_HEADER, INTERNAL_AUTH_NONCE_TTL_SECS,
    INTERNAL_AUTH_SIGNATURE_HEADER, INTERNAL_AUTH_SKEW_SECS, INTERNAL_AUTH_TIMESTAMP_HEADER,
    PRESIGNED_AUTH_ACCESS_KEY_QUERY, PRESIGNED_AUTH_EXPIRES_QUERY, PRESIGNED_AUTH_SIGNATURE_QUERY,
    normalize_remote_base_url, sign_presigned_request,
};
use chrono::Utc;
use hmac::{Hmac, KeyInit, Mac};
use sea_orm::{ConnectionTrait, Set};
use sha2::Sha256;

#[derive(Debug, Clone)]
pub struct AuthorizedMasterBinding {
    pub binding: master_binding::Model,
    pub ingress_policy: storage_policy::Model,
}

#[derive(Debug, Clone)]
pub struct UpsertMasterBindingInput {
    pub name: String,
    pub master_url: String,
    pub access_key: String,
    pub secret_key: String,
    pub namespace: String,
    pub ingress_policy_id: i64,
    pub is_enabled: bool,
}

#[derive(Debug, Clone)]
pub struct SyncMasterBindingInput {
    pub name: String,
    pub namespace: String,
    pub is_enabled: bool,
}

pub async fn upsert_from_enrollment<C: ConnectionTrait>(
    db: &C,
    input: UpsertMasterBindingInput,
) -> Result<(master_binding::Model, &'static str)> {
    let normalized = normalize_upsert_input(db, input).await?;
    let now = Utc::now();

    match master_binding_repo::find_by_access_key(db, &normalized.access_key).await? {
        Some(existing) => {
            let mut active: master_binding::ActiveModel = existing.into();
            active.name = Set(normalized.name);
            active.master_url = Set(normalized.master_url);
            active.secret_key = Set(normalized.secret_key);
            active.namespace = Set(normalized.namespace);
            active.ingress_policy_id = Set(normalized.ingress_policy_id);
            active.is_enabled = Set(normalized.is_enabled);
            active.updated_at = Set(now);
            let updated = master_binding_repo::update(db, active).await?;
            Ok((updated, "updated"))
        }
        None => {
            let created = master_binding_repo::create(
                db,
                master_binding::ActiveModel {
                    name: Set(normalized.name),
                    master_url: Set(normalized.master_url),
                    access_key: Set(normalized.access_key),
                    secret_key: Set(normalized.secret_key),
                    namespace: Set(normalized.namespace),
                    ingress_policy_id: Set(normalized.ingress_policy_id),
                    is_enabled: Set(normalized.is_enabled),
                    created_at: Set(now),
                    updated_at: Set(now),
                    ..Default::default()
                },
            )
            .await?;
            Ok((created, "created"))
        }
    }
}

pub async fn authorize_internal_request<S: FollowerRuntimeState>(
    state: &S,
    req: &actix_web::HttpRequest,
) -> Result<AuthorizedMasterBinding> {
    let binding = authorize_binding_request(state, req, false).await?;
    let ingress_policy = state
        .policy_snapshot()
        .get_policy_or_err(binding.ingress_policy_id)?;
    if ingress_policy.driver_type == crate::types::DriverType::Remote {
        return Err(precondition_failed_with_subcode(
            "master_binding.remote_ingress_unsupported",
            "master binding ingress policy cannot use remote driver",
        ));
    }

    Ok(AuthorizedMasterBinding {
        binding,
        ingress_policy,
    })
}

pub async fn authorize_binding_sync_request<S: FollowerRuntimeState>(
    state: &S,
    req: &actix_web::HttpRequest,
) -> Result<master_binding::Model> {
    authorize_binding_request(state, req, true).await
}

pub async fn authorize_presigned_put_request<S: FollowerRuntimeState>(
    state: &S,
    req: &actix_web::HttpRequest,
) -> Result<AuthorizedMasterBinding> {
    if req.method() != actix_web::http::Method::PUT {
        return Err(AsterError::auth_token_invalid(
            "remote presigned auth only supports PUT",
        ));
    }

    let binding = authorize_presigned_binding_request(state, req).await?;
    let ingress_policy = state
        .policy_snapshot()
        .get_policy_or_err(binding.ingress_policy_id)?;
    if ingress_policy.driver_type == crate::types::DriverType::Remote {
        return Err(precondition_failed_with_subcode(
            "master_binding.remote_ingress_unsupported",
            "master binding ingress policy cannot use remote driver",
        ));
    }

    Ok(AuthorizedMasterBinding {
        binding,
        ingress_policy,
    })
}

pub async fn authorize_presigned_get_request<S: FollowerRuntimeState>(
    state: &S,
    req: &actix_web::HttpRequest,
) -> Result<AuthorizedMasterBinding> {
    if req.method() != actix_web::http::Method::GET {
        return Err(AsterError::auth_token_invalid(
            "remote presigned auth only supports GET",
        ));
    }

    let binding = authorize_presigned_binding_request(state, req).await?;
    let ingress_policy = state
        .policy_snapshot()
        .get_policy_or_err(binding.ingress_policy_id)?;
    if ingress_policy.driver_type == crate::types::DriverType::Remote {
        return Err(precondition_failed_with_subcode(
            "master_binding.remote_ingress_unsupported",
            "master binding ingress policy cannot use remote driver",
        ));
    }

    Ok(AuthorizedMasterBinding {
        binding,
        ingress_policy,
    })
}

pub async fn sync_from_primary<S: FollowerRuntimeState>(
    state: &S,
    access_key: &str,
    input: SyncMasterBindingInput,
) -> Result<master_binding::Model> {
    let existing = master_binding_repo::find_by_access_key(state.db(), access_key)
        .await?
        .ok_or_else(|| AsterError::auth_invalid_credentials("unknown internal access_key"))?;
    let normalized = normalize_sync_input(input)?;

    let mut active: master_binding::ActiveModel = existing.into();
    active.name = Set(normalized.name);
    active.namespace = Set(normalized.namespace);
    active.is_enabled = Set(normalized.is_enabled);
    active.updated_at = Set(Utc::now());

    let updated = master_binding_repo::update(state.db(), active).await?;
    state
        .driver_registry()
        .reload_master_bindings(state.db())
        .await?;
    Ok(updated)
}

async fn authorize_binding_request<S: FollowerRuntimeState>(
    state: &S,
    req: &actix_web::HttpRequest,
    allow_disabled: bool,
) -> Result<master_binding::Model> {
    let access_key = header_value(req, INTERNAL_AUTH_ACCESS_KEY_HEADER)?;
    let timestamp = header_value(req, INTERNAL_AUTH_TIMESTAMP_HEADER)?
        .parse::<i64>()
        .map_err(|_| AsterError::auth_token_invalid("invalid internal auth timestamp"))?;
    let nonce = header_value(req, INTERNAL_AUTH_NONCE_HEADER)?;
    let signature = header_value(req, INTERNAL_AUTH_SIGNATURE_HEADER)?;

    let now = Utc::now().timestamp();
    if (now - timestamp).abs() > INTERNAL_AUTH_SKEW_SECS {
        return Err(AsterError::auth_token_invalid(
            "internal auth timestamp is outside allowed skew",
        ));
    }

    let nonce_cache_key = format!("internal_remote_nonce:{access_key}:{nonce}");
    if state.cache().get_bytes(&nonce_cache_key).await.is_some() {
        return Err(AsterError::auth_token_invalid(
            "internal auth nonce has already been used",
        ));
    }

    let binding = state
        .driver_registry()
        .find_master_binding_by_access_key(&access_key)
        .ok_or_else(|| AsterError::auth_invalid_credentials("unknown internal access_key"))?;
    if !allow_disabled && !binding.is_enabled {
        return Err(precondition_failed_with_subcode(
            "master_binding.disabled",
            "master binding is disabled",
        ));
    }

    let path_and_query = req
        .uri()
        .path_and_query()
        .map(|value| value.as_str())
        .unwrap_or_else(|| req.uri().path());
    let content_length = req
        .headers()
        .get(actix_web::http::header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok());

    if !verify_signature(
        &binding.secret_key,
        req.method().as_str(),
        path_and_query,
        timestamp,
        &nonce,
        content_length,
        &signature,
    ) {
        return Err(AsterError::auth_invalid_credentials(
            "internal auth signature mismatch",
        ));
    }

    state
        .cache()
        .set_bytes(
            &nonce_cache_key,
            Vec::new(),
            Some(INTERNAL_AUTH_NONCE_TTL_SECS),
        )
        .await;
    Ok(binding)
}

async fn authorize_presigned_binding_request<S: FollowerRuntimeState>(
    state: &S,
    req: &actix_web::HttpRequest,
) -> Result<master_binding::Model> {
    let access_key = query_value(req, PRESIGNED_AUTH_ACCESS_KEY_QUERY)?;
    let expires_at = query_value(req, PRESIGNED_AUTH_EXPIRES_QUERY)?
        .parse::<i64>()
        .map_err(|_| AsterError::auth_token_invalid("invalid remote presigned expiry"))?;
    let signature = query_value(req, PRESIGNED_AUTH_SIGNATURE_QUERY)?;

    if Utc::now().timestamp() > expires_at {
        return Err(AsterError::auth_token_invalid(
            "remote presigned URL has expired",
        ));
    }

    let binding = state
        .driver_registry()
        .find_master_binding_by_access_key(&access_key)
        .ok_or_else(|| AsterError::auth_invalid_credentials("unknown internal access_key"))?;
    if !binding.is_enabled {
        return Err(precondition_failed_with_subcode(
            "master_binding.disabled",
            "master binding is disabled",
        ));
    }

    if !verify_presigned_signature(
        &binding.secret_key,
        req.method().as_str(),
        &presigned_request_target(req),
        &access_key,
        expires_at,
        &signature,
    ) {
        return Err(AsterError::auth_invalid_credentials(
            "remote presigned signature mismatch",
        ));
    }

    Ok(binding)
}

pub fn provider_storage_path(binding: &master_binding::Model, object_key: &str) -> String {
    let object_key = object_key.trim_start_matches('/');
    if object_key.is_empty() {
        binding.namespace.clone()
    } else {
        format!("{}/{}", binding.namespace.trim_matches('/'), object_key)
    }
}

pub async fn assert_follower_ready<S: FollowerRuntimeState>(state: &S) -> Result<()> {
    let bindings = master_binding_repo::find_all(state.db()).await?;
    let enabled_bindings: Vec<_> = bindings
        .into_iter()
        .filter(|binding| binding.is_enabled)
        .collect();
    if enabled_bindings.is_empty() {
        return Err(AsterError::storage_driver_error(
            "no active master bindings configured",
        ));
    }

    for binding in enabled_bindings {
        let policy = state
            .policy_snapshot()
            .get_policy_or_err(binding.ingress_policy_id)?;
        if policy.driver_type == crate::types::DriverType::Remote {
            return Err(AsterError::storage_driver_error(format!(
                "master binding #{} ingress policy cannot use remote driver",
                binding.id
            )));
        }
        let _ = state.driver_registry().get_driver(&policy)?;
    }

    Ok(())
}

async fn normalize_upsert_input<C: ConnectionTrait>(
    db: &C,
    input: UpsertMasterBindingInput,
) -> Result<UpsertMasterBindingInput> {
    validate_ingress_policy(db, input.ingress_policy_id).await?;
    Ok(UpsertMasterBindingInput {
        name: normalize_non_blank("name", &input.name)?,
        master_url: normalize_remote_base_url(&input.master_url)?,
        access_key: normalize_non_blank("access_key", &input.access_key)?,
        secret_key: normalize_non_blank("secret_key", &input.secret_key)?,
        namespace: normalize_namespace(&input.namespace)?,
        ingress_policy_id: input.ingress_policy_id,
        is_enabled: input.is_enabled,
    })
}

fn normalize_sync_input(input: SyncMasterBindingInput) -> Result<SyncMasterBindingInput> {
    Ok(SyncMasterBindingInput {
        name: normalize_non_blank("name", &input.name)?,
        namespace: normalize_namespace(&input.namespace)?,
        is_enabled: input.is_enabled,
    })
}

async fn validate_ingress_policy<C: ConnectionTrait>(db: &C, ingress_policy_id: i64) -> Result<()> {
    let policy = policy_repo::find_by_id(db, ingress_policy_id).await?;
    if policy.driver_type == crate::types::DriverType::Remote {
        return Err(AsterError::validation_error(
            "master binding ingress policy cannot use remote driver",
        ));
    }
    Ok(())
}

fn verify_signature(
    secret_key: &str,
    method: &str,
    path_and_query: &str,
    timestamp: i64,
    nonce: &str,
    content_length: Option<u64>,
    provided_signature: &str,
) -> bool {
    let mut decoded = [0u8; 32];
    if hex::decode_to_slice(provided_signature, &mut decoded).is_err() {
        return false;
    }

    let canonical = format!(
        "{}\n{}\n{}\n{}\n{}",
        method,
        path_and_query,
        timestamp,
        nonce,
        content_length
            .map(|value| value.to_string())
            .unwrap_or_default()
    );
    let Ok(mut mac) = <Hmac<Sha256> as KeyInit>::new_from_slice(secret_key.as_bytes()) else {
        return false;
    };
    mac.update(canonical.as_bytes());
    mac.verify_slice(&decoded).is_ok()
}

fn verify_presigned_signature(
    secret_key: &str,
    method: &str,
    request_target: &str,
    access_key: &str,
    expires_at: i64,
    provided_signature: &str,
) -> bool {
    let mut decoded = [0u8; 32];
    if hex::decode_to_slice(provided_signature, &mut decoded).is_err() {
        return false;
    }

    let expected =
        sign_presigned_request(secret_key, method, request_target, access_key, expires_at);
    let Ok(expected) = hex::decode(expected) else {
        return false;
    };
    expected == decoded
}

fn presigned_request_target(req: &actix_web::HttpRequest) -> String {
    let path = req.uri().path();
    let Some(query) = req.uri().query() else {
        return path.to_string();
    };

    let filtered: Vec<&str> = query
        .split('&')
        .filter(|segment| !segment.is_empty())
        .filter(|segment| {
            let key = segment.split('=').next().unwrap_or_default();
            key != PRESIGNED_AUTH_ACCESS_KEY_QUERY
                && key != PRESIGNED_AUTH_EXPIRES_QUERY
                && key != PRESIGNED_AUTH_SIGNATURE_QUERY
        })
        .collect();

    if filtered.is_empty() {
        path.to_string()
    } else {
        format!("{path}?{}", filtered.join("&"))
    }
}

fn header_value(req: &actix_web::HttpRequest, name: &str) -> Result<String> {
    req.headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| AsterError::auth_token_invalid(format!("missing header {name}")))
}

fn query_value(req: &actix_web::HttpRequest, name: &str) -> Result<String> {
    actix_web::web::Query::<std::collections::HashMap<String, String>>::from_query(
        req.query_string(),
    )
    .map_err(|_| AsterError::auth_token_invalid("invalid query string"))?
    .get(name)
    .cloned()
    .filter(|value| !value.is_empty())
    .ok_or_else(|| AsterError::auth_token_invalid(format!("missing query parameter '{name}'")))
}

fn normalize_non_blank(field: &str, value: &str) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AsterError::validation_error(format!(
            "{field} cannot be blank"
        )));
    }
    Ok(trimmed.to_string())
}

fn normalize_namespace(value: &str) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AsterError::validation_error("namespace cannot be blank"));
    }
    if !trimmed
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(AsterError::validation_error(
            "namespace only allows ASCII letters, digits, '.', '_' and '-'",
        ));
    }
    Ok(trimmed.to_string())
}
