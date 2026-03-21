# 核心流程

这一页把当前版本最常见的业务流程串起来，便于从“服务能跑”过渡到“产品能用”。

## 1. 管理员初始化

第一个注册用户会自动成为管理员。管理员通常需要完成：

- 检查系统默认存储策略
- 给用户分配默认策略和配额
- 设置运行时配置，例如默认新用户配额、回收站保留期、版本保留数量

对应页面与接口：

- 前端：`/admin/users`、`/admin/policies`、`/admin/settings`
- API：`/api/v1/admin/*`

## 2. 存储策略如何生效

文件落到哪个后端，取决于这条优先级链：

```text
文件夹 policy_id -> 用户默认策略 -> 系统默认策略
```

典型用法：

- 全局默认走本地磁盘
- 某些团队目录改走 S3
- 某个用户单独分配更大的策略配额

## 3. 上传协商

当前上传链路不是只有一种。

`POST /api/v1/files/upload/init` 会返回：

- `direct`：小文件走普通 multipart
- `chunked`：大文件走分片上传，可断点续传
- `presigned`：S3 策略可直接让客户端 PUT 到预签名 URL

其中 `presigned` 只有在同时满足这些条件时才会出现：

- 当前策略驱动是 `s3`
- 策略 `options` 里启用了 `{"presigned_upload": true}`
- 文件大小不超过单次 `PUT` 的 5 GiB

## 4. 覆盖写入、版本与锁

REST 普通上传不会覆盖同名文件；覆盖写入主要来自 WebDAV `PUT`。

覆盖时会发生：

1. 先检查文件是否被锁定
2. 旧 Blob 被写入历史版本表
3. 当前文件切换到新 Blob
4. 超过 `max_versions_per_file` 的最老版本会被自动清理

相关接口：

- 查看版本：`GET /api/v1/files/{id}/versions`
- 恢复版本：`POST /api/v1/files/{id}/versions/{version_id}/restore`
- 删除版本：`DELETE /api/v1/files/{id}/versions/{version_id}`
- REST 简化锁：`POST /api/v1/files/{id}/lock`、`POST /api/v1/folders/{id}/lock`
- 管理员锁管理：`GET /api/v1/admin/locks`

## 5. 分享

分享支持文件和文件夹两种资源。

典型流程：

1. `POST /api/v1/shares` 创建分享
2. 将返回的 token 组装为公开地址
3. 如果分享有密码，访问者先调用 `/api/v1/s/{token}/verify`
4. 再下载文件或读取文件夹根层内容

当前实现里，文件夹公开页只展示分享根目录的内容，不提供继续下钻到子目录的 REST 接口。

## 6. WebDAV

WebDAV 有两种认证方式：

- Basic Auth：使用专用 WebDAV 账号，可限制到某个根目录
- Bearer JWT：直接复用登录态，访问范围是整个用户空间

管理员或用户通常会这样使用：

1. 在前端 `/settings/webdav` 或 `POST /api/v1/webdav-accounts` 创建专用账号
2. 选择是否限制 `root_folder_id`
3. 用桌面客户端挂载默认地址 `http://<host>:3000/webdav`

WebDAV 侧还额外支持：

- 数据库锁
- 属性存储
- DeltaV 最小子集：`REPORT version-tree`、`VERSION-CONTROL`

## 7. 删除、恢复与清理

普通删除不会立刻删除物理文件，而是进入回收站。

- 删除文件：`DELETE /api/v1/files/{id}`
- 删除文件夹：`DELETE /api/v1/folders/{id}`
- 查看回收站：`GET /api/v1/trash`
- 恢复：`POST /api/v1/trash/{entity_type}/{id}/restore`
- 彻底删除：`DELETE /api/v1/trash/{entity_type}/{id}`
- 清空回收站：`DELETE /api/v1/trash`

后台任务还会按 `trash_retention_days` 每小时清理一次过期条目。

## 8. 前端操作与后端接口的对应关系

当前前端已经接好这些核心流程：

- 文件树浏览、上传、预览、分享、版本查看
- 回收站恢复与清空
- WebDAV 账号创建、停用、测试
- 管理员用户、策略、分享、锁、系统设置

也就是说，`docs` 里描述的大部分主流程，已经可以直接在当前前端界面里完成，而不是只存在于 API 层。
