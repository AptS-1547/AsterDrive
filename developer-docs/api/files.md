# 文件 API

以下路径都相对于 `/api/v1`，且都需要认证。

## 接口列表

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| `POST` | `/files/upload` | 普通 multipart 直传 |
| `POST` | `/files/new` | 创建空文件 |
| `POST` | `/files/upload/init` | 协商上传模式 |
| `PUT` | `/files/upload/{upload_id}/{chunk_number}` | 上传单个分片 |
| `POST` | `/files/upload/{upload_id}/presign-parts` | 为 S3 multipart 上传批量申请分片 URL |
| `POST` | `/files/upload/{upload_id}/complete` | 组装分片或确认预签名上传 |
| `GET` | `/files/upload/{upload_id}` | 查询上传进度 |
| `DELETE` | `/files/upload/{upload_id}` | 取消上传 |
| `GET` | `/files/{id}` | 获取文件元信息 |
| `GET` | `/files/{id}/direct-link` | 生成直接下载链接 token |
| `POST` | `/files/{id}/preview-link` | 生成短期预览链接 |
| `POST` | `/files/{id}/wopi/open` | 为指定 WOPI 预览器创建启动会话 |
| `GET` | `/files/{id}/download` | 下载文件内容 |
| `GET` | `/files/{id}/thumbnail` | 获取缩略图 |
| `PUT` | `/files/{id}/content` | 覆盖文件内容并写入版本历史 |
| `PATCH` | `/files/{id}` | 重命名或移动文件 |
| `DELETE` | `/files/{id}` | 软删除到回收站 |
| `POST` | `/files/{id}/lock` | 简化锁定 / 解锁 |
| `POST` | `/files/{id}/copy` | 复制文件 |
| `GET` | `/files/{id}/versions` | 列出历史版本 |
| `POST` | `/files/{id}/versions/{version_id}/restore` | 恢复某个版本 |
| `DELETE` | `/files/{id}/versions/{version_id}` | 删除某个版本 |

## 上传

上传的入口主要有两类：

- `POST /files/upload/init`：先协商模式
- `POST /files/upload`：直接走普通 multipart 上传

这两条入口都支持目录上传语义：

- `POST /files/upload` 可通过 query 传 `relative_path`
- `POST /files/upload/init` 可在请求体里传 `relative_path`
- 服务端会按相对路径自动创建缺失目录、复用已存在目录
- `relative_path` 中的空 segment 会被拒绝，例如 `docs//bad.txt`

协商接口会返回四种模式之一：

- `direct`：小文件直接上传
- `chunked`：大文件分片上传，可断点续传
- `presigned`：S3 单次预签名 `PUT`
- `presigned_multipart`：S3 multipart 直传，客户端需要再申请每个 part 的 URL

前端仍然只会看到这四种模式，不会额外出现一个 `relay_stream` 模式。S3 传输策略由存储策略
`options.s3_upload_strategy` 控制：

- `proxy_tempfile`：`init` 仍返回 `direct` / `chunked`，但服务端会先写本地临时文件或分片目录，再写入 S3
- `relay_stream`：`init` 仍返回 `direct` / `chunked`，但服务端直接把字节流中继到 S3，不落本地临时文件
- `presigned`：`init` 才会返回 `presigned` / `presigned_multipart`

旧配置 `{"presigned_upload":true}` 仍兼容，等价于 `{"s3_upload_strategy":"presigned"}`；`{"presigned_upload":false}` 或缺省时，默认等价于 `{"s3_upload_strategy":"proxy_tempfile"}`。使用预签名模式时，对象存储侧还必须配置好 CORS。

### 直传、分片和完成阶段

- `POST /files/upload`：普通 multipart 上传；空文件会报错，同目录同名文件不会覆盖。若命中的 S3 策略是 `relay_stream`，这里会直接把请求体中继到 S3
- `POST /files/new`：创建一个 0 字节空文件，适合“新建文本文件”这类前端动作
- `PUT /files/upload/{upload_id}/{chunk_number}`：上传单个分片，`chunk_number` 从 `0` 开始
- `POST /files/upload/{upload_id}/presign-parts`：只用于 `presigned_multipart`，请求体里传 `part_numbers`
- `GET /files/upload/{upload_id}`：查询上传进度，也是前端断点续传依赖的接口；返回会带 `status`、`received_count`、`chunks_on_disk`、`chunk_size`、`total_chunks`、`filename`
- `POST /files/upload/{upload_id}/complete`：完成 `chunked`、`presigned` 或 `presigned_multipart` 上传

完成阶段的服务端行为分两类：

- 本地路径：会校验大小和配额；若 local 策略开启了 `content_dedup`，还会计算 SHA-256 并做 Blob 去重，否则直接创建独立 Blob
- 所有 S3 路径（`proxy_tempfile` / `relay_stream` / `presigned` / `presigned_multipart`）：都会校验大小和配额，但不会做 Blob 去重；最终会使用 `s3-{upload_id}` 风格的占位 hash 和 `files/{upload_id}` 风格的对象路径为每次上传创建独立 Blob。其中 `proxy_tempfile` 会先写服务端临时文件，`relay_stream` / `presigned*` 则不会回读对象计算 SHA-256

`POST /files/new` 创建空文件时也遵循同样规则：只有 local 显式开启 `content_dedup` 才会复用 0 字节 Blob，S3 始终创建独立 Blob。

`relay_stream` 的 multipart 场景下，服务端会把每个 part 的 `part_number + etag` 持久化到数据库；`complete` 时直接使用这些服务端记录完成 S3 multipart，不依赖客户端再回传 `parts`。

对 `presigned_multipart` 来说，`complete` 请求体需要带对象存储返回的 `parts` 列表；其他模式可以不带请求体。

## 文件操作

- `GET /files/{id}`：读取文件元信息；已进回收站的文件会按“找不到”处理
- `GET /files/{id}/direct-link`：返回一个短 token；真正下载走根路径 `/d/{token}/{filename}`
- `POST /files/{id}/preview-link`：返回一个短期预览链接；真正读取内容走根路径 `/pv/{token}/{filename}`
- `POST /files/{id}/wopi/open`：为配置成 `provider = "wopi"` 的预览器创建一次 WOPI 启动会话
- `GET /files/{id}/download`：流式下载文件；支持 `If-None-Match`，命中时返回 `304`
- `GET /files/{id}/thumbnail`：读取缩略图（仅支持的图片类型）；若后台仍在生成，会先返回 `202` 和 `Retry-After`
- `PUT /files/{id}/content`：覆盖已有文件内容，是当前编辑现有文件的核心接口
- `PATCH /files/{id}`：改名或移动
- `DELETE /files/{id}`：软删除到回收站

其中 `PUT /files/{id}/content` 支持 `If-Match`，会检查锁状态，成功后自动生成历史版本，并返回新的 `ETag`。

### `PATCH /files/{id}`

请求体：

```json
{
  "name": "renamed.pdf",
  "folder_id": 5
}
```

当前实现支持：

- 改名
- 移动到其他文件夹
- `folder_id = null` 时移回根目录

当前限制：

- 目标位置同名冲突会报错
- 被锁定文件不能修改

### `DELETE /files/{id}`

这是软删除，文件会进入回收站，而不是立刻删物理内容。

### 缩略图

当前缩略图只对支持的图片类型生成，统一返回 WebP，并按 Blob 复用缓存。

### `GET /files/{id}/direct-link`

这个接口只返回：

```json
{
  "code": 0,
  "msg": "",
  "data": {
    "token": "..."
  }
}
```

拿到 token 后，实际下载地址是：

```text
/d/{token}/{filename}
```

其中：

- `filename` 必须和当前文件名匹配；URL 编码后的同名也可以
- 这个下载入口不走 `/api/v1`，返回原始文件流，不是 JSON
- 追加 `?download=1` 可以强制走附件下载；不传时按 inline 处理

### `POST /files/{id}/preview-link`

这个接口会返回：

```json
{
  "code": 0,
  "msg": "",
  "data": {
    "path": "/pv/...",
    "expires_at": "2026-04-09T12:00:00Z",
    "max_uses": 5
  }
}
```

要点：

- `path` 是实际预览入口；如果配置了 `public_site_url`，这里可能已经是完整绝对 URL
- 真实预览内容不走 `/api/v1`，而是走根路径 `/pv/{token}/{filename}`
- 当前预览链接默认 5 分钟过期，且最多使用 5 次
- 这个能力主要给内联预览器、Office 在线预览桥接和只读浏览场景使用，不等价于长期分享链接

### `POST /files/{id}/wopi/open`

请求体很简单：

```json
{
  "app_key": "custom.onlyoffice"
}
```

成功后返回 `WopiLaunchSession`：

```json
{
  "code": 0,
  "msg": "",
  "data": {
    "access_token": "...",
    "access_token_ttl": 1775995200000,
    "action_url": "https://office.example.com/...&WOPISrc=https%3A%2F%2Fdrive.example.com%2Fapi%2Fv1%2Fwopi%2Ffiles%2F1",
    "form_fields": {},
    "mode": "iframe"
  }
}
```

要点：

- `app_key` 必须指向 `/public/preview-apps` 里当前启用、且 `provider = "wopi"` 的预览器
- `action_url` 会带上实际回调入口 `WOPISrc`
- 当前实现里的 `access_token_ttl` 实际返回的是“过期时间的 Unix 毫秒时间戳”，不是“剩余多少秒”
- 生成 WOPI 启动会话时要求系统已配置 `public_site_url`
- 真实 WOPI 回调不走这条路径，而是走 `/api/v1/wopi/files/{id}` 及其 `/contents` 变体，详细见 [WOPI](/api/wopi)

## 锁与复制

### `POST /files/{id}/lock`

这是简化的 REST 锁接口：`locked = true` 表示加锁，`locked = false` 表示解锁。底层真实锁记录仍保存在 `resource_locks`。

### `POST /files/{id}/copy`

复制文件不会物理复制 Blob，只增加引用计数；目标目录同名时会自动生成副本名。`folder_id = null` 表示复制到根目录。

## 版本历史

历史版本主要来自覆盖写入，例如：

- `PUT /files/{id}/content`
- WebDAV `PUT` 覆盖已有文件

对应接口：

- `GET /files/{id}/versions`
- `POST /files/{id}/versions/{version_id}/restore`
- `DELETE /files/{id}/versions/{version_id}`

当前语义要记住一条：恢复版本不会额外生成一条“回滚前版本”，被恢复的版本记录会直接消失，因为它已经重新变成当前版本。
