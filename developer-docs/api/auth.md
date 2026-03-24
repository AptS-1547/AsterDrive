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

## 限流

认证相关接口带轻量限流：

- `/auth/login`：每秒 1 次，突发 5
- `/auth/register`：每秒 1 次，突发 3
- `/auth/setup`：每秒 1 次，突发 3
