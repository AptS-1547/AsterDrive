# 回收站 API

以下路径都相对于 `/api/v1`，且都需要认证。

## 一览

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| `GET` | `/trash` | 列出回收站内容 |
| `POST` | `/trash/{entity_type}/{id}/restore` | 恢复单个文件或文件夹 |
| `DELETE` | `/trash/{entity_type}/{id}` | 彻底删除单个文件或文件夹 |
| `DELETE` | `/trash` | 清空当前用户回收站 |

其中 `entity_type` 只能是 `file` 或 `folder`。

`GET /trash` 当前支持这些分页参数：

- `folder_limit` / `folder_offset`
- `file_limit`
- `file_after_deleted_at` / `file_after_id`

返回体会带：

- `folders`
- `files`
- `folders_total`
- `files_total`
- `next_file_cursor`

也就是说，回收站和普通目录列表一样，文件夹用 offset 分页，文件用 cursor 分页。

## 恢复与清理规则

- `GET /trash` 会返回当前用户回收站里的 `folders` 和 `files`
- 恢复时，如果原父目录已经不存在，资源会回到根目录
- 如果恢复的是文件夹，会递归恢复其已删除子项
- `DELETE /trash/{entity_type}/{id}` 是永久删除
- `DELETE /trash` 会清空整个回收站，并返回 `{ "purged": <count> }`

永久删除时，文件会处理 Blob 引用计数、缩略图、版本与配额回收；文件夹则会递归清掉整棵目录树。

还需要注意一条实现语义：

- 永久删除会先提交业务记录删除和 Blob `ref_count` 递减
- 存储对象与缩略图的物理清理在事务后执行
- 只有对象确认已经删除后，才会移除 `file_blobs` 元数据
- 如果存储层临时删除失败，Blob 元数据会保留，通常表现为 `ref_count = 0`，后续由后台 maintenance 任务重试清理

## 自动清理

除了手动清空或永久删除，系统还会根据 `trash_retention_days` 每小时清理一次过期条目。
