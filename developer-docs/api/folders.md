# 文件夹 API

以下路径都相对于 `/api/v1`，且都需要认证。

## 一览

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| `GET` | `/folders` | 列出根目录内容 |
| `POST` | `/folders` | 创建文件夹 |
| `GET` | `/folders/{id}` | 列出指定文件夹内容 |
| `GET` | `/folders/{id}/info` | 读取单个文件夹完整信息 |
| `GET` | `/folders/{id}/ancestors` | 读取面包屑祖先链 |
| `PATCH` | `/folders/{id}` | 重命名、移动、设置策略 |
| `DELETE` | `/folders/{id}` | 软删除文件夹 |
| `POST` | `/folders/{id}/lock` | 简化锁定 / 解锁 |
| `POST` | `/folders/{id}/copy` | 递归复制文件夹 |

## 目录读取

- `GET /folders`：读取根目录内容
- `GET /folders/{id}`：读取指定目录内容
- `GET /folders/{id}/info`：读取单个目录的完整模型
- `GET /folders/{id}/ancestors`：返回当前目录的祖先链，供前端面包屑使用

当前 REST 目录列表按数据库中的真实条目返回，不会在 API 层额外过滤 `._*`、`~$*`、`.DS_Store` 这类名字。

`GET /folders` 和 `GET /folders/{id}` 还支持当前实现的分页 / 排序参数：

- `folder_limit` / `folder_offset`
- `file_limit`
- `sort_by` / `sort_order`
- `file_after_value` / `file_after_id`

也就是说，文件夹列表是 offset 分页，文件列表是 cursor 分页。

补两条现在前端已经依赖的细节：

- `folder_limit = 0` 或 `file_limit = 0` 可以显式跳过其中一类查询
- 返回体里会带 `next_file_cursor`；只要它非空，就可以继续翻下一页文件
- 列表接口里的 `folders` / `files` 条目会刻意裁掉一部分不必要字段；如果你需要某个目录自己的完整信息，应改用 `/folders/{id}/info`

## 创建与修改

创建请求很简单：

```json
{
  "name": "Documents",
  "parent_id": null
}
```

`parent_id = null` 表示在根目录创建。

`PATCH /folders/{id}` 当前支持三件事：

- 重命名
- 移动到其他父目录
- 设置目录级存储策略覆盖
- `parent_id = null` 时移回根目录
- `policy_id = null` 时清除目录级策略覆盖

同时会做这些校验：

- 不能移动到自己下面或子孙目录下面
- 目标位置同名会报错
- 被锁定文件夹不能修改

## 删除、锁和复制

- `DELETE /folders/{id}`：软删除，会递归进入回收站
- `POST /folders/{id}/lock`：`locked = true` 加锁，`locked = false` 解锁
- `POST /folders/{id}/copy`：递归复制目录树，成功返回 `201`

复制时底层文件内容不会物理复制，只增加 Blob 引用计数；目标位置同名会自动生成副本名。`parent_id = null` 表示复制到根目录，新目录树会保留源目录上的 `policy_id`。
