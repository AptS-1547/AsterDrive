# WebDAV API 与协议能力

WebDAV 相关能力包括三部分：

- 账号管理 REST API
- 实际的 WebDAV 挂载入口
- 与 WebDAV 相关的锁、属性和版本历史能力

## 账号管理 API

以下路径都相对于 `/api/v1`，且都需要认证。

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| `GET` | `/webdav-accounts` | 列出当前用户的 WebDAV 账号 |
| `POST` | `/webdav-accounts` | 创建 WebDAV 账号 |
| `DELETE` | `/webdav-accounts/{id}` | 删除 WebDAV 账号 |
| `POST` | `/webdav-accounts/{id}/toggle` | 启用或停用账号 |
| `POST` | `/webdav-accounts/test` | 测试一组 WebDAV 凭据 |

### `POST /webdav-accounts`

请求体：

```json
{
  "username": "dav-demo",
  "password": null,
  "root_folder_id": 12
}
```

行为说明：

- `password` 为空时会自动生成 16 位随机密码
- 明文密码只会在创建接口返回一次
- `root_folder_id` 为空表示可访问该用户的全部空间

## 实际 WebDAV 挂载地址

默认地址：

```text
/webdav
```

完整 URL 例如：

```text
http://localhost:3000/webdav
```

如果修改了 `[webdav].prefix`，挂载地址也会一起变化。

## 协议层当前支持的能力

### 标准 WebDAV

仓库测试已经覆盖并确认可用的常见方法包括：

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

### DeltaV 最小子集

当前仓库还额外补了 `dav-server` 不自带的最小 RFC3253 支持：

- `REPORT` 的 `DAV:version-tree`
- `VERSION-CONTROL`
- `OPTIONS` 的 `DAV: version-control`

这部分直接复用 `file_versions` 表，因此 WebDAV 客户端可以读取历史版本树。

当前限制：

- `REPORT version-tree` 只支持文件，不支持文件夹
- 目前实现的是“最小可用子集”，不是完整 DeltaV 服务器

## 认证方式

### Basic Auth

- 使用 `webdav_accounts` 中的专用用户名和密码
- 可限制到 `root_folder_id`

### Bearer JWT

- 复用普通登录 JWT
- 不受 `root_folder_id` 限制，直接访问整个用户空间

## 锁与属性

WebDAV 使用数据库锁与属性表：

- 锁记录保存在 `resource_locks`
- 自定义属性保存在 `entity_properties`
- 管理员可通过 `/api/v1/admin/locks` 查看和清理锁

REST 层的 `/files/{id}/lock` 与 `/folders/{id}/lock` 也是走同一套锁服务。

## 运行时开关

管理员把 `webdav_enabled` 设为 `"false"` 后，WebDAV 请求会直接返回 `503`。
