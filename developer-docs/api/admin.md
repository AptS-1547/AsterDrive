# 管理 API

以下路径都相对于 `/api/v1`，且都需要管理员权限。

这页只保留管理端最值得记住的接口分组；更偏使用体验的内容见 [管理面板](/guide/admin-console)。

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

- 创建逻辑目前不会采用请求里的 `chunk_size`，而是先写固定 `5 MiB`
- 若要精确调整分片大小，需要创建后再 `PATCH`
- 当前 `PATCH` 不能修改 `driver_type`

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

## 用户与用户策略

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| `GET` | `/admin/users` | 列出用户 |
| `POST` | `/admin/users` | 管理员直接创建用户 |
| `GET` | `/admin/users/{id}` | 获取用户详情 |
| `PATCH` | `/admin/users/{id}` | 更新角色、状态、总配额 |
| `DELETE` | `/admin/users/{id}` | 永久删除用户及其全部数据 |
| `GET` | `/admin/users/{id}/avatar/{size}` | 读取指定用户头像 |
| `GET` | `/admin/users/{user_id}/policies` | 列出用户绑定的策略 |
| `POST` | `/admin/users/{user_id}/policies` | 给用户分配策略 |
| `PATCH` | `/admin/users/{user_id}/policies/{id}` | 更新用户策略项 |
| `DELETE` | `/admin/users/{user_id}/policies/{id}` | 删除用户策略项 |

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
  "storage_quota": 107374182400
}
```

注意：

- `storage_quota = 0` 表示不限
- 当前实现禁止禁用初始管理员 `id = 1`
- 当前实现也禁止把初始管理员 `id = 1` 降级为非管理员
- `DELETE /admin/users/{id}` 是物理删除，不是软删除；当前也不允许删除管理员用户

### 分配用户策略示例

```json
{
  "policy_id": 3,
  "is_default": true,
  "quota_bytes": 53687091200
}
```

`quota_bytes` 是“用户在该策略项上的额度”，与用户总配额不是同一个概念。

## 系统运行时配置

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| `GET` | `/admin/config` | 列出全部运行时配置 |
| `GET` | `/admin/config/schema` | 读取系统配置 schema |
| `GET` | `/admin/config/{key}` | 获取单个配置项 |
| `PUT` | `/admin/config/{key}` | 设置配置项 |
| `DELETE` | `/admin/config/{key}` | 删除配置项 |

### 当前常用 key

- `default_storage_quota`
- `webdav_enabled`
- `trash_retention_days`
- `max_versions_per_file`
- `audit_log_enabled`
- `audit_log_retention_days`

### 读取 schema

这个接口会返回：

- `value_type`
- `default_value`
- `category`
- `description`
- `requires_restart`
- `is_sensitive`

前端管理后台就是靠它动态渲染设置页，而不是写死每个配置项。

### 设置配置项示例

```json
{
  "value": "14"
}
```

## 分享审计

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| `GET` | `/admin/shares` | 查看全站分享 |
| `DELETE` | `/admin/shares/{id}` | 管理员删除任意分享 |

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

`DELETE /admin/locks/expired` 会返回：

```json
{
  "removed": 3
}
```
