# 登录与会话配置

登录、注册和会话相关的设置，分成两层：

- `config.toml` 里的 `[auth]`：只负责启动时的静态引导
- `管理 -> 系统设置`：负责公开注册、Cookie 安全策略、Token 有效期、激活邮件和密码重置这类日常规则

## `config.toml` 里的 `[auth]`

```toml
[auth]
jwt_secret = "<首次生成的一串随机密钥>"
bootstrap_insecure_cookies = false
```

### `jwt_secret`

首次自动生成配置时，服务会写入一个随机密钥。  
你可以把它理解成“全站登录签名密钥”。  
正式环境里要固定它，不要来回改。

一旦修改：

- 当前登录会话会失效
- 公开分享的密码验证 Cookie 也会失效
- 所有人都需要重新登录

### `bootstrap_insecure_cookies`

- 纯 HTTP 首次试跑：临时设为 `true`
- 正式 HTTPS 部署：保持 `false`

它只影响第一次初始化 `auth_cookie_secure` 时写入什么默认值。  
如果数据库里已经有这个运行时设置，再改这里不会自动回写旧值。

## 登录页现在的真实流程

登录页不是固定的“登录页”或“注册页”，而是按当前状态自动判断：

- 系统里还没有任何用户：进入初始化流程，直接创建第一个管理员账号
- 系统里已经有用户，且输入的是现有账号：登录
- 系统里已经有用户，且输入的是新账号，同时管理员允许公开注册：创建普通账号

需要注意：

- 第一个账号会直接成为管理员，不需要走邮箱激活
- 后续公开注册出来的普通账号，需要先点激活邮件才能登录
- 如果管理员关闭了公开注册，登录页就只保留登录和找回密码

## 公开注册开关现在在哪里

文档里以前写“没有关闭注册的开关”，这已经过时了。

当前实际入口：

```text
管理 -> 系统设置 -> 用户管理 -> 允许公开注册新用户
```

关闭后：

- 外部用户不能再从登录页创建新账号
- 第一个管理员初始化流程仍然存在
- 管理员在后台创建的用户仍然可以使用

## 邮箱激活、找回密码和改绑邮箱依赖什么

下面这些功能都依赖邮件配置：

- 公开注册后的激活邮件
- 登录页里的找回密码
- `设置 -> 安全` 里的邮箱改绑确认邮件

所以只要你准备开放这些能力，就要一起检查：

1. `管理 -> 系统设置 -> 邮件投递`
2. `管理 -> 系统设置 -> 站点配置 -> 公开站点地址`

## 常见场景

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

也可以用环境变量覆盖：

```bash
ASTER__AUTH__JWT_SECRET="replace-with-your-own-secret"
ASTER__AUTH__BOOTSTRAP_INSECURE_COOKIES=false
```

## 日常真正常改的是哪里

下面这些不是改 `config.toml`，而是在后台系统设置里维护：

- `auth_cookie_secure`
- `auth_access_token_ttl_secs`
- `auth_refresh_token_ttl_secs`
- `auth_register_activation_ttl_secs`
- `auth_contact_change_ttl_secs`
- `auth_password_reset_ttl_secs`
- `auth_contact_verification_resend_cooldown_secs`
- `auth_password_reset_request_cooldown_secs`
- `auth_allow_user_registration`

具体说明见 [系统设置](/config/runtime)。
