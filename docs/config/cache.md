# 缓存配置

```toml
[cache]
enabled = true
backend = "memory"
redis_url = ""
default_ttl = 3600
```

## 大多数部署怎么选

如果你只是单机部署、NAS 部署或普通小团队使用，保持默认的内存缓存就够了。

只有在多实例部署，或者你明确希望多个应用实例共享缓存命中时，才需要考虑 Redis。

## 字段说明

| 字段 | 类型 | 默认值 | 说明 |
| --- | --- | --- | --- |
| `enabled` | bool | `true` | 是否启用缓存 |
| `backend` | string | `"memory"` | 缓存后端，支持 `memory` 与 `redis` |
| `redis_url` | string | `""` | Redis 连接地址，仅 `backend = "redis"` 时使用 |
| `default_ttl` | u64 | `3600` | 默认 TTL，单位秒 |

## 什么时候需要 Redis

- 单机、小规模部署：默认 `memory` 足够
- 多实例部署：可以考虑 `redis`
- 不确定时：先不要引入 Redis，保持默认即可

## 关闭缓存

```toml
[cache]
enabled = false
```
