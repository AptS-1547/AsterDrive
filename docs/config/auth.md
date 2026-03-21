# 认证配置

```toml
[auth]
jwt_secret = "<随机生成的 32 字节十六进制字符串>"
access_token_ttl_secs = 900
refresh_token_ttl_secs = 604800
```

## 字段说明

| 字段 | 类型 | 默认值 | 说明 |
| --- | --- | --- | --- |
| `jwt_secret` | string | 首次启动自动生成 | JWT 签名密钥，生产环境必须固定 |
| `access_token_ttl_secs` | u64 | `900` | Access token 有效期，默认 15 分钟 |
| `refresh_token_ttl_secs` | u64 | `604800` | Refresh token 有效期，默认 7 天 |

## 当前认证机制

- 登录成功后会写入两个 HttpOnly Cookie：
  - `aster_access`
  - `aster_refresh`
- `/api/v1/auth/me` 同时支持 Cookie 与 `Authorization: Bearer <token>`
- `/api/v1/auth/refresh` 当前只读取 refresh Cookie
- 当前实现会刷新 access token，但不会轮换 refresh token

## 注册行为

- 第一个注册用户自动成为管理员
- 新注册用户的 `storage_quota` 会从运行时配置 `default_storage_quota` 读取
- 该默认配额只影响未来新注册用户，不会回写已有用户

## 限流

认证相关接口内置轻量限流：

- `POST /auth/login`：`1 req/s`，突发 5
- `POST /auth/register`：`1 req/s`，突发 3

## 生产环境注意事项

如果继续使用自动生成的 `jwt_secret`，每次重启都会让已签发 token 全部失效。

推荐固定为显式值：

```toml
[auth]
jwt_secret = "your-fixed-secret-at-least-32-chars"
```

或通过环境变量覆盖：

```bash
ASTER__AUTH__JWT_SECRET="your-fixed-secret-at-least-32-chars"
```
