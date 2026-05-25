# Storage Policies

::: tip This page covers two layers of "where files are stored"
- **`Admin -> Storage Policies`** - Where files are actually stored: local / S3 / follower node
- **`Admin -> Policy Groups`** - Which storage policy a user or team hits when uploading

Users and teams are not bound to storage policies directly. They are bound to **policy groups**; policy groups then route uploads to specific policies by rules.
:::

## What Exists After the First Startup?

After a newly deployed instance starts for the first time, the system automatically prepares:

- Default local storage policy `Local Default`
- Default policy group `Default Policy Group`

If you change nothing, new users are automatically bound to the default policy group, and that policy group routes uploads to the default local storage policy. When a system administrator creates a new team without manually choosing a policy group, the default policy group is used.

## Currently Supported Storage Types

| Type | Description |
| --- | --- |
| `local` | Files are stored in a local directory |
| `s3` | Files are stored in S3 or S3-compatible object storage, such as MinIO / R2 / B2 / OSS / COS |
| `remote` | Files are written to another AsterDrive follower node through the internal remote storage protocol |

## Only Change Storage Policies, or Also Change Policy Groups?

- If you only want to change "which storage backend files ultimately land in", look at storage policies
- If you want different users, teams, or file sizes to use different routes, configure policy groups too

Typical admin-console workflow:

1. Create or test the storage policy
2. Create policy group rules
3. Bind users or teams to the target policy group

## Common Storage Policy Options

| Item | Purpose |
| --- | --- |
| Name | Display name in the admin console |
| Driver type | `local`, `s3`, or `remote` |
| Connection information | Local directory / S3 endpoint, bucket, secrets / bound follower node |
| Single-file size limit | Maximum upload size. `0` = unlimited. |
| Chunk size | Size of each chunk for large-file uploads |
| Default policy | Preferred by newly created default groups or default routing rules |
| Extra options | Local content deduplication, S3 upload/download methods, remote upload/download methods, and so on |

## How to Choose Between the Three Storage Types

### `local`

Suitable for single-node deployments, NAS, and files that should land directly on local disk.

::: tip Content deduplication is disabled by default
After enabling it: when an upload completes, AsterDrive reads the temporary file again, calculates its content fingerprint, and reuses the same underlying file for identical content to save disk space.

Keep it disabled: the upload path is more direct, without an extra full-file read, and identical content is stored separately.

Home and single-node deployments usually do not need deduplication. Small teams that repeatedly upload the same assets can enable it.
:::

### `s3`

Suitable when files are stored in MinIO, AWS S3, or other compatible object storage.

If you are ready to configure buckets, credentials, policy group rules, and user/team bindings, go directly to the [S3 / MinIO / R2 storage policy tutorial](/en/storage/s3-minio-r2).

### `remote`

Suitable when the control plane should stay on the primary node while real object placement is split to another AsterDrive follower node.

If you have already enrolled the follower node and are ready to use it in policy groups and real upload routes, go directly to the [follower node storage policy tutorial](/en/storage/remote-follower).

Remember three points first:

- The policy itself only binds a follower node; it no longer has a separate endpoint or access key
- Where the follower actually writes objects is decided by the **default ingress target** in the follower node details
- Remote downloads support `relay_stream` and `presigned`
- Remote uploads support `relay_stream` and `presigned`; `presigned` requires the browser to reach the follower node directly

To actually use it, first register the node under `Admin -> Follower Nodes`, and make sure it is enrolled, enabled, has a reachable `base_url`, and has at least one applied default ingress target. See [follower nodes](/en/guide/remote-nodes) for the complete flow.

## S3 Upload Methods

### Server-Side Streaming Relay `relay_stream`

The browser uploads the file to AsterDrive first, and the server relays it to S3. The **normal path does not land in the local temporary directory** and does not perform content deduplication.

Use this when the network between browsers and S3 is unstable, or when you want all ingress and egress to pass through the application node.

### Presigned Direct Upload `presigned`

The browser uploads **directly to S3 / MinIO**. Files no larger than the chunk size use a single upload; larger files automatically use S3 multipart upload.

::: warning Configure CORS before using presigned uploads
The object storage side must configure browser-upload CORS:

- Allow the upload origin
- Allow `PUT`
- Include `ETag` in `ExposeHeaders`

Without CORS, the browser reports a cross-origin error directly.
:::

See the [S3 / MinIO / R2 storage policy tutorial](/en/storage/s3-minio-r2) for detailed CORS configuration, MinIO / R2 / AWS S3 field examples, and policy group routing steps.

## S3 Download Methods

### Server-Side Relay Download `relay_stream`

AsterDrive reads from object storage first, then streams bytes back to the browser. This is suitable when the application node still needs to control response headers, same-origin download behavior, or downstream network policy.

### Presigned Redirect `presigned`

AsterDrive performs permission checks first, then **redirects** the browser to a short-lived S3 `GET` URL. Download bandwidth and long-connection pressure move to object storage, but response headers and cache behavior also depend more on the object storage side.

::: tip Know the current boundaries

- Logged-in file downloads, team downloads, and share downloads - routed by storage policy
- Public direct links `/d/...` - default inline responses still use the server response; after adding `?download=1`, attachment download routing is reused, and `presigned` returns a redirect when matched
- Preview path `/pv/...` - still uses the server response and does not redirect
- Share download counts - increment after a usable download response is generated; `304` does not increment

:::

Before using `presigned` downloads, confirm:

- Client networks can connect directly to object storage
- Object storage can return the expected `Content-Disposition` / `Content-Type`
- You accept that download cache behavior is determined more by object storage response headers

## Follower Node Upload Methods

### Pure Streaming Forwarding `relay_stream`

The browser uploads the file to the primary node first, and the primary streams it directly to the follower node.  
No full-file relay copy is generated in between, but the path strongly depends on both browser-to-primary and primary-to-follower network stability.

### Presigned Direct Upload `presigned`

The browser uploads the file directly to the follower node.  
This reduces upload pressure on the primary node, but the browser must be able to access the follower node, and the follower node must expose the response headers required by the upload.

## Follower Node Ingress Targets

Remote storage policies answer "which follower node the primary sends traffic to".  
Ingress targets answer "where this follower writes objects after receiving them".

Entry point:

```text
Admin -> Follower Nodes -> Open a node -> Primary-Managed Ingress Targets
```

Current ingress target types:

- `local`: write to a local directory on the follower
- `s3`: write to object storage that the follower can access

The base path of a `local` ingress target can only be a relative path. The final location is under the follower's `server.follower.managed_ingress_local_root`.  
If there is no default ingress target, remote writes are rejected. If the default ingress target is still "pending apply" or "apply failed", do not rush production traffic to it.

## Follower Node Download Methods

### Server-Side Relay Download `relay_stream`

The primary node pulls the object from the follower node first, then streams bytes back to the browser.  
This is suitable when the primary node still needs to control response headers, same-origin download behavior, or downstream network policy.

### Presigned Redirect `presigned`

AsterDrive performs permission checks first, then redirects the browser to a short-lived follower-node download address.  
This reduces download bandwidth pressure on the primary node, but final response headers and cache behavior depend more on the follower node side.

## How to Understand Policy Groups

A policy group is a set of **ordered rules**. Each rule specifies:

- Which storage policy to hit
- Priority
- Applicable file size range

The simplest policy group has only one rule: files of any size go to `Local Default`.

More common advanced combinations:

- Small files use local storage for fast response and no external network path
- Large files use S3 / MinIO to save local disk space
- Some teams use follower nodes separately, splitting the control plane from real placement
- Some teams bind independent policy groups and route differently from personal spaces

## Things to Check Before Using It

- For long-running deployments, write local storage directories as absolute paths; if you use relative paths, first confirm the service process working directory
- Local policies disable content deduplication by default; enable it only when you need to save disk space
- Configure object storage CORS before `presigned` uploads
- Before `presigned` downloads, confirm clients can reach object storage directly, and accept that bandwidth moves from AsterDrive nodes to object storage
- For remote policies, first confirm the bound follower node is enrolled, enabled, has a reachable `base_url`, and has an applied default ingress target
- Remote policies depend on internal remote storage protocol `v2` capability negotiation; if connection tests fail or the capability summary is incompatible, do not switch real traffic to it
- Before remote `presigned` uploads/downloads, confirm the browser can reach the follower node directly, and that the follower exposes `content-type` / `range` request headers and `ETag` / Range-related response headers externally
- Local storage / server-side temporary processing paths need enough local temporary directory space
- Neither S3 upload method performs content deduplication
- Single-file size limits, policy group rules, and user/team quotas can all affect upload success

## Changes You Should Not Make Directly

::: warning Do not change these on a policy that already has files written

- Local directory
- Bucket
- Endpoint
- Bound follower node

Old files are read from their original locations. Changing the location directly means existing files cannot be found.

A safer approach:

1. Create a new policy
2. Migrate old data
3. Switch users or teams to the policy group containing the new policy

:::

## Daily Maintenance

- Keep at least one usable default storage policy
- Keep at least one enabled default policy group
- Test the connection once before saving
- When assigning different storage routes to different users/teams, bind policy groups under `Admin -> Users` or `Admin -> Teams`
