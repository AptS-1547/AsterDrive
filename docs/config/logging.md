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

## 不合法值和文件不可写时的行为

当前代码会尽量降级而不是直接启动失败：

- `logging.level` 非法：回退到 `info`
- `logging.file` 打不开：回退到 stdout

这些回退都会产生 warning。

## 生产环境建议

```toml
[logging]
level = "info"
format = "json"
file = "/var/log/asterdrive.log"
```
