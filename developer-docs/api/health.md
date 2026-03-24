# 健康检查 API

健康检查路径不在 `/api/v1` 下，而是直接挂在根路径。

## 接口列表

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| `GET` / `HEAD` | `/health` | 存活检查 |
| `GET` / `HEAD` | `/health/ready` | 就绪检查，包含数据库连通性 |
| `GET` | `/health/memory` | 堆内存统计 |
| `GET` | `/health/metrics` | Prometheus 指标，仅 `metrics` feature 启用时存在 |

## `GET /health`

典型响应：

```json
{
  "code": 0,
  "msg": "",
  "data": {
    "status": "ok",
    "version": "0.0.0",
    "build_time": "2026-03-22T00:00:00Z"
  }
}
```

`build_time` 来自编译期写入的 `ASTER_BUILD_TIME`。

## `GET /health/ready`

该接口会 `ping` 数据库：

- 数据库正常：`200`
- 数据库不可用：`503`

部署建议：

- 用 `/health` 做 liveness / 基础探活
- 用 `/health/ready` 做 readiness / 上线前探针

## `GET /health/memory`

返回当前堆分配量与峰值，单位是 MB 字符串。

## `GET /health/metrics`

只有在编译时启用了 `metrics` feature 才会注册，输出格式为 Prometheus text exposition。

这个接口更适合 Prometheus 等监控系统抓取，不建议直接暴露给公网。
