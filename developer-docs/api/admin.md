# 管理 API

以下路径都相对于 `/api/v1`，且都需要管理员权限。

这页只保留管理端最值得记住的接口分组；更偏使用体验的内容见 [管理面板](../../docs/guide/admin-console.md)。

当前大多数“列表类”管理员接口都已经是 offset 分页：

- `/admin/policies`
- `/admin/policy-groups`
- `/admin/users`
- `/admin/teams`
- `/admin/teams/{team_id}/members`
- `/admin/shares`
- `/admin/config`
- `/admin/locks`
- `/admin/audit-logs`

## 存储策略

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| `GET` | `/admin/policies` | 列出全部存储策略 |
| `POST` | `/admin/policies` | 创建存储策略 |
| `GET` | `/admin/policies/{id}` | 读取策略详情 |
| `PATCH` | `/admin/policies/{id}` | 更新策略 |
| `DELETE` | `/admin/policies/{id}` | 删除策略 |
| `POST` | `/admin/policies/{id}/test` | 测试已保存策略 |
| `POST` | `/admin/policies/test` | 用临时参数测试连接 |

### 创建策略示例

```json
{
  "name": "archive-s3",
  "driver_type": "s3",
  "endpoint": "https://s3.example.com",
  "bucket": "archive",
  "access_key": "AKIA...",
  "secret_key": "...",
  "base_path": "asterdrive/",
  "max_file_size": 10737418240,
  "chunk_size": 10485760,
  "is_default": false
}
```

当前实现注意点：

- 创建和更新都会采用请求里的 `chunk_size`
- `options` 当前主要承载 S3 上传 / 下载策略，例如 `{"s3_upload_strategy":"presigned","s3_download_strategy":"presigned"}`
- 旧配置 `{"presigned_upload":true}` 仍兼容
- REST 仍然不能管理 `allowed_types`
- 当前 `PATCH` 不能修改 `driver_type`

## 策略组

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| `GET` | `/admin/policy-groups` | 列出全部存储策略组 |
| `POST` | `/admin/policy-groups` | 创建策略组 |
| `GET` | `/admin/policy-groups/{id}` | 读取策略组详情 |
| `PATCH` | `/admin/policy-groups/{id}` | 更新策略组 |
| `DELETE` | `/admin/policy-groups/{id}` | 删除策略组 |
| `POST` | `/admin/policy-groups/{id}/migrate-users` | 把用户批量迁移到另一个策略组 |

创建示例：

```json
{
  "name": "default-hot-cold",
  "description": "小文件走本地，大文件走对象存储",
  "is_enabled": true,
  "is_default": false,
  "items": [
    {
      "policy_id": 1,
      "priority": 10,
      "min_file_size": 0,
      "max_file_size": 10485760
    },
    {
      "policy_id": 2,
      "priority": 20,
      "min_file_size": 10485761,
      "max_file_size": 0
    }
  ]
}
```

当前实现注意点：

- 策略组至少要包含一个策略项
- 同一组里 `policy_id` 和 `priority` 都不能重复
- `is_default = true` 的组必须保持启用
- 已被用户或团队绑定的策略组不能直接删掉；被绑定时也不能随便禁用
- `/migrate-users` 只迁移 `users.policy_group_id`，不会替你改团队绑定

迁移请求体很简单：

```json
{
  "target_group_id": 9
}
```

## 总览面板

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| `GET` | `/admin/overview` | 读取管理后台总览 PoC 所需的聚合数据 |

当前返回内容包含：

- 总用户数、启用中用户、禁用用户
- 总文件数、总文件字节数、总 blob 数、总 blob 字节数、总分享数
- 今日审计事件数、今日新增用户数、今日上传数、今日新分享数
- 最近 N 天日报（默认 7）
- 最近一批审计事件

支持这些查询参数：

- `days`：日报天数，默认 `7`，最大 `90`
- `timezone`：IANA 时区名，例如 `UTC`、`Asia/Shanghai`
- `event_limit`：最近活动返回数量，默认 `8`，最大 `50`

这个接口当前的日报和“最近活动”都基于审计日志统计，因此如果审计日志关闭，对应数据会偏少或为 0。总量类指标（用户 / 文件 / blob / 分享 / 字节数）不依赖审计日志。

## 用户

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| `GET` | `/admin/users` | 列出用户 |
| `POST` | `/admin/users` | 管理员直接创建用户 |
| `GET` | `/admin/users/{id}` | 获取用户详情 |
| `PATCH` | `/admin/users/{id}` | 更新角色、状态、总配额和策略组绑定 |
| `PUT` | `/admin/users/{id}/password` | 管理员直接重置用户密码 |
| `POST` | `/admin/users/{id}/sessions/revoke` | 吊销该用户所有现有会话 |
| `DELETE` | `/admin/users/{id}` | 永久删除用户及其全部数据 |
| `GET` | `/admin/users/{id}/avatar/{size}` | 读取指定用户已上传头像 |

`GET /admin/users` 现在支持：

- `limit`
- `offset`
- `keyword`
- `role`
- `status`

`POST /admin/users` 的请求体与普通注册类似：

```json
{
  "username": "alice",
  "email": "alice@example.com",
  "password": "password"
}
```

### 更新用户示例

```json
{
  "role": "user",
  "status": "active",
  "storage_quota": 107374182400,
  "policy_group_id": 3
}
```

注意：

- `storage_quota = 0` 表示不限
- `policy_group_id` 不传表示保持不变；当前实现明确拒绝 `null`
- 当前实现禁止禁用初始管理员 `id = 1`
- 当前实现也禁止把初始管理员 `id = 1` 降级为非管理员
- `PUT /admin/users/{id}/password` 使用 `{ "password": "new-secret" }`
- `POST /admin/users/{id}/sessions/revoke` 会让这个用户现有 JWT / Cookie 会话全部失效
- `GET /admin/users/{id}/avatar/{size}` 只会返回“已上传头像”的二进制资源；Gravatar 应看用户详情里的 `profile.avatar.url_*`
- `DELETE /admin/users/{id}` 是物理删除，不是软删除；当前也不允许删除管理员用户

## 团队

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| `GET` | `/admin/teams` | 分页查看全部团队 |
| `POST` | `/admin/teams` | 创建团队并指定初始团队管理员 |
| `GET` | `/admin/teams/{id}` | 读取团队详情 |
| `PATCH` | `/admin/teams/{id}` | 更新团队名称、描述、策略组 |
| `DELETE` | `/admin/teams/{id}` | 归档团队 |
| `POST` | `/admin/teams/{id}/restore` | 恢复已归档团队 |
| `GET` | `/admin/teams/{id}/audit-logs` | 查看团队审计记录 |
| `GET` | `/admin/teams/{id}/members` | 分页查看团队成员 |
| `POST` | `/admin/teams/{id}/members` | 添加团队成员 |
| `PATCH` | `/admin/teams/{id}/members/{member_user_id}` | 调整成员角色 |
| `DELETE` | `/admin/teams/{id}/members/{member_user_id}` | 移除团队成员 |

`GET /admin/teams` 支持：

- `limit`
- `offset`
- `keyword`
- `archived`

创建示例：

```json
{
  "name": "Operations",
  "description": "跨职能运营空间",
  "admin_identifier": "lead@example.com",
  "policy_group_id": 4
}
```

当前实现注意点：

- `admin_user_id` 和 `admin_identifier` 二选一，不能同时传，也不能都不传
- 创建团队时如果没传 `policy_group_id`，会退回系统默认策略组；如果系统没有默认组，创建会失败
- 团队更新接口也支持 `policy_group_id`，但和用户一样，当前实现拒绝显式传 `null`
- 团队成员列表支持 `keyword`、`role`、`status`、`limit`、`offset`
- 团队审计接口支持 `user_id`、`action`、`after`、`before`、`limit`、`offset`

## 系统运行时配置

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| `GET` | `/admin/config` | 列出全部运行时配置 |
| `GET` | `/admin/config/schema` | 读取系统配置 schema |
| `GET` | `/admin/config/template-variables` | 读取模板变量清单 |
| `GET` | `/admin/config/{key}` | 获取单个配置项 |
| `PUT` | `/admin/config/{key}` | 设置配置项 |
| `DELETE` | `/admin/config/{key}` | 删除配置项 |
| `POST` | `/admin/config/{key}/action` | 对特定配置目标执行动作 |

### 当前常用 key

- 具体定义以 `/admin/config/schema` 和 `src/config/definitions.rs` 为准；下面只列一批当前高频项，不是完整清单。邮件 SMTP、邮件模板、头像上传限制、注册/找回 TTL、分页上限等键也都在 schema 里
- `default_storage_quota`
- `webdav_enabled`
- `trash_retention_days`
- `team_archive_retention_days`
- `max_versions_per_file`
- `auth_allow_user_registration`
- `auth_register_activation_enabled`
- `audit_log_enabled`
- `audit_log_retention_days`
- `public_site_url`
- `auth_cookie_secure`
- `cors_enabled`
- `cors_allowed_origins`
- `cors_allow_credentials`
- `cors_max_age_secs`
- `gravatar_base_url`
- `mail_outbox_dispatch_interval_secs`
- `background_task_dispatch_interval_secs`
- `maintenance_cleanup_interval_secs`
- `blob_reconcile_interval_secs`
- `team_member_list_max_limit`
- `task_list_max_limit`
- `task_retention_hours`
- `avatar_max_upload_size_bytes`
- `thumbnail_max_source_bytes`
- `wopi_access_token_ttl_secs`
- `wopi_lock_ttl_secs`
- `wopi_discovery_cache_ttl_secs`
- `frontend_preview_apps_json`

### `public_site_url`

`public_site_url` 的数据库 key 保持单数，但值语义是“公开站点来源列表”：

```text
https://drive.example.com
https://panel.example.com
```

实现约束：

- 输入兼容旧的逗号分隔和新的换行分隔；规范化后保存为换行分隔
- 每一项必须是精确 HTTP(S) origin，只包含协议、host 和可选端口
- 不接受路径、查询、片段、通配符、`*` 或非 HTTP(S) scheme
- 第一项是无请求上下文时的默认回退来源
- 有请求上下文时，服务端会用当前请求的 scheme/Host 在列表里做精确匹配，命中后用对应来源生成 WebDAV、分享、预览和 WOPI URL
- 这个列表不是 CORS 白名单；浏览器跨域访问仍然由 `cors_allowed_origins` 控制
- 这个列表会参与 Cookie 认证写操作的 same-site CSRF 来源信任判断

`GET /admin/config` 当前也支持：

- `limit`
- `offset`

### 读取 schema

这个接口会返回：

- `value_type`
- `default_value`
- `category`
- `description`
- `requires_restart`
- `is_sensitive`

前端管理后台就是靠它动态渲染设置页，而不是写死每个配置项。

### 读取模板变量

`GET /admin/config/template-variables` 会返回按类别分组的模板变量清单，当前主要给管理后台在邮件、品牌文案等支持模板占位符的配置项旁边做提示，不必把变量表硬编码在前端里。

### 设置配置项示例

```json
{
  "value": "14"
}
```

### 执行配置动作

当前已经落地两类动作目标：

- `POST /admin/config/mail/action`
- `POST /admin/config/frontend_preview_apps_json/action`

邮件测试示例：

```json
{
  "action": "send_test_email",
  "target_email": "ops@example.com"
}
```

当前语义：

- `target_email` 不传时，默认发给当前管理员自己的邮箱
- `action = send_test_email` 会立即走运行时邮件发送链路
- 成功响应里会返回一段可直接展示给前端的 `message`
- 这条调用也会写管理员审计日志

预览应用 WOPI discovery 导入示例：

```json
{
  "action": "build_wopi_discovery_preview_config",
  "discovery_url": "https://office.example.com/hosting/discovery"
}
```

这条动作的当前语义：

- 目标 key 必须是 `frontend_preview_apps_json`
- `discovery_url` 必填，用来拉取并解析远端 WOPI discovery XML
- `value` 可选；传了就把它当“预览应用草稿 JSON”来导入并返回结果，不直接落库
- `value` 不传时，会基于当前线上配置或默认配置生成并直接写回 `frontend_preview_apps_json`
- 成功响应除了 `message`，还可能带一份新的 `value`，也就是归一化后的预览应用 JSON 草稿

## 分享审计

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| `GET` | `/admin/shares` | 查看全站分享 |
| `DELETE` | `/admin/shares/{id}` | 管理员删除任意分享 |

`GET /admin/shares` 支持：

- `limit`
- `offset`

## 审计日志

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| `GET` | `/admin/audit-logs` | 分页查询审计日志 |

当前实现支持这些查询参数：

- `user_id`
- `action`
- `entity_type`
- `after`
- `before`
- `limit`
- `offset`

其中 `after` 和 `before` 使用 RFC3339 时间字符串。

返回结果包含分页信息与日志项，日志项里会带时间、用户、动作、实体、名称、IP 等字段。

## 锁管理

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| `GET` | `/admin/locks` | 查看全部资源锁 |
| `DELETE` | `/admin/locks/{id}` | 强制解锁 |
| `DELETE` | `/admin/locks/expired` | 清理全部过期锁 |

`GET /admin/locks` 支持：

- `limit`
- `offset`

`DELETE /admin/locks/expired` 会返回：

```json
{
  "removed": 3
}
```
