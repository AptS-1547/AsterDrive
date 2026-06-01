# External Authentication Module

This document explains the current external-authentication implementation in the repository, not a future plan.

## What this module does

External authentication lets users sign in through external identity providers such as OpenID Connect or Generic OAuth2. The module also covers account binding, email verification fallback, auto-provisioning, and admin-side provider management.

## Code locations

| Area | Path | Notes |
| --- | --- | --- |
| Route | `src/api/routes/auth/external_auth.rs` | Anonymous provider list, login start, callback, email verification fallback, password linking, user unbinding |
| Admin route | `src/api/routes/admin/external_auth.rs` | Provider kind list, provider CRUD, draft testing, saved provider testing |
| Service | `src/services/external_auth_service/` | Provider config, login flow, identity binding, and account provisioning |
| Entity / repo | `src/entities/external_auth_*`, `src/db/repository/external_auth_*` | Persistent provider and identity storage |

## Supported provider kinds

Current supported provider kinds are:

- `oidc`
- `generic_oauth2`

Both are configured by admins and shown on the login page only after being enabled.

## High-level flow

1. An admin creates a provider in `Admin -> External Auth`.
2. The login page reads the enabled public summary and shows the corresponding entry.
3. The user starts the external login flow.
4. The provider redirects back to the callback endpoint.
5. The service resolves the returned identity.
6. Depending on provider settings and account state, the user is either:
   - signed in directly
   - linked to an existing local account
   - asked to complete email verification
   - asked to bind the external identity with a local password

## Important provider behaviors

### OIDC

- Uses discovery
- Uses PKCE and nonce validation
- Verifies the ID token

### Generic OAuth2

- Uses manually configured authorization, token, and userinfo endpoints
- Uses PKCE and token exchange
- Maps claims from the UserInfo response

## Account provisioning and binding

The service supports several account-resolution paths:

- If the external identity already has a local binding, sign in directly
- If verified email auto-linking is enabled and the provider returns a verified email, find the local user with the same email and create a binding
- If auto-provisioning is enabled, check the public registration switch, email, email domain, and email verification policy, then create a normal user and bind the identity
- If the identity cannot be resolved directly, create an email verification flow or ask the user to bind through their local password

When auto-provisioning a user, the system creates a random internal password. The user can still later manage the account through normal local password reset / change flows.

## API entry points

- Admin provider API: [`./api/admin.md#external-authentication-providers`](./api/admin.md)
- Login-side external-auth API: [`./api/auth.md#external-authentication`](./api/auth.md)
- Deployment-facing configuration guide: [`../../docs/config/external-auth.md`](../../docs/config/external-auth.md)

## Testing

Key tests cover:

- provider CRUD
- callback and identity resolution
- verified-email linking
- auto-provisioning policy checks
- email verification fallback
- password binding
- unlinking external identities
