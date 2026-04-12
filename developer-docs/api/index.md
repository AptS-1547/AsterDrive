# API 概览

这页按功能分组带你找接口，不打算把每个接口都写成 OpenAPI 导出物。

绝大多数 JSON / REST 接口都挂在：

```text
/api/v1
```

当前明确不在这个前缀下的入口有：

- 健康检查：`/health*`
- 直接下载链接：`/d/{token}/{filename}`
- 预览下载链接：`/pv/{token}/{filename}`

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
- 直接下载链接
- 预览下载链接
- 文件缩略图
- 分享文件下载
- 分享缩略图
- 当前用户已上传头像
- 管理员读取用户已上传头像
- 分享拥有者已上传头像
- 当前用户存储变更事件流
- WOPI `CheckFileInfo` 与内容回调
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

## 工作空间作用域

当前有两类受保护工作空间：

- 个人空间：接口直接挂在 `/files`、`/folders`、`/batch`、`/search`、`/shares`、`/trash`
- 团队空间：复用同一套语义，但统一加前缀 `/teams/{team_id}`

常见团队路径长这样：

```text
/api/v1/teams/{team_id}/folders
/api/v1/teams/{team_id}/files/{id}
/api/v1/teams/{team_id}/batch/move
/api/v1/teams/{team_id}/search
/api/v1/teams/{team_id}/shares
/api/v1/teams/{team_id}/trash
```

也就是说，团队空间不是另一套业务模型，而是把同一套文件 / 文件夹 / 搜索 / 回收站语义切到团队作用域下执行。

## 模块索引

- [认证](/api/auth)
- [文件](/api/files)
- [文件夹](/api/folders)
- [团队与团队空间](/api/teams)
- [批量操作](/api/batch)
- [分享](/api/shares)
- [回收站](/api/trash)
- [搜索](/api/search)
- [后台任务](/api/tasks)
- [WOPI](/api/wopi)
- [WebDAV](/api/webdav)
- [属性](/api/properties)
- [公共接口](/api/public)
- [管理](/api/admin)
- [健康检查](/api/health)

其中比较值得优先看的几组能力是：

- 上传与版本：见 [文件](/api/files)
- 批量删除 / 移动 / 复制：见 [批量操作](/api/batch)
- 回收站恢复与清理：见 [回收站](/api/trash)
- 搜索与筛选：见 [搜索](/api/search)
- 后台任务列表与重试：见 [后台任务](/api/tasks)
- 团队管理与团队工作空间：见 [团队与团队空间](/api/teams)
- 公开分享：见 [分享](/api/shares)
- Office / WOPI 预览与回调：见 [WOPI](/api/wopi)
- WebDAV 协议、账号与 DeltaV：见 [WebDAV](/api/webdav)
- 登录页与公开页启动配置：见 [公共接口](/api/public)
- 后台策略、锁、运行时配置与审计：见 [管理](/api/admin)

## OpenAPI 与 Swagger

如果你就是想要机器可读规范，也还是有两条路：

- `debug` 构建：访问 `/swagger-ui` 与 `/api-docs/openapi.json`
- 任意构建：运行 `cargo test --features openapi --test generate_openapi` 导出静态规范到 `frontend-panel/generated/openapi.json`

## 继续阅读

- [认证](/api/auth)
- [文件](/api/files)
- [团队与团队空间](/api/teams)
- [搜索](/api/search)
- [后台任务](/api/tasks)
- [WOPI](/api/wopi)
- [分享](/api/shares)
- [公共接口](/api/public)
- [管理](/api/admin)
