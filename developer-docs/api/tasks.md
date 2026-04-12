# 后台任务 API

以下路径都相对于 `/api/v1`，且都需要认证。

这组接口只负责“列出现有任务、查看详情、重试失败任务”，不负责创建任务。

## 个人空间

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| `GET` | `/tasks` | 分页列出当前用户个人空间任务 |
| `GET` | `/tasks/{id}` | 读取单个个人空间任务 |
| `POST` | `/tasks/{id}/retry` | 重试失败的个人空间任务 |

## 团队空间

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| `GET` | `/teams/{team_id}/tasks` | 分页列出指定团队任务 |
| `GET` | `/teams/{team_id}/tasks/{id}` | 读取单个团队任务 |
| `POST` | `/teams/{team_id}/tasks/{id}/retry` | 重试失败的团队任务 |

## 分页

列表接口都使用 offset 分页参数：

- `limit`
- `offset`

当前实现细节：

- 默认 `limit = 20`
- 实际上限受运行时配置 `task_list_max_limit` 控制，默认 `100`
- 个人接口只会返回 `creator_user_id = 当前用户` 且 `team_id = null` 的任务
- 团队接口只会返回 `team_id = {team_id}` 的任务

## `TaskInfo`

列表和详情都会返回 `TaskInfo`，当前主要字段包括：

- `id`
- `kind`
- `status`
- `display_name`
- `progress_current`
- `progress_total`
- `progress_percent`
- `status_text`
- `attempt_count`
- `max_attempts`
- `last_error`
- `payload_json`
- `result_json`
- `can_retry`
- `started_at`
- `finished_at`
- `expires_at`
- `created_at`
- `updated_at`

其中：

- `can_retry = true` 目前只在 `status = failed` 时出现
- `progress_total <= 0` 时，成功任务的 `progress_percent` 会直接视为 `100`
- `expires_at` 表示这条任务记录以及相关临时产物的清理时间；默认保留期来自运行时配置 `task_retention_hours`，默认 `24` 小时

## `POST /tasks/{id}/retry`

这条接口和团队对应版本都只接受“失败态”任务：

- 只有 `status = failed` 才能重试
- 成功重试后，任务会被重置回待执行状态
- 当前实现会先清掉该任务旧的临时目录，再做重置

如果任务当前不是 `failed`，会返回 `400`。

## 当前实现现状

有件事得直说，不然你看接口名会被带沟里：

- 当前 `/batch/archive-download` 和团队对应接口走的是“短期 stream ticket + 直接 ZIP 流下载”
- 它们不会创建这里的 `background_task` 记录
- `BackgroundTaskKind` 虽然还保留了 `archive_download`、`archive_extract`、`archive_compress` 这些枚举值，但现阶段公开 API 主要把这组接口当成任务面板 / 诊断入口来用

也就是说，如果你只用当前公开 REST 能力，任务列表很可能长期为空，这是实现现状，不是你调错了。
