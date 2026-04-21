# WebDAV API 与协议能力

WebDAV 相关内容可以分成三块：账号、挂载入口、协议能力。

## 账号接口

以下路径都相对于 `/api/v1`，且都需要认证。

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| `GET` | `/webdav-accounts` | 列出当前用户的 WebDAV 账号 |
| `POST` | `/webdav-accounts` | 创建 WebDAV 账号 |
| `DELETE` | `/webdav-accounts/{id}` | 删除 WebDAV 账号 |
| `POST` | `/webdav-accounts/{id}/toggle` | 启用或停用账号 |
| `GET` | `/webdav-accounts/settings` | 读取当前挂载前缀和客户端可直接使用的挂载地址 |
| `POST` | `/webdav-accounts/test` | 测试一组 WebDAV 凭据 |

常用点：

- 创建账号时，`password` 为空会自动生成随机密码
- 明文密码只在创建时返回一次
- `root_folder_id` 为空表示可访问整个用户空间
- `/toggle` 没有请求体，每调用一次就在启用 / 停用之间切换
- `/settings` 会返回两个字段：
  - `prefix`：服务端当前实际启用的挂载前缀
  - `endpoint`：面向客户端的可访问地址；如果配置了 `public_site_url`，这里会是绝对 URL，否则返回相对路径
- `/test` 用来先验账号密码，不必真的挂载客户端
- `GET /webdav-accounts` 是分页接口，支持 `limit` 和 `offset`

创建请求示例：

```json
{
  "username": "dav-demo",
  "password": null,
  "root_folder_id": 12
}
```

## 挂载地址

默认 WebDAV 路径是：

```text
/webdav
```

完整地址例如：

```text
http://localhost:3000/webdav
```

如果修改了 `[webdav].prefix`，挂载地址也会一起变化。

## 协议能力

当前已覆盖常见 WebDAV 方法：

- `PROPFIND`
- `MKCOL`
- `PUT`
- `GET`
- `DELETE`
- `COPY`
- `MOVE`
- `LOCK`
- `UNLOCK`
- `OPTIONS`

另外还补了最小 DeltaV 子集：

- `REPORT` 的 `DAV:version-tree`
- `VERSION-CONTROL`
- `OPTIONS` 的 `DAV: version-control`

这部分直接复用 `file_versions`，所以客户端可以读取历史版本树。

限制也很直接：

- `REPORT version-tree` 只支持文件
- 当前不是完整 DeltaV 服务器，只是最小可用子集

## 认证与运行时开关

- Basic Auth：使用 WebDAV 专用账号，可限制到 `root_folder_id`
- Bearer JWT：复用普通登录态，不受 `root_folder_id` 限制
- `webdav_enabled = false` 时，WebDAV 请求会直接返回 `503`

如果部署在反向代理后面，还要确认代理层允许 WebDAV 方法和相关请求头，见 [反向代理部署](../../docs/deployment/reverse-proxy.md)。
