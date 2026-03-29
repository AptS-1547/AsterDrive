# 认证 API

以下路径都相对于 `/api/v1`。

## 一览

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| `POST` | `/auth/check` | 检查用户名或邮箱是否已存在，并返回系统是否已初始化 |
| `POST` | `/auth/setup` | 初始化系统并创建首个管理员 |
| `POST` | `/auth/register` | 注册用户；第一个用户自动成为管理员 |
| `POST` | `/auth/login` | 登录并写入认证 Cookie |
| `POST` | `/auth/refresh` | 使用 refresh Cookie 换新的 access token |
| `POST` | `/auth/logout` | 清除认证 Cookie |
| `GET` | `/auth/me` | 读取当前登录用户信息 |
| `PUT` | `/auth/password` | 修改当前用户密码 |
| `PATCH` | `/auth/preferences` | 更新当前用户偏好设置 |
| `PATCH` | `/auth/profile` | 更新当前用户资料 |
| `POST` | `/auth/profile/avatar/upload` | 上传头像图片 |
| `PUT` | `/auth/profile/avatar/source` | 切换头像来源 |
| `GET` | `/auth/profile/avatar/{size}` | 读取当前用户已上传头像 |

## 初始化与注册

- `POST /auth/check`：提交 `identifier`，返回 `exists` 和 `has_users`，主要给前端初始化流程做预检查
- `POST /auth/setup`：仅在系统还没有任何用户时可用，用来创建首个管理员
- `POST /auth/register`：普通注册入口；第一个注册用户自动成为 `admin`，新用户默认配额来自 `default_storage_quota`

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

相关接口：

- `POST /auth/refresh`：只读取 refresh Cookie，签发新的 access token，不轮换 refresh token
- `POST /auth/logout`：清除两个认证 Cookie
- `GET /auth/me`：既支持 Cookie，也支持 `Authorization: Bearer <jwt>`

如果用户状态是 `disabled`，登录会直接失败。

## 当前用户资料、密码与偏好

- `PUT /auth/password`：修改当前用户密码，请求体如下：

```json
{
  "current_password": "old-password",
  "new_password": "new-password"
}
```

这个接口会校验当前密码；新密码仍然走和注册相同的长度校验。

- `PATCH /auth/preferences`：只会合并请求体里非 `null` 的字段，并返回完整的最新偏好对象
- `PATCH /auth/profile`：当前只支持修改 `display_name`

## 头像

头像相关接口都需要登录：

- `POST /auth/profile/avatar/upload`：`multipart/form-data` 上传图片，后端会生成 WebP 头像资源
- `PUT /auth/profile/avatar/source`：只能在 `none` 和 `gravatar` 之间切换；`upload` 来源必须通过上传接口设置
- `GET /auth/profile/avatar/{size}`：只读取“已上传头像”的 WebP 资源，当前支持 `512` 和 `1024`

也就是说：

- 如果你要把头像来源切到上传图，应该调用 `/auth/profile/avatar/upload`
- 如果当前来源是 `gravatar` 或 `none`，应优先使用 `/auth/me` 或资料更新接口返回的 `profile.avatar.url_*`

公开分享页和管理员接口会复用同一套头像资源，但读取路径不同。

## 限流

`/auth` 整个 scope 共用同一档认证限流配置，不再按单个接口分别硬编码。

默认配置来自 `[rate_limit].auth`：

- `seconds_per_request = 2`
- `burst_size = 5`

如果全局 `rate_limit.enabled = false`，则不会启用这层限流。
