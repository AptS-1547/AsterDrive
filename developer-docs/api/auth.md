# 认证 API

以下路径都相对于 `/api/v1`。

## 一览

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| `POST` | `/auth/check` | 返回公开认证状态（系统是否已初始化、是否允许公开注册） |
| `POST` | `/auth/setup` | 初始化系统并创建首个管理员 |
| `POST` | `/auth/register` | 注册用户；第一个用户自动成为管理员 |
| `POST` | `/auth/register/resend` | 重发注册激活邮件 |
| `GET` | `/auth/contact-verification/confirm` | 消费邮箱验证 token 并重定向前端 |
| `POST` | `/auth/password/reset/request` | 请求密码重置邮件 |
| `POST` | `/auth/password/reset/confirm` | 使用 token 完成密码重置 |
| `POST` | `/auth/login` | 登录并写入认证 Cookie |
| `POST` | `/auth/passkeys/login/start` | 发起 WebAuthn Passkey 登录挑战 |
| `POST` | `/auth/passkeys/login/finish` | 完成 Passkey 登录并写入认证 Cookie |
| `GET` | `/auth/external-auth/providers` | 列出匿名态可用的外部认证提供商 |
| `POST` | `/auth/external-auth/{kind}/{provider}/start` | 发起外部认证登录 |
| `GET` | `/auth/external-auth/{kind}/{provider}/callback` | 外部认证回调入口 |
| `POST` | `/auth/external-auth/email-verification/start` | 为外部认证补验邮箱发送邮件 |
| `GET` | `/auth/external-auth/email-verification/confirm` | 完成外部认证邮箱补验并重定向前端 |
| `POST` | `/auth/external-auth/password-link` | 用本地密码把外部身份绑定到已有账号 |
| `POST` | `/auth/refresh` | 使用 refresh Cookie 轮换 access/refresh token |
| `POST` | `/auth/logout` | 清除认证 Cookie |
| `GET` | `/auth/me` | 读取当前登录用户信息 |
| `GET` | `/auth/sessions` | 列出当前用户的活跃登录会话 |
| `DELETE` | `/auth/sessions/others` | 吊销除当前 refresh session 外的其他会话 |
| `DELETE` | `/auth/sessions/{id}` | 吊销指定登录会话 |
| `PUT` | `/auth/password` | 修改当前用户密码 |
| `GET` | `/auth/passkeys` | 列出当前用户已注册的 Passkey |
| `POST` | `/auth/passkeys/register/start` | 发起 Passkey 注册挑战 |
| `POST` | `/auth/passkeys/register/finish` | 完成 Passkey 注册 |
| `PATCH` | `/auth/passkeys/{id}` | 重命名 Passkey |
| `DELETE` | `/auth/passkeys/{id}` | 删除 Passkey |
| `GET` | `/auth/external-auth/links` | 列出当前用户绑定的外部认证身份 |
| `DELETE` | `/auth/external-auth/links/{id}` | 解绑外部认证身份 |
| `POST` | `/auth/email/change` | 请求变更当前登录用户邮箱 |
| `POST` | `/auth/email/change/resend` | 重发邮箱变更确认邮件 |
| `PATCH` | `/auth/preferences` | 更新当前用户偏好设置 |
| `PATCH` | `/auth/profile` | 更新当前用户资料 |
| `POST` | `/auth/profile/avatar/upload` | 上传头像图片 |
| `PUT` | `/auth/profile/avatar/source` | 切换头像来源 |
| `GET` | `/auth/events/storage` | 订阅当前用户可见工作空间的存储变更事件 |
| `GET` | `/auth/profile/avatar/{size}` | 读取当前用户已上传头像 |

## 初始化与注册

- `POST /auth/check`：返回 `has_users` 和 `allow_user_registration`，只用于判断实例处于初始化、登录还是“关闭公开注册”的大状态，不会公开暴露账号是否存在
  这条接口当前不需要请求体。
- `POST /auth/setup`：仅在系统还没有任何用户时可用，用来创建首个管理员
- `POST /auth/register`：普通注册入口；当 `auth_allow_user_registration = true` 时可用。第一个注册用户自动成为 `admin`，新用户默认配额来自 `default_storage_quota`
- `POST /auth/register/resend`：对“尚未完成激活”的账号重发确认邮件，请求体如下：

```json
{
  "identifier": "admin@example.com"
}
```

公开请求的重发与找回流程都会做最短响应时间填充，尽量避免把账号存在性直接暴露给外部。

如果运营方关闭了 `auth_allow_user_registration`：

- `/auth/register` 会返回 `403`
- `/auth/setup` 仍然可以在系统尚未初始化时创建首个管理员

`/auth/setup` 和 `/auth/register` 的请求体相同：

```json
{
  "username": "admin",
  "email": "admin@example.com",
  "password": "password"
}
```

## 登录态

`POST /auth/login` 使用下面的请求体：

```json
{
  "identifier": "admin",
  "password": "password"
}
```

成功后会写入两个 HttpOnly Cookie：

- `aster_access`
- `aster_refresh`

其中 `aster_refresh` 的 Cookie Path 是 `/api/v1/auth`，会随 `/api/v1/auth/*` 下的请求一起发送。

相关接口：

- `POST /auth/refresh`：读取 refresh Cookie，原子消费旧 refresh token，签发新的 access/refresh token；旧 refresh token 再次使用会被视为复用攻击并撤销该用户全部会话
- `POST /auth/logout`：清除两个认证 Cookie，并吊销当前 refresh token
- `GET /auth/me`：既支持 Cookie，也支持 `Authorization: Bearer <jwt>`
- `GET /auth/sessions`：列出当前用户的活跃登录设备 / 会话；如果请求带当前 refresh Cookie，会标记当前会话
- `DELETE /auth/sessions/others`：要求当前请求能定位到 refresh session，只吊销其他会话
- `DELETE /auth/sessions/{id}`：吊销指定会话；如果删的是当前会话，响应会同时清理认证 Cookie

如果用户状态是 `disabled`，登录会直接失败。

### Passkey 登录与管理

Passkey 使用 WebAuthn 两段式流程。所有 challenge 响应和 credential 请求体都保持 WebAuthn 原始 JSON 结构，由浏览器的 `navigator.credentials.*` 直接消费或回传。

登录流程：

- `POST /auth/passkeys/login/start`：请求体可传 `{ "identifier": "alice", "conditional": false }`；`identifier` 可省略，用于 conditional UI / discoverable credential 场景
- `POST /auth/passkeys/login/finish`：请求体是 `{ "flow_id": "...", "credential": { ... } }`；成功后和密码登录一样写入 `aster_access`、`aster_refresh` 和 CSRF Cookie

注册和管理流程需要已登录：

- `GET /auth/passkeys`：返回当前用户的 Passkey 列表
- `POST /auth/passkeys/register/start`：请求体可传 `{ "name": "MacBook Touch ID" }`
- `POST /auth/passkeys/register/finish`：请求体是 `{ "flow_id": "...", "credential": { ... }, "name": "MacBook Touch ID" }`
- `PATCH /auth/passkeys/{id}`：请求体是 `{ "name": "New name" }`
- `DELETE /auth/passkeys/{id}`：删除当前用户自己的 Passkey

当前 Passkey 记录保存在 `passkeys` 表，credential 以强类型包装后的 JSON 存储。服务端要求可发现凭证；不支持的 credential 会返回带 `passkey.*` 子码的校验错误。

### 外部认证

外部认证当前实现的 provider kind 是 `oidc`，管理端通过 `/admin/external-auth/*` 配置。匿名登录页先调用 `GET /auth/external-auth/providers` 读取启用中的 provider：

```json
{
  "code": 0,
  "msg": "",
  "data": [
    {
      "key": "corp",
      "kind": "oidc",
      "display_name": "Corp SSO",
      "icon_url": "/static/external-auth/corp.svg"
    }
  ]
}
```

登录流程：

- `POST /auth/external-auth/{kind}/{provider}/start`：请求体可传 `{ "return_path": "/files" }`，返回 `authorization_url`
- 浏览器跳到 `authorization_url` 后，OIDC 提供商回调 `GET /auth/external-auth/{kind}/{provider}/callback`
- 回调成功时服务端写入认证 Cookie，并 `302` 到 `return_path`

如果 provider 返回的身份缺少可用邮箱，而当前策略需要邮箱确认，回调会重定向到登录页并带上 `external_auth=email_required` 和 `flow`。随后前端使用：

- `POST /auth/external-auth/email-verification/start`：请求体 `{ "flow_token": "...", "email": "alice@example.com" }`
- `GET /auth/external-auth/email-verification/confirm?token=...`：消费邮件里的 token，成功后写 Cookie 并重定向

如果外部身份需要绑定已有本地账号，可以调用：

```json
{
  "flow_token": "...",
  "identifier": "alice",
  "password": "local-password"
}
```

对应接口是 `POST /auth/external-auth/password-link`，成功后同样写入认证 Cookie。

登录后用户可管理自己的外部身份绑定：

- `GET /auth/external-auth/links`
- `DELETE /auth/external-auth/links/{id}`

外部认证临时 login flow 和 email verification flow 会由 primary 后台的 `external-auth-flow-cleanup` 周期任务清理。

### Cookie 写操作的 CSRF 来源判断

使用 Cookie 鉴权执行不安全方法时，服务端同时检查双提交 CSRF token 和请求来源：

- `same-origin` 请求允许继续做 CSRF token 校验
- `same-site` 请求必须带可信 `Origin` 或 `Referer`
- 可信来源必须精确匹配当前请求来源，或命中 `public_site_url` 列表中的某个 HTTP(S) origin
- `cross-site`、非法 `Sec-Fetch-Site`、不可信 `Origin` / `Referer`、以及缺少可信来源的 `same-site` 请求都会被拒绝

这里的 `public_site_url` 是运行时配置里的公开站点来源列表，不是 CORS 白名单。它的作用是声明哪些同站公开入口属于本实例，避免把浏览器层面的 `same-site` 直接等同于可信。

## 当前用户资料、密码与偏好

- `PUT /auth/password`：修改当前用户密码，请求体如下：

```json
{
  "current_password": "old-password",
  "new_password": "new-password"
}
```

这个接口会校验当前密码；新密码仍然走和注册相同的长度校验。

- `GET /auth/me`：返回的 `preferences` 除了内置界面偏好外，还可能包含 `preferences.custom`，用于自定义前端读写自己的用户级 KV 配置
- `PATCH /auth/preferences`：只会合并请求体里非 `null` 的内置字段，并返回完整的最新偏好对象；当前偏好里也包含 `storage_event_stream_enabled`
  还支持两个和自定义前端有关的字段：
  - `custom`：把任意 JSON 值按 key 合并写入当前用户的自定义偏好
  - `remove_custom_keys`：显式删除一组自定义偏好 key
  自定义 key 不能和内置字段重名（例如 `theme_mode`、`language`）
- `PATCH /auth/profile`：当前只支持修改 `display_name`

`PATCH /auth/preferences` 的一个自定义 KV 示例：

```json
{
  "language": "zh",
  "custom": {
    "my-frontend.sidebar": { "collapsed": true },
    "my-frontend.accent": "sunset"
  },
  "remove_custom_keys": ["my-frontend.legacy"]
}
```

## 联系方式验证与密码重置

- `GET /auth/contact-verification/confirm?token=...`：这是浏览器入口，不返回 JSON，而是消费 token 后 `302` 重定向到前端页面。注册激活和邮箱变更都复用这条确认路径
- `POST /auth/email/change`：请求体是 `{ "new_email": "new@example.com" }`，会为当前登录用户写入待确认邮箱并发送确认邮件
- `POST /auth/email/change/resend`：对当前登录用户尚未完成的邮箱变更请求重发确认邮件
- `POST /auth/password/reset/request`：请求体是 `{ "email": "alice@example.com" }`，如果地址有效会发密码重置邮件；对外仍返回“请求已接受”的统一成功响应
- `POST /auth/password/reset/confirm`：请求体如下：

```json
{
  "token": "reset-token",
  "new_password": "new-password"
}
```

密码重置成功后，不需要当前登录态；接口会直接校验 token、写入新密码并记审计日志。

## 头像

头像相关接口都需要登录：

- `POST /auth/profile/avatar/upload`：`multipart/form-data` 上传图片，后端会生成 WebP 头像资源
- `PUT /auth/profile/avatar/source`：只能在 `none` 和 `gravatar` 之间切换；`upload` 来源必须通过上传接口设置
- `GET /auth/profile/avatar/{size}`：只读取“已上传头像”的 WebP 资源，当前支持 `512` 和 `1024`

也就是说：

- 如果你要把头像来源切到上传图，应该调用 `/auth/profile/avatar/upload`
- 如果当前来源是 `gravatar` 或 `none`，应优先使用 `/auth/me` 或资料更新接口返回的 `profile.avatar.url_*`

公开分享页和管理员接口会复用同一套头像资源，但读取路径不同。

## 实时存储事件

`GET /auth/events/storage` 是登录后可用的 SSE 接口，返回 `text/event-stream`，不是普通 JSON：

- 只会推送当前用户可见的个人空间和团队空间事件
- 空闲时每 15 秒发一次 `: keep-alive`
- 如果订阅端落后太多，服务端会发一个 `sync.required` 事件，提示前端整页重新同步
- 前端当前会用 `EventSource(..., { withCredentials: true })` 走 Cookie 鉴权
- 用户可通过偏好 `storage_event_stream_enabled = false` 关闭这条事件流

## 限流

`/auth` 整个 scope 共用同一档认证限流配置，不再按单个接口分别硬编码。

默认配置来自 `[rate_limit].auth`：

- `seconds_per_request = 2`
- `burst_size = 5`

如果全局 `rate_limit.enabled = false`，则不会启用这层限流。
