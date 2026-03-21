# 文件编辑

这一页只描述当前仓库已经落地的编辑能力，不写“理想中的协作编辑”，只写现在代码真的支持什么。

## 当前可用的三种入口

| 入口 | 适用场景 | 真实能力 |
| --- | --- | --- |
| 浏览器内文本编辑 | 快速改 `.txt`、`.md`、`.json`、`.xml` 一类文本文件 | 前端先读取内容与 `ETag`，进入编辑时加锁，保存时走 `PUT /api/v1/files/{id}/content` |
| REST 覆盖写入 | 自己做编辑器、脚本或集成外部服务 | 支持 `If-Match` 乐观锁，支持显式文件锁，自动生成历史版本 |
| WebDAV 编辑 | Finder、cadaver、桌面同步盘、Office 类客户端 | 支持 `LOCK` / `UNLOCK` / `PUT`，覆盖写入自动进版本历史，附带最小 DeltaV |

有两点先说清楚：

- 普通上传接口 `POST /api/v1/files/upload`、分片上传和预签名上传的职责是“创建文件”，不是“编辑现有文件”。
- 当前没有多人实时协作、冲突自动合并、Office 在线渲染这类能力。

## 浏览器内编辑

当前前端的入口在文件列表：

- 点击文件名打开预览弹窗
- 右侧时钟按钮打开版本历史
- 锁图标可以手动锁定 / 解锁

### 哪些文件能直接编辑

当前前端只会把下面这些 MIME 类型当成“可编辑文本”：

- `text/*`
- `application/json`
- `application/xml`

也就是说，是否出现 `Edit` 按钮取决于后端识别出的 `mime_type`。后端的 MIME 主要来自文件名推断，所以同样是文本文件，如果扩展名不典型，前端可能只会把它当普通文件而不是文本编辑器。

### 前端真实流程

浏览器内文本编辑现在走的是这条链路：

```text
打开预览
-> GET /api/v1/files/{id}/download
-> 读取文本内容 + ETag
-> 点击 Edit
-> POST /api/v1/files/{id}/lock { "locked": true }
-> 本地 textarea 编辑
-> PUT /api/v1/files/{id}/content + If-Match
-> 成功后刷新内容与 ETag
-> POST /api/v1/files/{id}/lock { "locked": false }
```

当前前端行为：

- 保存前会携带上一次读取到的 `ETag`
- 如果服务端返回 `412 Precondition Failed`，前端会提示文件已被其他人修改
- `Cancel` 会放弃本地改动并显式解锁
- 保存成功后会重新拉取文件，拿到新的 `ETag`

当前限制：

- 只支持纯文本编辑，实际控件就是一个 `textarea`
- 没有 diff、语法高亮、多人协作或自动合并
- 当前前端只在 `Save` 或 `Cancel` 时显式解锁；如果编辑中直接关闭预览弹窗，锁不会自动释放，需要在文件列表手动解锁，或者由管理员在 `/admin/locks` 强制解锁

## REST 编辑接口

如果你要自己接入编辑器、脚本或自动化流程，核心接口就三个：

| 方法 | 路径 | 用途 |
| --- | --- | --- |
| `GET` | `/api/v1/files/{id}/download` | 下载当前内容，并读取响应头 `ETag` |
| `PUT` | `/api/v1/files/{id}/content` | 覆盖文件内容 |
| `POST` | `/api/v1/files/{id}/lock` | 显式锁定 / 解锁文件 |

相关 API 的完整列表见 [文件 API](../api/files.md)。

### 推荐流程

```text
1. POST /api/v1/files/{id}/lock     -> 锁定文件（可选，但推荐）
2. GET  /api/v1/files/{id}/download -> 读取内容和 ETag
3. 本地编辑
4. PUT  /api/v1/files/{id}/content  -> 带 If-Match 保存
5. POST /api/v1/files/{id}/lock     -> 解锁
```

### 读取当前内容

```http
GET /api/v1/files/123/download
Cookie: aster_access=...
```

关键响应头：

- `ETag: "<sha256>"`，当前实现里的 `ETag` 就是 Blob 的 SHA-256
- `Content-Type`
- `Content-Length`

### 覆盖内容

```http
PUT /api/v1/files/123/content
Cookie: aster_access=...
Content-Type: application/octet-stream
If-Match: "current-sha256"

new file content
```

行为说明：

- `If-Match` 可选
- 传了 `If-Match` 就会做乐观锁校验
- 不传 `If-Match` 也能保存，但等于强制覆盖，不做并发冲突检测
- 每次成功覆盖都会自动生成一条历史版本
- 响应头会返回新的 `ETag`

常见状态码：

| 状态码 | 含义 |
| --- | --- |
| `200` | 保存成功 |
| `401` | 未登录或认证失效 |
| `403` | 文件不属于当前用户 |
| `404` | 文件不存在，或文件已经在回收站 |
| `412` | `If-Match` 不匹配，说明保存前文件已经变了 |
| `423` | 文件被其他用户锁住 |
| `507` | 超出用户存储配额 |

### 简化文件锁

```http
POST /api/v1/files/123/lock
Content-Type: application/json

{ "locked": true }
```

```http
POST /api/v1/files/123/lock
Content-Type: application/json

{ "locked": false }
```

这套 REST 锁和 WebDAV 锁共用同一张 `resource_locks` 表，但语义并不完全相同：

| 项目 | REST 锁 |
| --- | --- |
| 创建方式 | `POST /api/v1/files/{id}/lock` |
| 锁所有者 | 当前登录用户 |
| 超时 | 当前实现没有超时，除非主动解锁或管理员介入 |
| 谁可以写 `PUT /content` | 文件所有者；如果锁也是自己加的，这把锁不会拦住自己 |
| 谁可以解锁 | 锁持有者或文件所有者 |
| 管理员介入 | `/api/v1/admin/locks` 和前端 `/admin/locks` 可强制解锁 |

补充一点：文件一旦进入锁定状态，普通 REST 删除、重命名、移动都会被拒绝；只有“覆盖内容”这条路径会允许文件所有者在自己持有锁时继续保存。

## WebDAV 编辑

如果你希望让桌面客户端直接挂载目录，当前 WebDAV 已经可以承担真实编辑流量。

### 已支持的方法

常见 WebDAV 方法：

- `PROPFIND`
- `MKCOL`
- `PUT`
- `GET`
- `DELETE`
- `COPY`
- `MOVE`
- `LOCK`
- `UNLOCK`
- `OPTIONS`

额外补上的 DeltaV 最小子集：

- `REPORT` 的 `DAV:version-tree`
- `VERSION-CONTROL`
- `OPTIONS` 响应里的 `DAV: version-control`

更完整的协议说明见 [WebDAV API 与协议能力](../api/webdav.md)。

### WebDAV 写入链路

WebDAV 覆盖文件时，底层最终也会走和 REST 共用的 `store_from_temp()`：

```text
WebDAV PUT
-> 临时文件
-> SHA-256 / 去重 / 配额检查
-> 覆盖当前 file.blob_id
-> 把旧 Blob 写入 file_versions
-> 超限时清理最老版本
```

这意味着：

- WebDAV `PUT` 覆盖已有文件时会自动进入版本历史
- 同一份内容仍然会走 Blob 去重
- 配额和文件大小限制依然生效

### WebDAV 锁和 REST 锁的差异

| 项目 | WebDAV 锁 |
| --- | --- |
| 创建方式 | `LOCK` |
| 身份标识 | 由 lock token 驱动，不走 REST 的 `owner_id` 语义 |
| 超时 | 由客户端请求决定；有超时的锁过期后会在后续检查或后台清理时被移除 |
| 写入校验 | `dav-server` 通过提交的 lock token 校验 |
| 解锁方式 | `UNLOCK` + `Lock-Token` |
| 管理员介入 | `/api/v1/admin/locks` 或前端 `/admin/locks` |

要点：

- REST 的 `is_locked` 布尔值本身不是 WebDAV 写入授权依据
- WebDAV 侧真正决定能不能写的是 lock token 校验
- 所以“文件显示为已锁定”和“当前客户端能否提交写入”不是一个概念

### DeltaV 现在能做什么

当前实现已经能让支持 DeltaV 的客户端看到版本树：

- 当前版本在 `REPORT version-tree` 里显示为 `current`
- 历史版本显示为 `V1`、`V2` 这类编号
- 版本信息来自同一张 `file_versions` 表

当前限制：

- 只支持查看版本树，不是完整 DeltaV 服务器
- `REPORT version-tree` 只支持文件，不支持文件夹

## 版本历史

只要发生“覆盖已有文件”的写入，就会产生历史版本。当前最常见的来源有两个：

- `PUT /api/v1/files/{id}/content`
- WebDAV `PUT` 覆盖已有文件

普通新建上传不会生成历史版本。

### 当前可用操作

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| `GET` | `/api/v1/files/{id}/versions` | 按版本号倒序列出历史版本 |
| `POST` | `/api/v1/files/{id}/versions/{version_id}/restore` | 恢复某个版本 |
| `DELETE` | `/api/v1/files/{id}/versions/{version_id}` | 删除某个版本 |

前端也已经接了版本历史弹窗，入口就是文件列表里的时钟按钮。

### 保留策略

版本上限由运行时配置 `max_versions_per_file` 控制：

- 默认值是 `10`
- 每次新增版本后，超出上限会自动清理最老版本
- 管理员可在 `/admin/settings` 在线修改

### 恢复版本时的当前语义

恢复历史版本不是“再额外创建一条回滚快照”，当前实现是：

1. 当前文件直接切回目标版本对应的 Blob
2. 被恢复的那条历史记录从 `file_versions` 删除
3. 不额外生成一条“恢复前版本”

因此，如果你在前端点了“恢复版本”，恢复成功后看到那条版本记录消失，这是当前实现的预期行为，不是数据丢了。

## 适合怎么用

按现在这套实现，比较务实的建议是：

- 浏览器里快速改文本文件：直接用前端预览里的 `Edit`
- 自己做集成：走 `GET /download` + `PUT /content` + `If-Match`
- 桌面客户端或外部编辑器：挂 WebDAV
- 锁卡住了：先让用户手动解锁，不行就去 `/admin/locks`

如果你接下来要补更细的接口说明，可以继续看：

- [文件 API](../api/files.md)
- [WebDAV API 与协议能力](../api/webdav.md)
- [管理面板](./admin-console.md)
