# 搜索 API

以下路径都相对于 `/api/v1`，且都需要认证。

## 接口列表

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| `GET` | `/search` | 搜索当前用户个人空间的文件和文件夹 |
| `GET` | `/teams/{team_id}/search` | 搜索指定团队工作空间内的文件和文件夹 |

## 查询参数

常用参数：

- `q`：名称模糊匹配，大小写不敏感
- `type`：`file`、`folder` 或 `all`，默认 `all`
- `mime_type`：按精确 MIME 类型过滤文件
- `category`：按文件分类过滤，支持 `image`、`video`、`audio`、`document`、`spreadsheet`、`presentation`、`archive`、`code`、`other`
- `extensions`：按扩展名过滤文件，推荐逗号分隔字符串，例如 `pdf,docx,tar.gz`
- `min_size` / `max_size`：按文件大小过滤
- `created_after` / `created_before`：RFC3339 时间字符串
- `folder_id`：把搜索范围限制到某个目录
- `limit`：每种资源类型的返回上限，默认 `50`，最大 `100`
- `offset`：偏移量

当前实现会校验时间参数：

- `created_after` / `created_before` 必须是合法 RFC3339 时间字符串，否则返回 `400`
- 如果两者都传，要求 `created_after <= created_before`，否则同样返回 `400`

文件类型过滤也会校验：

- `category` 必须是上面列出的固定值
- `extensions` 不能为空，不能包含空段，不能包含路径分隔或非法字符
- 扩展名会规范化为小写并去掉前导点；`tar.gz` 这类复合扩展会匹配 `compound_extension`
- `category` / `extensions` 是文件专用过滤；如果 `type=folder` 同时传它们，会返回 `400`

## 返回结构

响应会同时返回两组结果：

- `files`
- `folders`
- `total_files`
- `total_folders`

其中 `files` / `folders` 复用列表接口里的条目结构，因此会带当前的 `is_locked`、`is_shared` 等状态。
文件条目还会带 `extension`、`compound_extension` 和 `file_category`，这些字段来自 `files` 表上的持久化分类结果。

## 当前语义

- `/search` 只搜索当前用户个人空间资源
- `/teams/{team_id}/search` 会先校验当前用户的团队访问权限，再搜索该团队工作空间资源
- 已进回收站的资源不会出现在结果里
- `type=folder` 时不会返回文件；`type=file` 时不会返回文件夹
- `folder_id` 对文件按 `folder_id` 过滤，对文件夹按 `parent_id` 过滤
- `category` 优先使用扩展名分类结果；扩展名无法判断时才用 MIME 类型兜底
- 复合扩展只对明确支持的后缀保存到 `compound_extension`，例如 `tar.gz`
