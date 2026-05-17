use base64::Engine as _;
use chrono::Utc;

use crate::config::auth_runtime::RuntimeAuthPolicy;
use crate::db::repository::{external_auth_identity_repo, user_repo};
use crate::entities::{
    external_auth_email_verification_flow, external_auth_identity, external_auth_provider, user,
};
use crate::errors::{AsterError, Result};
use crate::external_auth::ExternalAuthProfile;
use crate::runtime::PrimaryAppState;
use crate::services::auth_service;
use crate::types::{UserRole, UserStatus};
use crate::utils::hash;

use super::normalize::email_domain_allowed;
use super::{EXTERNAL_AUTH_USER_PASSWORD_BYTES, USERNAME_MAX_LEN, USERNAME_MIN_LEN};

pub(super) type ExternalAuthUserClaims = ExternalAuthProfile;

#[derive(Debug)]
pub(super) struct ResolvedExternalAuthUser {
    pub(super) user: user::Model,
    pub(super) linked: bool,
    pub(super) auto_provisioned: bool,
}

fn require_email_if_configured(
    provider: &external_auth_provider::Model,
    claims: &ExternalAuthUserClaims,
) -> Result<()> {
    if !provider.require_email_verified {
        return Ok(());
    }
    if claims.email.as_deref().is_none_or(str::is_empty) {
        return Err(AsterError::auth_forbidden(
            "external auth provider requires a verified email but no email claim was returned",
        ));
    }
    if !claims.email_verified {
        return Err(AsterError::auth_forbidden(
            "external auth provider requires verified email",
        ));
    }
    Ok(())
}

fn random_internal_password() -> String {
    let mut bytes = [0_u8; EXTERNAL_AUTH_USER_PASSWORD_BYTES];
    let mut rng = rand::rng();
    rand::RngExt::fill(&mut rng, &mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn sanitize_username_piece(value: &str) -> String {
    value
        .chars()
        .filter_map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                Some(c.to_ascii_lowercase())
            } else if c == '.' || c == ' ' {
                Some('-')
            } else {
                None
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

async fn unique_username<C: sea_orm::ConnectionTrait>(
    db: &C,
    claims: &ExternalAuthUserClaims,
) -> Result<String> {
    let mut base = claims
        .preferred_username
        .as_deref()
        .map(sanitize_username_piece)
        .filter(|value| value.len() >= USERNAME_MIN_LEN)
        .or_else(|| {
            claims
                .email
                .as_deref()
                .and_then(|email| email.split('@').next())
                .map(sanitize_username_piece)
                .filter(|value| value.len() >= USERNAME_MIN_LEN)
        })
        .unwrap_or_else(|| format!("oidc{}", &hash::sha256_hex(claims.subject.as_bytes())[..8]));

    if base.len() > USERNAME_MAX_LEN {
        base.truncate(USERNAME_MAX_LEN);
        base = base.trim_matches('-').to_string();
    }
    while base.len() < USERNAME_MIN_LEN {
        base.push('0');
    }

    if user_repo::find_by_username(db, &base).await?.is_none() {
        return Ok(base);
    }

    let stem_max = USERNAME_MAX_LEN.saturating_sub(5);
    let mut stem = base;
    if stem.len() > stem_max {
        stem.truncate(stem_max);
        stem = stem.trim_matches('-').to_string();
    }
    if stem.len() < USERNAME_MIN_LEN {
        stem = "oidc".to_string();
    }
    for index in 1..10_000 {
        let candidate = format!("{stem}-{index}");
        if candidate.len() > USERNAME_MAX_LEN {
            continue;
        }
        if user_repo::find_by_username(db, &candidate).await?.is_none() {
            return Ok(candidate);
        }
    }
    Err(AsterError::validation_error(
        "failed to allocate unique username for external auth user",
    ))
}

pub(super) fn claims_with_verified_local_email(
    flow: &external_auth_email_verification_flow::Model,
    email: &str,
) -> ExternalAuthUserClaims {
    ExternalAuthUserClaims {
        identity_namespace: flow.identity_namespace.clone(),
        subject: flow.subject.clone(),
        email: Some(email.to_string()),
        email_verified: true,
        display_name: flow.display_name_snapshot.clone(),
        preferred_username: flow.preferred_username_snapshot.clone(),
    }
}

pub(super) fn claims_without_provider_email(
    flow: &external_auth_email_verification_flow::Model,
) -> ExternalAuthUserClaims {
    ExternalAuthUserClaims {
        identity_namespace: flow.identity_namespace.clone(),
        subject: flow.subject.clone(),
        email: None,
        email_verified: false,
        display_name: flow.display_name_snapshot.clone(),
        preferred_username: flow.preferred_username_snapshot.clone(),
    }
}

async fn create_identity_for_claims<C: sea_orm::ConnectionTrait>(
    db: &C,
    user_id: i64,
    provider: &external_auth_provider::Model,
    claims: &ExternalAuthUserClaims,
    now: chrono::DateTime<Utc>,
) -> Result<external_auth_identity::Model> {
    external_auth_identity_repo::create_identity(
        db,
        external_auth_identity_repo::CreateExternalAuthIdentityInput {
            user_id,
            provider_id: provider.id,
            identity_namespace: claims.identity_namespace.clone(),
            subject: claims.subject.clone(),
            email_snapshot: claims.email.clone(),
            display_name_snapshot: claims.display_name.clone(),
            now,
        },
    )
    .await
}

pub(super) async fn link_external_auth_identity_to_authenticated_user<
    C: sea_orm::ConnectionTrait,
>(
    db: &C,
    provider: &external_auth_provider::Model,
    claims: &ExternalAuthUserClaims,
    user: user::Model,
    now: chrono::DateTime<Utc>,
) -> Result<ResolvedExternalAuthUser> {
    if let Some(identity) = external_auth_identity_repo::find_by_identity_namespace_subject(
        db,
        &claims.identity_namespace,
        &claims.subject,
    )
    .await?
    {
        if identity.user_id != user.id {
            return Err(AsterError::auth_forbidden(
                "external auth identity is already linked to another user",
            ));
        }
        external_auth_identity_repo::touch_login(
            db,
            identity.id,
            claims.email.as_deref(),
            claims.display_name.as_deref(),
            now,
        )
        .await?;
        return Ok(ResolvedExternalAuthUser {
            user,
            linked: false,
            auto_provisioned: false,
        });
    }

    create_identity_for_claims(db, user.id, provider, claims, now).await?;
    Ok(ResolvedExternalAuthUser {
        user,
        linked: true,
        auto_provisioned: false,
    })
}

async fn create_external_auth_user_and_identity(
    state: &PrimaryAppState,
    provider: &external_auth_provider::Model,
    claims: &ExternalAuthUserClaims,
    now: chrono::DateTime<Utc>,
) -> Result<ResolvedExternalAuthUser> {
    let auth_policy = RuntimeAuthPolicy::from_runtime_config(&state.runtime_config);
    if !auth_policy.allow_user_registration {
        return Err(AsterError::auth_forbidden(
            "new user registration is disabled",
        ));
    }

    let email = claims.email.as_deref().ok_or_else(|| {
        AsterError::auth_forbidden("external auth auto provisioning requires an email claim")
    })?;
    if (provider.require_email_verified || provider.auto_link_verified_email_enabled)
        && !claims.email_verified
    {
        return Err(AsterError::auth_forbidden(
            "external auth auto provisioning requires verified email",
        ));
    }
    if !email_domain_allowed(provider, email)? {
        return Err(AsterError::auth_forbidden(
            "external auth email domain is not allowed for this provider",
        ));
    }

    let txn = crate::db::transaction::begin(&state.db).await?;
    let result = async {
        if let Some(existing) = user_repo::find_by_email(&txn, email).await? {
            return Err(AsterError::validation_error(format!(
                "user email '{}' already exists but automatic email linking is disabled",
                existing.email
            )));
        }
        let username = unique_username(&txn, claims).await?;
        let password = random_internal_password();
        let user = auth_service::shared::create_user_with_role(
            &txn,
            state,
            auth_service::shared::CreateUserWithRoleInput {
                username: &username,
                email,
                password: &password,
                role: UserRole::User,
                status: UserStatus::Active,
                email_verified_at: claims.email_verified.then_some(now),
            },
        )
        .await?;
        create_identity_for_claims(&txn, user.id, provider, claims, now).await?;
        Ok(user)
    }
    .await;

    match result {
        Ok(user) => {
            crate::db::transaction::commit(txn).await?;
            Ok(ResolvedExternalAuthUser {
                user,
                linked: true,
                auto_provisioned: true,
            })
        }
        Err(err) => Err(err),
    }
}

async fn create_external_auth_user_and_identity_in_connection<C: sea_orm::ConnectionTrait>(
    db: &C,
    state: &PrimaryAppState,
    provider: &external_auth_provider::Model,
    claims: &ExternalAuthUserClaims,
    now: chrono::DateTime<Utc>,
) -> Result<user::Model> {
    let auth_policy = RuntimeAuthPolicy::from_runtime_config(&state.runtime_config);
    if !auth_policy.allow_user_registration {
        return Err(AsterError::auth_forbidden(
            "new user registration is disabled",
        ));
    }

    let email = claims.email.as_deref().ok_or_else(|| {
        AsterError::auth_forbidden("external auth auto provisioning requires an email claim")
    })?;
    if !email_domain_allowed(provider, email)? {
        return Err(AsterError::auth_forbidden(
            "external auth email domain is not allowed for this provider",
        ));
    }
    if user_repo::find_by_email(db, email).await?.is_some() {
        return Err(AsterError::validation_error(
            "user email already exists but automatic email linking is disabled",
        ));
    }

    let username = unique_username(db, claims).await?;
    let password = random_internal_password();
    let user = auth_service::shared::create_user_with_role(
        db,
        state,
        auth_service::shared::CreateUserWithRoleInput {
            username: &username,
            email,
            password: &password,
            role: UserRole::User,
            status: UserStatus::Active,
            email_verified_at: Some(now),
        },
    )
    .await?;
    create_identity_for_claims(db, user.id, provider, claims, now).await?;
    Ok(user)
}

pub(super) async fn resolve_external_auth_user_with_verified_email<C: sea_orm::ConnectionTrait>(
    db: &C,
    state: &PrimaryAppState,
    provider: &external_auth_provider::Model,
    claims: &ExternalAuthUserClaims,
    now: chrono::DateTime<Utc>,
) -> Result<ResolvedExternalAuthUser> {
    let email = claims.email.as_deref().ok_or_else(|| {
        AsterError::auth_forbidden("external auth email verification requires an email")
    })?;
    if !email_domain_allowed(provider, email)? {
        return Err(AsterError::auth_forbidden(
            "external auth email domain is not allowed for this provider",
        ));
    }

    if let Some(identity) = external_auth_identity_repo::find_by_identity_namespace_subject(
        db,
        &claims.identity_namespace,
        &claims.subject,
    )
    .await?
    {
        external_auth_identity_repo::touch_login(
            db,
            identity.id,
            claims.email.as_deref(),
            claims.display_name.as_deref(),
            now,
        )
        .await?;
        let user = user_repo::find_by_id(db, identity.user_id).await?;
        if !user.status.is_active() {
            return Err(AsterError::auth_forbidden("account is disabled"));
        }
        return Ok(ResolvedExternalAuthUser {
            user,
            linked: false,
            auto_provisioned: false,
        });
    }

    if let Some(user) = user_repo::find_by_email(db, email).await? {
        if !user.status.is_active() {
            return Err(AsterError::auth_forbidden("account is disabled"));
        }
        if user.email_verified_at.is_none() {
            return Err(AsterError::auth_forbidden(
                "local account email is not verified",
            ));
        }
        create_identity_for_claims(db, user.id, provider, claims, now).await?;
        return Ok(ResolvedExternalAuthUser {
            user,
            linked: true,
            auto_provisioned: false,
        });
    }

    let user =
        create_external_auth_user_and_identity_in_connection(db, state, provider, claims, now)
            .await?;
    Ok(ResolvedExternalAuthUser {
        user,
        linked: true,
        auto_provisioned: true,
    })
}

pub(super) async fn resolve_external_auth_user(
    state: &PrimaryAppState,
    provider: &external_auth_provider::Model,
    claims: &ExternalAuthUserClaims,
) -> Result<Option<ResolvedExternalAuthUser>> {
    let now = Utc::now();
    if let Some(identity) = external_auth_identity_repo::find_by_identity_namespace_subject(
        &state.db,
        &claims.identity_namespace,
        &claims.subject,
    )
    .await?
    {
        external_auth_identity_repo::touch_login(
            &state.db,
            identity.id,
            claims.email.as_deref(),
            claims.display_name.as_deref(),
            now,
        )
        .await?;
        let user = user_repo::find_by_id(&state.db, identity.user_id).await?;
        if !user.status.is_active() {
            return Err(AsterError::auth_forbidden("account is disabled"));
        }
        return Ok(Some(ResolvedExternalAuthUser {
            user,
            linked: false,
            auto_provisioned: false,
        }));
    }

    require_email_if_configured(provider, claims)?;
    if let Some(email) = claims.email.as_deref()
        && !email_domain_allowed(provider, email)?
    {
        return Err(AsterError::auth_forbidden(
            "external auth email domain is not allowed for this provider",
        ));
    }

    if provider.auto_link_verified_email_enabled
        && claims.email_verified
        && let Some(email) = claims.email.as_deref()
        && let Some(user) = user_repo::find_by_email(&state.db, email).await?
    {
        if !user.status.is_active() {
            return Err(AsterError::auth_forbidden("account is disabled"));
        }
        create_identity_for_claims(&state.db, user.id, provider, claims, now).await?;
        return Ok(Some(ResolvedExternalAuthUser {
            user,
            linked: true,
            auto_provisioned: false,
        }));
    }

    if provider.auto_provision_enabled {
        let auth_policy = RuntimeAuthPolicy::from_runtime_config(&state.runtime_config);
        let Some(email) = claims.email.as_deref().filter(|email| !email.is_empty()) else {
            return Ok(None);
        };
        if (provider.require_email_verified || provider.auto_link_verified_email_enabled)
            && !claims.email_verified
        {
            return Ok(None);
        }
        if !auth_policy.allow_user_registration {
            return Ok(None);
        }
        if user_repo::find_by_email(&state.db, email).await?.is_some() {
            return Ok(None);
        }
        return create_external_auth_user_and_identity(state, provider, claims, now)
            .await
            .map(Some);
    }

    Ok(None)
}

pub(super) fn external_auth_claims_missing_email(claims: &ExternalAuthUserClaims) -> bool {
    claims.email.as_deref().is_none_or(str::is_empty)
}

pub(super) async fn resolve_existing_external_auth_identity<C: sea_orm::ConnectionTrait>(
    db: &C,
    claims: &ExternalAuthUserClaims,
    now: chrono::DateTime<Utc>,
) -> Result<Option<ResolvedExternalAuthUser>> {
    let Some(identity) = external_auth_identity_repo::find_by_identity_namespace_subject(
        db,
        &claims.identity_namespace,
        &claims.subject,
    )
    .await?
    else {
        return Ok(None);
    };

    external_auth_identity_repo::touch_login(
        db,
        identity.id,
        claims.email.as_deref(),
        claims.display_name.as_deref(),
        now,
    )
    .await?;
    let user = user_repo::find_by_id(db, identity.user_id).await?;
    if !user.status.is_active() {
        return Err(AsterError::auth_forbidden("account is disabled"));
    }
    Ok(Some(ResolvedExternalAuthUser {
        user,
        linked: false,
        auto_provisioned: false,
    }))
}
