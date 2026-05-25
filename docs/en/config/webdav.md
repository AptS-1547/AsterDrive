# WebDAV

::: tip This page has two layers

- **`[webdav]` in `config.toml`** - Path prefix and hard upload size limit. **Requires a restart after changes.**
- **`Admin -> System Settings -> WebDAV`** - The global switch. Disabling it takes effect immediately without a restart.

Regular WebDAV users usually only need to create a dedicated account on the `WebDAV` page in the left sidebar of their personal space, then enter the address in Finder, Windows, or rclone.
:::

## Static Configuration in `config.toml`

```toml
[webdav]
prefix = "/webdav"
payload_limit = 10737418240
```

| Option | Default | Purpose |
| --- | --- | --- |
| `prefix` | `"/webdav"` | WebDAV path prefix. Client addresses must change with it. |
| `payload_limit` | `10737418240` | Hard WebDAV upload size limit. Default is 10 GiB. |

::: warning Restart the service after changing these two options
Unlike the runtime global switch, static configuration is read only once during startup.
:::

## Runtime Switch in the Admin Console

`Admin -> System Settings -> WebDAV -> Enable WebDAV`. After it is disabled, desktop clients disconnect immediately. **No restart is required.**

## Standard Usage for Regular Users

1. Create a dedicated account on the `WebDAV` page in the left sidebar of your personal space
2. Set a username and password
3. Optionally restrict it to a folder under the root directory
4. Enter the address, username, and password in Finder, Windows Explorer, rclone, or Mountain Duck

::: tip Use a dedicated account. Do not reuse the web login password.
A WebDAV dedicated account has independently managed password and scope. Losing it will not affect the main account.
:::

## Default Address

```text
https://your-domain/webdav/
```

If you change `prefix` to `/dav`, change the client address too:

```text
https://your-domain/dav/
```

## Large Uploads Depend on Three Limits

When uploading large files through WebDAV, these three limits apply, and **the smallest one wins**:

1. `webdav.payload_limit`
2. Reverse proxy upload size limit, such as Nginx `client_max_body_size` or Caddy equivalents
3. Single-file size limit in the storage policy

If any one of them blocks the upload, the whole upload is blocked. Check all three while troubleshooting.

## Do Not Drop These When Using a Reverse Proxy

::: warning WebDAV is not only GET/PUT
WebDAV uses a set of extension methods and headers that reverse proxies often drop by default. Make sure the proxy layer forwards:

**Headers:** `Authorization`, `Depth`, `Destination`, `Overwrite`, `If`, `Lock-Token`, `Timeout`

**Methods:** `PROPFIND`, `PROPPATCH`, `MKCOL`, `MOVE`, `COPY`, `LOCK`, `UNLOCK`
:::

See [reverse proxy deployment](/en/deployment/reverse-proxy) for complete reverse proxy examples.
