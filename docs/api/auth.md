# 认证 API

以下路径都相对于 `/api/v1`。

## 接口列表

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| `POST` | `/auth/register` | 注册用户；第一个用户自动成为管理员 |
| `POST` | `/auth/login` | 登录并写入认证 Cookie |
| `POST` | `/auth/refresh` | 使用 refresh Cookie 换新的 access token |
| `POST` | `/auth/logout` | 清除认证 Cookie |
| `GET` | `/auth/me` | 读取当前登录用户信息 |

## `POST /auth/register`

请求体：

```json
{
  "username": "admin",
  "email": "admin@example.com",
  "password": "password"
}
```

当前实现行为：

- `username` 与 `email` 必须唯一
- 第一个注册用户自动成为 `admin`
- 新用户默认配额来自运行时配置 `default_storage_quota`

## `POST /auth/login`

请求体：

```json
{
  "username": "admin",
  "password": "password"
}
```

成功后会设置两个 HttpOnly Cookie：

- `aster_access`
- `aster_refresh`

如果用户状态是 `disabled`，会直接返回禁止访问。

## `POST /auth/refresh`

使用 `aster_refresh` Cookie 刷新 access token。

当前实现注意点：

- 只读取 refresh Cookie
- 不支持 Bearer header 刷新
- 只会签发新的 access token，不会轮换 refresh token

## `POST /auth/logout`

清除 `aster_access` 与 `aster_refresh` Cookie。

## `GET /auth/me`

支持两种认证方式：

- 浏览器 Cookie
- `Authorization: Bearer <jwt>`

## 限流

认证相关接口带轻量限流：

- `/auth/login`：每秒 1 次，突发 5
- `/auth/register`：每秒 1 次，突发 3
