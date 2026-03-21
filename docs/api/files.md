# 文件 API

以下路径都相对于 `/api/v1`，且都需要认证。

## 接口列表

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| `POST` | `/files/upload` | 普通 multipart 直传 |
| `POST` | `/files/upload/init` | 协商上传模式 |
| `PUT` | `/files/upload/{upload_id}/{chunk_number}` | 上传单个分片 |
| `POST` | `/files/upload/{upload_id}/complete` | 组装分片或确认预签名上传 |
| `GET` | `/files/upload/{upload_id}` | 查询上传进度 |
| `DELETE` | `/files/upload/{upload_id}` | 取消上传 |
| `GET` | `/files/{id}` | 获取文件元信息 |
| `GET` | `/files/{id}/download` | 下载文件内容 |
| `GET` | `/files/{id}/thumbnail` | 获取缩略图 |
| `PATCH` | `/files/{id}` | 重命名或移动文件 |
| `DELETE` | `/files/{id}` | 软删除到回收站 |
| `POST` | `/files/{id}/lock` | 简化锁定 / 解锁 |
| `POST` | `/files/{id}/copy` | 复制文件 |
| `GET` | `/files/{id}/versions` | 列出历史版本 |
| `POST` | `/files/{id}/versions/{version_id}/restore` | 恢复某个版本 |
| `DELETE` | `/files/{id}/versions/{version_id}` | 删除某个版本 |

## 上传模式协商

### `POST /files/upload/init`

请求体：

```json
{
  "filename": "archive.zip",
  "total_size": 5368709120,
  "folder_id": 12
}
```

服务端会根据目标存储策略返回三种模式之一：

### `mode = "direct"`

说明：

- 客户端应改走 `POST /files/upload`
- 响应里不会有 `upload_id`

### `mode = "chunked"`

返回值会带：

- `upload_id`
- `chunk_size`
- `total_chunks`

### `mode = "presigned"`

只在 S3 兼容策略场景出现。返回值会带：

- `upload_id`
- `presigned_url`

然后客户端需要：

1. 把文件直接 `PUT` 到 `presigned_url`
2. 再调用 `POST /files/upload/{upload_id}/complete`

## 普通直传

### `POST /files/upload`

查询参数：

| 参数 | 类型 | 说明 |
| --- | --- | --- |
| `folder_id` | `i64?` | 目标文件夹；为空时表示根目录 |

请求体是 `multipart/form-data`。

当前实现行为：

- 空文件会报错
- 同目录同名文件会报错
- 该接口不会覆盖已有文件

## 分片上传

### `PUT /files/upload/{upload_id}/{chunk_number}`

- `chunk_number` 从 `0` 开始
- 请求体使用 `application/octet-stream`
- 对已经存在的分片是幂等的

### `GET /files/upload/{upload_id}`

返回：

- 上传状态
- 已接收分片数量
- 服务器磁盘上已存在的分片编号 `chunks_on_disk`

这正是前端断点续传依赖的接口。

### `POST /files/upload/{upload_id}/complete`

对于 `chunked` 模式，服务端会：

1. 组装临时文件
2. 计算 SHA-256
3. 校验大小和配额
4. 进行 Blob 去重
5. 创建最终文件记录

对于 `presigned` 模式，服务端会：

1. 从 S3 临时 key 读取对象
2. 计算 SHA-256
3. 进行去重与最终落盘
4. 创建最终文件记录

## 普通文件操作

### `GET /files/{id}`

读取文件元信息。

已经进入回收站的文件会按“找不到”处理。

### `GET /files/{id}/download`

流式下载文件，响应会带：

- `Content-Type`
- `Content-Length`
- `Content-Disposition: attachment`

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

当前限制：

- `folder_id` 传 `null` 与“不传”在后端等价，因此现有接口无法把文件移动回根目录
- 目标位置同名冲突会报错
- 被锁定文件不能修改

### `DELETE /files/{id}`

这是软删除，文件会进入回收站，而不是立刻删物理内容。

## 缩略图

### `GET /files/{id}/thumbnail`

当前实现会：

- 仅对支持的图片类型生成缩略图
- 统一返回 WebP
- 以 Blob 为粒度复用与缓存

## 锁与复制

### `POST /files/{id}/lock`

请求体：

```json
{ "locked": true }
```

这是一层简化的 REST 锁接口。底层真实锁记录仍保存在 `resource_locks`。

### `POST /files/{id}/copy`

请求体：

```json
{ "folder_id": 8 }
```

当前行为：

- 底层 Blob 不会物理复制，只增加引用计数
- 若目标目录已有同名文件，会自动生成 `name (1).ext` 这类副本名
- `folder_id = null` 当前表示“沿用原目录”，不能把文件复制到根目录

## 版本历史

历史版本主要来自覆盖写入流程，例如 WebDAV 覆盖已有文件。

### `GET /files/{id}/versions`

返回该文件当前保存的历史版本数组，按版本号倒序。

### `POST /files/{id}/versions/{version_id}/restore`

把当前文件切回指定历史版本。

当前实现细节：

- 恢复后不会再额外创建一个“回滚前版本”
- 被恢复的版本记录会被删除，因为它已经成为当前版本

### `DELETE /files/{id}/versions/{version_id}`

删除指定历史版本；若其底层 Blob 不再被任何文件或版本引用，会连带清理物理内容。
