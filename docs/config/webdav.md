# WebDAV 配置

WebDAV 相关配置分成两部分：

- 静态配置：`config.toml` 中的 `[webdav]`
- 运行时开关：数据库 `system_config` 中的 `webdav_enabled`

## 静态配置

```toml
[webdav]
prefix = "/webdav"
payload_limit = 10737418240
```

## 字段说明

| 字段 | 类型 | 默认值 | 说明 |
| --- | --- | --- | --- |
| `prefix` | string | `"/webdav"` | WebDAV 路径前缀，修改后需要重启 |
| `payload_limit` | usize | `10737418240` | WebDAV 请求体硬上限，默认 10 GiB |

## 运行时开关

`webdav_enabled` 为 `false` 时，WebDAV 路由仍存在，但所有请求会直接返回 `503`。

## 当前认证方式

WebDAV 支持两种认证头：

- `Authorization: Basic ...`
  - 使用独立的 `webdav_accounts`
  - 可限制到某个 `root_folder_id`
- `Authorization: Bearer <jwt>`
  - 复用普通登录后的 JWT
  - 访问范围是整个用户空间

## 路由注册顺序

WebDAV 在前端 SPA fallback 之前注册，因此：

- `/webdav` 不会被前端路由吞掉
- 修改 `prefix` 后，客户端挂载地址也必须同步修改

## 和普通 HTTP 限制的关系

- 普通 REST payload 上限：固定 `10 MiB`
- JSON body 上限：固定 `1 MiB`
- WebDAV payload 上限：单独走 `webdav.payload_limit`

## 反向代理要求

如果 WebDAV 放在反向代理后面，请确保代理层不会丢失：

- `Authorization`
- WebDAV 方法：`PROPFIND`、`LOCK`、`UNLOCK`、`MOVE`、`COPY`
- 相关头：`Depth`、`Destination`、`Overwrite`、`If`、`Lock-Token`

完整示例见 [反向代理部署](/deployment/proxy)。
