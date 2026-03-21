# 管理 API

以下路径都相对于 `/api/v1`，且都需要管理员权限。

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

## 用户与用户策略

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| `GET` | `/admin/users` | 列出用户 |
| `GET` | `/admin/users/{id}` | 获取用户详情 |
| `PATCH` | `/admin/users/{id}` | 更新角色、状态、总配额 |
| `GET` | `/admin/users/{user_id}/policies` | 列出用户绑定的策略 |
| `POST` | `/admin/users/{user_id}/policies` | 给用户分配策略 |
| `PATCH` | `/admin/users/{user_id}/policies/{id}` | 更新用户策略项 |
| `DELETE` | `/admin/users/{user_id}/policies/{id}` | 删除用户策略项 |

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
| `GET` | `/admin/config/{key}` | 获取单个配置项 |
| `PUT` | `/admin/config/{key}` | 设置配置项 |
| `DELETE` | `/admin/config/{key}` | 删除配置项 |

### 当前常用 key

- `default_storage_quota`
- `webdav_enabled`
- `trash_retention_days`
- `max_versions_per_file`

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
