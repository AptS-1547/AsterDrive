# 缓存配置

```toml
[cache]
enabled = true
backend = "memory"
redis_url = ""
default_ttl = 3600
```

## 大多数部署怎么选

如果你是单机部署、NAS 部署或普通小团队使用，保持默认的内存缓存就够了。

只有在这些场景里，才值得考虑 Redis：

- 多实例部署
- 希望多个应用实例共享缓存

## 这些选项怎么理解

| 选项 | 默认值 | 作用 |
| --- | --- | --- |
| `enabled` | `true` | 是否启用缓存 |
| `backend` | `"memory"` | 缓存方式，支持 `memory` 与 `redis` |
| `redis_url` | `""` | Redis 连接地址，仅 `backend = "redis"` 时使用 |
| `default_ttl` | `3600` | 默认 TTL，单位秒 |

## 什么时候需要 Redis

- 单机、小规模部署：默认 `memory` 足够
- 多实例部署：可以考虑 `redis`
- 不确定时：先不要引入 Redis

## 如果 Redis 连不上会怎样

当你把 `backend` 设成 `redis` 但 Redis 连接失败时，AsterDrive 会自动回退到内存缓存继续运行。  
这意味着服务一般不会因为 Redis 暂时不可用而直接起不来，但多实例之间就不会再共享缓存。

## 关闭缓存

```toml
[cache]
enabled = false
```

即使关闭缓存，AsterDrive 仍然可以正常运行，只是部分查询和读取不会命中缓存。

## 对应环境变量

```bash
ASTER__CACHE__ENABLED=true
ASTER__CACHE__BACKEND=memory
ASTER__CACHE__REDIS_URL=redis://127.0.0.1:6379/0
```
