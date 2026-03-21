# 快速开始

## 1. 启动服务

```bash
cargo run
```

首次启动会自动完成这些动作：

- 在当前工作目录生成 `config.toml`
- 创建默认 SQLite 数据库
- 执行数据库迁移
- 如果系统里还没有任何存储策略，自动创建默认本地策略 `Local Default`

默认地址：

```text
http://127.0.0.1:3000
```

## 2. 注册第一个账号

第一个注册的用户会自动成为管理员。

```bash
curl -X POST http://localhost:3000/api/v1/auth/register \
  -H "Content-Type: application/json" \
  -d '{"username":"admin","email":"admin@example.com","password":"your-password"}'
```

## 3. 登录

```bash
curl -X POST http://localhost:3000/api/v1/auth/login \
  -H "Content-Type: application/json" \
  -c cookies.txt \
  -d '{"username":"admin","password":"your-password"}'
```

登录成功后服务会写入两个 HttpOnly Cookie：

- `aster_access`
- `aster_refresh`

## 4. 打开管理面板

浏览器访问：

```text
http://localhost:3000
```

当前前端已落地的主要页面包括：

- 文件浏览器 `/`
- 回收站 `/trash`
- WebDAV 账号管理 `/settings/webdav`
- 管理后台 `/admin/*`
- 公开分享页 `/s/:token`

## 5. 上传第一个文件

小文件可直接 multipart 上传：

```bash
curl -X POST http://localhost:3000/api/v1/files/upload \
  -b cookies.txt \
  -F "file=@/path/to/file.pdf"
```

更推荐先调用协商接口：

```text
POST /api/v1/files/upload/init
```

服务端会根据当前存储策略返回三种模式之一：

- `direct`
- `chunked`
- `presigned`

## 6. 创建分享

```bash
curl -X POST http://localhost:3000/api/v1/shares \
  -b cookies.txt \
  -H "Content-Type: application/json" \
  -d '{"file_id":1,"password":"123456","max_downloads":10}'
```

公开 API 地址：

```text
/api/v1/s/{token}
```

公开前端页面地址：

```text
/s/{token}
```

## 7. 配置管理员常用项

启动后通常还要做三件事：

1. 检查默认存储策略与用户策略分配
2. 为新用户设置默认配额或单独配额
3. 设置运行时开关，例如 WebDAV 开关、回收站保留天数、版本保留数量

对应页面：

- `/admin/users`
- `/admin/policies`
- `/admin/settings`

## 8. 健康检查与 OpenAPI

- `GET /health`
- `GET /health/ready`
- `GET /health/memory`

OpenAPI 有两种使用方式：

- `debug` 构建下访问 `http://localhost:3000/swagger-ui`
- 运行 `cargo test --test generate_openapi` 生成静态规范
