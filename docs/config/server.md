# 服务器配置

```toml
[server]
host = "127.0.0.1"
port = 3000
workers = 0
```

## 字段说明

| 字段 | 类型 | 默认值 | 说明 |
| --- | --- | --- | --- |
| `host` | string | `"127.0.0.1"` | 绑定地址；容器或反向代理场景通常应设为 `0.0.0.0` |
| `port` | u16 | `3000` | HTTP 监听端口 |
| `workers` | usize | `0` | Actix worker 数量；`0` 表示自动取 CPU 核心数 |

## 固定的请求体限制

除 WebDAV 以外，当前代码里还有两处固定上限：

- 通用 payload：`10 MiB`
- JSON body：`1 MiB`

这两个值在 `src/main.rs` 里硬编码，不能通过 `config.toml` 修改。

WebDAV 例外，单独走 `[webdav].payload_limit`。

## Keep-Alive 与超时

当前 HTTP 服务还固定了这些行为：

- keep-alive：30 秒
- client request timeout：5 秒
- client disconnect timeout：1 秒

## 容器环境常见设置

```bash
ASTER__SERVER__HOST=0.0.0.0
ASTER__SERVER__PORT=3000
```
