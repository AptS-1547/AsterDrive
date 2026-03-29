# API 概览

这页按功能分组带你找接口，不打算把每个接口都写成 OpenAPI 导出物。

除健康检查外，所有 REST 接口都挂在：

```text
/api/v1
```

## 统一响应格式

大多数 JSON 接口都使用统一包装：

```json
{
  "code": 0,
  "msg": "",
  "data": {}
}
```

字段含义：

- `code`：数字错误码，`0` 表示成功
- `msg`：错误消息；成功时通常为空
- `data`：响应体；部分成功接口会省略

## 不走统一 JSON 包装的接口

以下能力返回原始内容而不是 `ApiResponse`：

- 文件下载
- 文件缩略图
- 分享文件下载
- 分享缩略图
- WebDAV 协议响应
- Prometheus 指标

## 错误码分域

| 范围 | 含义 |
| --- | --- |
| `0` | 成功 |
| `1000-1099` | 通用错误 |
| `2000-2099` | 认证错误 |
| `3000-3099` | 文件、上传、锁、缩略图错误 |
| `4000-4099` | 存储策略与驱动错误 |
| `5000-5099` | 文件夹错误 |
| `6000-6099` | 分享错误 |

## 当前支持的认证方式

### REST / 前端

- HttpOnly Cookie
- `Authorization: Bearer <jwt>`

### WebDAV

- `Authorization: Basic ...`
- `Authorization: Bearer <jwt>`

## 模块索引

- [认证](/api/auth)
- [文件](/api/files)
- [文件夹](/api/folders)
- [批量操作](/api/batch)
- [分享](/api/shares)
- [回收站](/api/trash)
- [搜索](/api/search)
- [WebDAV](/api/webdav)
- [属性](/api/properties)
- [管理](/api/admin)
- [健康检查](/api/health)

其中比较值得优先看的几组能力是：

- 上传与版本：见 [文件](/api/files)
- 批量删除 / 移动 / 复制：见 [批量操作](/api/batch)
- 回收站恢复与清理：见 [回收站](/api/trash)
- 搜索与筛选：见 [搜索](/api/search)
- 公开分享：见 [分享](/api/shares)
- WebDAV 协议、账号与 DeltaV：见 [WebDAV](/api/webdav)
- 后台策略、锁、运行时配置与审计：见 [管理](/api/admin)

## OpenAPI 与 Swagger

如果你就是想要机器可读规范，也还是有两条路：

- `debug` 构建：访问 `/swagger-ui` 与 `/api-docs/openapi.json`
- 任意构建：运行 `cargo test --test generate_openapi` 导出静态规范到 `frontend-panel/generated/openapi.json`

## 继续阅读

- [认证](/api/auth)
- [文件](/api/files)
- [搜索](/api/search)
- [分享](/api/shares)
- [管理](/api/admin)
