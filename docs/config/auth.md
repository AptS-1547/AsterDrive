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

::: warning 正式环境固定它，别来回改
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

需要注意：

- 第一个账号直接成为管理员，不走邮箱激活
- 后续公开注册的普通账号，要先点激活邮件才能登录
- 管理员关闭公开注册后，登录页只剩登录和找回密码

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

::: warning 别先开放注册再回头补邮件
顺序反了的话，新用户账号已经创建出来，却收不到激活邮件，只会卡在"等待激活"。

准备开放这些能力前，先一起检查：
1. `管理 -> 系统设置 -> 邮件投递`
2. `管理 -> 系统设置 -> 站点配置 -> 公开站点地址`
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

具体说明见 [系统设置](/config/runtime)。
