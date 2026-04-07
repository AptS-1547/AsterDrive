# 登录与会话配置

```toml
[auth]
jwt_secret = "<随机生成的 32 字节十六进制字符串>"
bootstrap_insecure_cookies = false
```

`config.toml` 里的这一组现在只负责认证的静态引导项。
真正会影响浏览器 Cookie 安全策略和 Token 时长的值，已经迁到 `管理 -> 系统设置`。

大多数部署最需要确认的只有两项:

- `jwt_secret`
- `bootstrap_insecure_cookies`

## 最先确认的两项

### `jwt_secret`

首次自动生成配置时，服务会写入一个随机密钥。  
正式环境里不要随意改它，除非你准备让现有登录全部失效。

### `bootstrap_insecure_cookies`

- 纯 HTTP 首次引导环境: 设为 `true`
- 正式 HTTPS 部署: 保持 `false`

它只影响首次初始化 `system_config.auth_cookie_secure` 时写入什么默认值。
如果数据库里已经有这个运行时配置，再改这个静态项不会回写覆盖。

## 字段说明

| 字段 | 默认值 | 说明 |
| --- | --- | --- |
| `jwt_secret` | 首次启动自动生成 | JWT 签名密钥，正式环境应固定 |
| `bootstrap_insecure_cookies` | `false` | 首次初始化运行时配置时，是否把 `auth_cookie_secure` 设为 `false` |

## 首次部署时你通常要怎么做

### 本地或内网 HTTP 测试

```toml
[auth]
bootstrap_insecure_cookies = true
```

### 正式 HTTPS 部署

```toml
[auth]
bootstrap_insecure_cookies = false
```

如果你已经通过反向代理对外提供 HTTPS，通常就应该保持 `false`。

## 还需要知道的默认行为

- 第一个创建成功的账号会自动成为管理员
- 登录页会自动判断当前应该是“登录”“注册”还是“创建管理员”
- 当前版本默认允许新用户从登录页自行注册
- 当前版本暂时没有内置的“关闭注册”开关
- 修改 `jwt_secret` 后，现有登录会话会失效，需要重新登录
- `auth_cookie_secure` / `auth_access_token_ttl_secs` / `auth_refresh_token_ttl_secs`
  在 `管理 -> 系统设置` 中维护
- 新用户默认配额由 `管理 -> 系统设置` 里的 `default_storage_quota` 决定
- 新用户默认存储路线由系统默认策略组决定

## 一个常见的正式环境写法

```toml
[auth]
jwt_secret = "replace-with-your-own-secret"
bootstrap_insecure_cookies = false
```

也可以用环境变量覆盖:

```bash
ASTER__AUTH__JWT_SECRET="replace-with-your-own-secret"
ASTER__AUTH__BOOTSTRAP_INSECURE_COOKIES=false
```
