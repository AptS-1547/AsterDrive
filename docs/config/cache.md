# 缓存

::: tip 这一篇覆盖 `[cache]`
单机部署保持默认（内存缓存）就够了。只有多实例部署、希望共享缓存时才考虑 Redis。
不确定要不要引入 Redis？答案多半是"不要"。
:::

```toml
[cache]
enabled = true
backend = "memory"
redis_url = ""
default_ttl = 3600
```

## 大多数部署直接保持默认

单机、NAS、小团队部署，内存缓存够用。**只有这两种情况才值得上 Redis**：

- 多实例部署
- 多个应用实例之间需要共享缓存

## 选项一览

| 选项 | 默认值 | 作用 |
| --- | --- | --- |
| `enabled` | `true` | 是否启用缓存 |
| `backend` | `"memory"` | `memory` 或 `redis` |
| `redis_url` | `""` | Redis 连接地址，仅 `backend = "redis"` 时使用 |
| `default_ttl` | `3600` | 默认 TTL，单位秒 |

## Redis 连不上会怎样

把 `backend` 设成 `redis` 但 Redis 连不上时，AsterDrive 会**自动回退到内存缓存继续运行**。

服务一般不会因为 Redis 暂时不可用就直接起不来——但多实例之间也就不再共享缓存了。

## 关闭缓存

```toml
[cache]
enabled = false
```

关掉之后服务仍能正常运行，只是部分查询和读取不再命中缓存。**性能问题排查时**这是一个有用的对照开关。

## 对应环境变量

```bash
ASTER__CACHE__ENABLED=true
ASTER__CACHE__BACKEND=memory
ASTER__CACHE__REDIS_URL=redis://127.0.0.1:6379/0
ASTER__CACHE__DEFAULT_TTL=3600
```
