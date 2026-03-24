# 登录与会话配置

```toml
[auth]
jwt_secret = "<随机生成的 32 字节十六进制字符串>"
access_token_ttl_secs = 900
refresh_token_ttl_secs = 604800
cookie_secure = true
```

## 字段说明

| 字段 | 类型 | 默认值 | 说明 |
| --- | --- | --- | --- |
| `jwt_secret` | string | 首次启动自动生成 | JWT 签名密钥，生产环境必须固定 |
| `access_token_ttl_secs` | u64 | `900` | 短期登录令牌有效期，默认 15 分钟 |
| `refresh_token_ttl_secs` | u64 | `604800` | 用于续期登录会话的长期令牌有效期，默认 7 天 |
| `cookie_secure` | bool | `true` | 是否只允许浏览器通过 HTTPS 发送登录 Cookie |

## 最重要的一项：`cookie_secure`

### 本地 HTTP 测试

```toml
[auth]
cookie_secure = false
```

### 正式 HTTPS 部署

```toml
[auth]
cookie_secure = true
```

如果你已经通过反向代理对外提供 HTTPS，通常就应该保持 `true`。

## 其他需要知道的事情

- 第一个注册成功的账号会自动成为管理员
- 修改 `jwt_secret` 后，现有登录会话会失效，需要重新登录
- 新用户默认配额由管理员后台里的系统设置决定

## 推荐写法

```toml
[auth]
jwt_secret = "your-fixed-secret-at-least-32-chars"
access_token_ttl_secs = 900
refresh_token_ttl_secs = 604800
cookie_secure = true
```

或通过环境变量覆盖：

```bash
ASTER__AUTH__JWT_SECRET="your-fixed-secret-at-least-32-chars"
ASTER__AUTH__COOKIE_SECURE=true
```
