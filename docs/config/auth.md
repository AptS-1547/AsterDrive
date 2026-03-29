# 登录与会话配置

```toml
[auth]
jwt_secret = "<随机生成的 32 字节十六进制字符串>"
access_token_ttl_secs = 900
refresh_token_ttl_secs = 604800
cookie_secure = true
```

这一组配置主要影响登录和会话。
大多数部署最需要确认的其实只有两项:

- `jwt_secret`
- `cookie_secure`

## 最先确认的两项

### `jwt_secret`

首次自动生成配置时，服务会写入一个随机密钥。  
正式环境里不要随意改它，除非你准备让现有登录全部失效。

### `cookie_secure`

- 纯 HTTP 测试环境: 设为 `false`
- 正式 HTTPS 部署: 保持 `true`

## 字段说明

| 字段 | 默认值 | 说明 |
| --- | --- | --- |
| `jwt_secret` | 首次启动自动生成 | JWT 签名密钥，正式环境应固定 |
| `access_token_ttl_secs` | `900` | 短期登录令牌有效期，默认 15 分钟 |
| `refresh_token_ttl_secs` | `604800` | 长期续期令牌有效期，默认 7 天 |
| `cookie_secure` | `true` | 是否只允许浏览器通过 HTTPS 发送登录 Cookie |

## `cookie_secure` 怎么选

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

## 还需要知道的几件事

- 第一个创建成功的账号会自动成为管理员
- 登录页会自动判断当前应该是“登录”“注册”还是“创建管理员”
- 当前版本默认允许新用户从登录页自行注册
- 修改 `jwt_secret` 后，现有登录会话会失效，需要重新登录
- 新用户默认配额由 `管理 -> 系统设置` 里的 `default_storage_quota` 决定

## 一个常见的正式环境写法

```toml
[auth]
jwt_secret = "replace-with-your-own-secret"
access_token_ttl_secs = 900
refresh_token_ttl_secs = 604800
cookie_secure = true
```

也可以用环境变量覆盖:

```bash
ASTER__AUTH__JWT_SECRET="replace-with-your-own-secret"
ASTER__AUTH__COOKIE_SECURE=true
```
