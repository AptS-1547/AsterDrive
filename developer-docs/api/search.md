# 搜索 API

以下路径都相对于 `/api/v1`，且都需要认证。

## 接口列表

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| `GET` | `/search` | 搜索当前用户可见的文件和文件夹 |

## 查询参数

常用参数：

- `q`：名称模糊匹配，大小写不敏感
- `type`：`file`、`folder` 或 `all`，默认 `all`
- `mime_type`：按精确 MIME 类型过滤文件
- `min_size` / `max_size`：按文件大小过滤
- `created_after` / `created_before`：RFC3339 时间字符串
- `folder_id`：把搜索范围限制到某个目录
- `limit`：每种资源类型的返回上限，默认 `50`，最大 `100`
- `offset`：偏移量

当前实现里，非法的 `created_after` / `created_before` 不会报错，而是被忽略。

## 返回结构

响应会同时返回两组结果：

- `files`
- `folders`
- `total_files`
- `total_folders`

其中 `files` / `folders` 复用列表接口里的条目结构，因此会带当前的 `is_locked`、`is_shared` 等状态。

## 当前语义

- 只搜索当前用户自己的资源
- 已进回收站的资源不会出现在结果里
- `type=folder` 时不会返回文件；`type=file` 时不会返回文件夹
- `folder_id` 对文件按 `folder_id` 过滤，对文件夹按 `parent_id` 过滤
