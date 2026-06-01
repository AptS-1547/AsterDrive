# API Overview

This page groups the HTTP surface by feature instead of trying to duplicate the OpenAPI output.

Most user-facing JSON / REST endpoints live under:

```text
/api/v1
```

## Primary vs. Follower

The repository exposes two HTTP node modes:

- `primary`
  - ordinary user REST API
  - public sharing API
  - remote-node reverse-tunnel internal entry `/api/v1/internal/remote-tunnel/*`
  - WebDAV
  - frontend pages
  - health checks
- `follower`
  - health checks
  - internal object storage protocol `/api/v1/internal/storage/*`

So anything used by a browser, a frontend SDK, or a public share page is usually in this directory. The two internal paths above are between primary and follower nodes only.

## Endpoints outside `/api/v1`

The following capabilities are intentionally mounted elsewhere:

- health checks: `/health*`
- direct download links: `/d/{token}/{filename}`
- preview links: `/pv/{token}/{filename}`
- WebDAV: default `/webdav`
- frontend pages, public share pages, and static fallback routes: registered last on the primary node

## Unified response shape

Most JSON APIs use the same wrapper:

```json
{
  "code": 0,
  "msg": "",
  "data": {}
}
```

Field meaning:

- `code`: numeric error code, `0` means success
- `msg`: human-readable fallback message
- `data`: payload, omitted by some successful endpoints

Error responses also include an `error` object:

```json
{
  "code": 2003,
  "msg": "untrusted request origin for cookie-authenticated action",
  "error": {
    "code": "auth.request_origin_untrusted",
    "internal_code": "E013",
    "subcode": "auth.request_origin_untrusted",
    "retryable": false
  }
}
```

The important part is that the new stable string error code lives in `error.code`. The old numeric top-level `code` remains for compatibility during the transition.

## Error-code migration

The repository is currently moving from `ApiSubcode` to `ApiErrorCode`.

- The backend response must write `error.code: ApiErrorCode`
- `ApiErrorInfo.subcode` is deprecated and only kept for older clients
- Compatibility shims that still mention subcodes must carry `TODO(0.3.0)`
- OpenAPI should expose `ApiErrorCode` and `ApiErrorInfo.code`
- New user-visible errors should prefer a stable `ApiErrorCode` instead of inventing string matches in the client

## Non-JSON endpoints

The following responses are raw content instead of `ApiResponse`:

- file downloads
- direct download links
- preview links
- share stream sessions
- thumbnails
- image previews
- archive-download ZIP streams
- share file downloads
- share thumbnails
- share image previews
- uploaded avatars
- storage event streams
- WOPI `CheckFileInfo` and content callbacks
- WebDAV responses
- Prometheus metrics
- follower object streams `/api/v1/internal/storage/objects/{tail:.*}`
- primary reverse-tunnel WebSocket `/api/v1/internal/remote-tunnel/connect`

Public branding, preview-app configuration, thumbnail support, media-data support, and remote enrollment are unauthenticated, but they are still ordinary `/api/v1/public/*` JSON endpoints.

## Error-code ranges

| Range | Meaning |
| --- | --- |
| `0` | success |
| `1000-1099` | general, database, config, rate-limit, mail, conflict |
| `2000-2099` | authentication, authorization, activation, contact verification |
| `3000-3099` | file, upload session, chunk, lock, thumbnail, conditional request |
| `4000-4099` | storage policy, quota, driver, object storage, backend-specific errors |
| `5000-5099` | folder errors |
| `6000-6099` | sharing errors |

## Supported authentication modes

### REST / frontend

- HttpOnly cookie
- `Authorization: Bearer <jwt>`

### WebDAV

- `Authorization: Basic ...`
- `Authorization: Bearer <jwt>`

### Follower internal storage protocol

- primary-signed headers:
  - `x-aster-access-key`
  - `x-aster-timestamp`
  - `x-aster-nonce`
  - `x-aster-signature`
- some object GET / PUT operations also accept presigned query parameters

### Remote-tunnel internal entry

- the follower connects to the primary's `/api/v1/internal/remote-tunnel/*` with remote-node signatures
- this entry is for reverse-tunnel polling, completion callbacks, and WebSocket streaming only

## Workspace scopes

There are two protected workspace types:

- personal space: `/files`, `/folders`, `/batch`, `/search`, `/shares`, `/trash`
- team space: the same semantics, but prefixed with `/teams/{team_id}`

Typical team paths:

```text
/api/v1/teams/{team_id}/folders
/api/v1/teams/{team_id}/files/{id}
/api/v1/teams/{team_id}/batch/move
/api/v1/teams/{team_id}/search
/api/v1/teams/{team_id}/shares
/api/v1/teams/{team_id}/trash
/api/v1/teams/{team_id}/tasks
```

## Module index

- [Authentication](./auth.md)
- [Files](./files.md)
- [Folders](./folders.md)
- [Teams and team spaces](./teams.md)
- [Batch operations](./batch.md)
- [Sharing](./shares.md)
- [Trash](./trash.md)
- [Search](./search.md)
- [Background tasks](./tasks.md)
- [WOPI](./wopi.md)
- [WebDAV](./webdav.md)
- [Properties](./properties.md)
- [Public API](./public.md)
- [Admin API](./admin.md)
- [Health checks](./health.md)
- [Internal storage protocol (follower)](./internal-storage.md)

Useful clusters to read first:

- upload and versioning: [Files](./files.md)
- MFA, passkeys, external auth, and login sessions: [Authentication](./auth.md)
- archive-only preview: [Files](./files.md), [Sharing](./shares.md), and [Background tasks](./tasks.md)
- batch delete / move / copy / package download: [Batch operations](./batch.md)
- trash restore and purge: [Trash](./trash.md)
- search, file categories, and extension filters: [Search](./search.md)
- task retry and storage migration tasks: [Background tasks](./tasks.md)
- team management and team workspaces: [Teams](./teams.md)
- public shares, preview links, and stream sessions: [Sharing](./shares.md)
- Office / WOPI preview and callbacks: [WOPI](./wopi.md)
- WebDAV, accounts, and DeltaV: [WebDAV](./webdav.md)
- login page, anonymous page, thumbnail / media-data support, and remote enrollment: [Public API](./public.md)
- internal object protocol and reverse-tunnel control plane: [Internal storage protocol](./internal-storage.md)
- admin policies, remote nodes, storage migration, file / blob observability, external auth providers, locks, runtime config, and audit: [Admin API](./admin.md)

OpenAPI registration lives in `src/api/openapi.rs`, but the actual runtime route registration still comes from `src/api/primary.rs`, `src/api/follower.rs`, and `src/api/routes/**`. If the OpenAPI spec and the route code disagree, trust the route code first and then repair the spec.
