# 日志配置

```toml
[logging]
level = "info"
format = "text"
file = ""
```

## 字段说明

| 字段 | 类型 | 默认值 | 说明 |
| --- | --- | --- | --- |
| `level` | string | `"info"` | 日志级别：`trace`、`debug`、`info`、`warn`、`error` |
| `format` | string | `"text"` | 输出格式：`text` 或 `json` |
| `file` | string | `""` | 日志文件路径；留空时输出到 stdout |

## 一般怎么选

- 本机排障：`text`
- 对接集中式日志系统：`json`
- Docker：通常留空，直接输出到 stdout
- systemd：可以留空交给 journald，也可以写到专用日志文件

## 优先级

日志初始化时会优先读取 `RUST_LOG`，如果没有再回退到 `logging.level`。

例如：

```bash
RUST_LOG=debug
```

也可以继续通过配置系统环境变量覆盖：

```bash
ASTER__LOGGING__LEVEL=debug
```

## 生产环境建议

```toml
[logging]
level = "info"
format = "json"
file = "/var/log/asterdrive.log"
```

审计日志和这里的运行日志不是同一回事。运行日志用于排障；审计日志用于记录用户和管理员操作。
