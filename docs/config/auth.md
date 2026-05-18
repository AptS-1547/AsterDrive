# 登录与会话

::: tip 这一篇分两层讲
- `config.toml` 里的 `[auth]` —— **只负责启动时的静态引导**（签名密钥、首次纯 HTTP 引导）
- `管理 -> 系统设置` —— **日常规则**（公开注册、Cookie、Token 有效期、激活 / 重置链接、各种冷却时间）

平时真正常改的几乎都在后台，本页静态部分只在初次部署或换机时碰一次。
:::

## `config.toml` 里的 `[auth]`

```toml
[auth]
jwt_secret = "<首次生成的一串随机密钥>"
bootstrap_insecure_cookies = false
```

### `jwt_secret`

首次自动生成配置时，服务会写入一段随机密钥。可以理解成"全站登录签名密钥"。

::: warning 正式环境固定它，避免来回改动
一旦修改：
- 当前所有登录会话失效
- 公开分享的密码验证 Cookie 失效
- 所有人都要重新登录
:::

### `bootstrap_insecure_cookies`

- **纯 HTTP 首次试跑** —— 临时设 `true`
- **正式 HTTPS 部署** —— 保持 `false`

它**只影响第一次初始化** `auth_cookie_secure` 时写入的默认值。如果数据库里已经有这个运行时设置，再改这里不会回写旧值。

## 登录页是按状态自动判断的

登录页不是固定的"登录"或"注册"页面，而是按当前状态走：

- **系统里还没有任何用户** —— 进入初始化流程，直接创建第一个管理员
- **系统里已有用户，输入的是现有账号** —— 登录
- **系统里已有用户，输入的是新账号，且管理员允许公开注册** —— 创建普通账号
- **管理员启用了外部认证提供商** —— 登录页会出现对应的外部登录入口
- **当前浏览器支持 Passkey** —— 登录页会显示 Passkey 登录入口，已登记 Passkey 的账号可以直接用设备解锁或安全密钥登录

需要注意：

- 第一个账号直接成为管理员，不走邮箱激活
- 后续公开注册的普通账号，要先点激活邮件才能登录
- 管理员关闭公开注册后，登录页只剩登录和找回密码

## Passkey 登录

Passkey 是每个用户自己管理的登录方式，入口在：

```text
设置 -> 安全 -> Passkey
```

用户可以在这里：

- 添加新的 Passkey
- 给 Passkey 改名，例如 `MacBook`、`iPhone` 或某把安全密钥
- 查看创建时间和最近使用时间
- 删除不再使用的 Passkey

添加时浏览器会打开系统自己的 WebAuthn / Passkey 验证窗口。正式部署要先填对 `管理 -> 系统设置 -> 站点配置 -> 公开站点地址`，并使用 HTTPS；本地 `localhost` / `127.0.0.1` 调试例外。浏览器通常只在安全上下文里开放完整 Passkey 能力。

Passkey 不替代本地密码。用户仍然可以继续使用密码登录；删除某个 Passkey 后，只是那台设备或那把安全密钥不能再直接登录当前账号。

## 外部认证 / SSO

管理员可以在这里接入外部身份提供商：

```text
管理 -> 外部认证
```

当前内置支持 OpenID Connect。创建提供商后，登录页会展示对应的外部登录入口；管理员需要把页面生成的重定向 URI 登记到身份提供商侧。

外部身份和本地用户的关系由提供商规则决定：

- 已绑定过的外部身份会直接登录对应本地用户
- 开启“按已验证邮箱自动绑定”后，身份提供商返回 `email_verified=true` 且本地存在唯一匹配邮箱时，系统可以自动绑定
- 开启“自动创建本地用户”后，未绑定身份可以自动创建普通用户
- 没开启自动绑定或自动创建时，用户需要先登录现有账号完成绑定，或按邮箱验证流程继续

用户已经绑定的外部身份在这里查看和解绑：

```text
设置 -> 安全 -> 外部身份
```

如果管理员开启了自动绑定，用户解绑后，后续满足相同规则的外部登录仍可能重新绑定到本地账号。

## 公开注册开关在哪

```text
管理 -> 系统设置 -> 用户管理 -> 允许公开注册新用户
```

关闭后：

- 外部用户不能再从登录页创建新账号
- 第一个管理员初始化流程仍然存在
- 管理员在后台手动创建的用户仍然可以使用

## 哪些功能依赖邮件配置

下面这些功能没邮件就用不了：

- 公开注册后的激活邮件
- 登录页的找回密码
- `设置 -> 安全` 里的邮箱改绑确认邮件
- 外部认证无法直接匹配本地账号时的邮箱验证流程

::: warning 先配通邮件，再开放注册
顺序反了的话，新用户账号已经创建出来，却收不到激活邮件，只会卡在"等待激活"。

准备开放这些能力前，先一起检查：
1. `管理 -> 系统设置 -> 邮件投递`
2. `管理 -> 系统设置 -> 站点配置 -> 公开站点地址`
3. 如果要接外部认证，再检查 `管理 -> 外部认证` 里的重定向 URI 是否已经登记到身份提供商侧
:::

## 常见写法

### 本地或内网 HTTP 试跑

```toml
[auth]
bootstrap_insecure_cookies = true
```

### 正式 HTTPS 部署

```toml
[auth]
jwt_secret = "replace-with-your-own-secret"
bootstrap_insecure_cookies = false
```

环境变量覆盖：

```bash
ASTER__AUTH__JWT_SECRET="replace-with-your-own-secret"
ASTER__AUTH__BOOTSTRAP_INSECURE_COOKIES=false
```

## 日常真正常改的是后台这些

下面这些不在 `config.toml` 里，全在后台维护：

- `auth_cookie_secure` —— Cookie 是否仅 HTTPS 发送
- `auth_access_token_ttl_secs` —— 访问令牌有效期
- `auth_refresh_token_ttl_secs` —— 刷新令牌有效期
- `auth_register_activation_ttl_secs` —— 注册激活链接有效期
- `auth_contact_change_ttl_secs` —— 邮箱改绑链接有效期
- `auth_password_reset_ttl_secs` —— 密码重置链接有效期
- `auth_contact_verification_resend_cooldown_secs` —— 验证邮件重发冷却
- `auth_password_reset_request_cooldown_secs` —— 密码重置请求冷却
- `auth_allow_user_registration` —— 公开注册开关
- `auth_register_activation_enabled` —— 新注册用户是否必须先完成邮箱激活
- 外部认证邮箱验证邮件模版 —— 在 `邮件投递` 分组里，供外部身份无法直接匹配本地账号时使用

具体说明见 [系统设置](/config/runtime)。
