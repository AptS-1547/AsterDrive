# 数据库配置

```toml
[database]
url = "sqlite://asterdrive.db?mode=rwc"
pool_size = 10
retry_count = 3
```

## 字段说明

| 字段 | 类型 | 默认值 | 说明 |
| --- | --- | --- | --- |
| `url` | string | `"sqlite://asterdrive.db?mode=rwc"` | 数据库连接字符串 |
| `pool_size` | u32 | `10` | 连接池大小 |
| `retry_count` | u32 | `3` | 启动阶段数据库连接失败时的重试次数 |

## 支持的数据库

### SQLite

```toml
url = "sqlite://asterdrive.db?mode=rwc"
```

### PostgreSQL

```toml
url = "postgres://user:password@localhost:5432/asterdrive"
```

### MySQL

```toml
url = "mysql://user:password@localhost:3306/asterdrive"
```

## 启动时行为

每次启动都会：

1. 建立数据库连接
2. 执行全部 migration
3. 再继续初始化存储策略、缓存和 HTTP 服务

## 相对路径语义

默认 SQLite URL 使用相对路径，因此数据库文件会落在当前工作目录。

例如：

- 本地直接运行：落在你执行 `cargo run` 的目录
- systemd：落在 `WorkingDirectory`
- 容器：落在容器当前工作目录
