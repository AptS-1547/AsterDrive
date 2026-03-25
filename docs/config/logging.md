# 日志配置

```toml
[logging]
level = "info"
format = "text"
file = ""
enable_rotation = true
max_backups = 5
```

## 先决定日志写到哪里

- Docker：通常直接输出到 stdout
- systemd：通常交给 journald
- 单进程裸跑：可以写入单独的日志文件

## 字段说明

| 字段 | 默认值 | 说明 |
| --- | --- | --- |
| `level` | `"info"` | 日志级别：`trace`、`debug`、`info`、`warn`、`error` |
| `format` | `"text"` | 输出格式：`text` 或 `json` |
| `file` | `""` | 日志文件路径；留空时输出到 stdout |
| `enable_rotation` | `true` | 是否按天轮转日志文件，仅 `file` 非空时生效 |
| `max_backups` | `5` | 保留的历史日志文件数量 |

## 一般怎么选

- 本机排障：`text`
- 对接集中式日志系统：`json`
- Docker：通常留空，直接输出到 stdout
- systemd：可以留空交给 journald，也可以写到专用日志文件

## 日志轮转怎么理解

只有在你设置了 `logging.file` 的情况下，`enable_rotation` 和 `max_backups` 才会生效。

常见做法：

- Docker：不写文件，让容器日志系统处理
- systemd：不写文件，让 journald 处理
- 裸机单进程：写入文件并开启轮转

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

## 生产环境示例

```toml
[logging]
level = "info"
format = "json"
file = "/var/log/asterdrive.log"
enable_rotation = true
max_backups = 7
```

审计日志和这里的运行日志不是同一回事。运行日志用于排障；审计日志用于记录用户和管理员操作。
