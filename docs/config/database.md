# 数据库配置

```toml
[database]
url = "sqlite://asterdrive.db?mode=rwc"
pool_size = 10
retry_count = 3
```

## 该选哪种数据库

- SQLite：单机、NAS、个人或小团队部署最省心
- PostgreSQL：已有现成 PostgreSQL 环境，或希望接入现有运维体系
- MySQL：你已经在用 MySQL，想保持统一

大多数第一次部署都可以先用 SQLite。

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

适合：

- 本机部署
- 小型团队
- 先快速把系统跑起来

### PostgreSQL

```toml
url = "postgres://user:password@localhost:5432/asterdrive"
```

适合：

- 已有 PostgreSQL 服务
- 需要接入现有备份和监控体系

### MySQL

```toml
url = "mysql://user:password@localhost:3306/asterdrive"
```

## 启动时会做什么

每次启动都会：

1. 建立数据库连接
2. 自动执行数据库迁移
3. 然后继续启动服务

## 相对路径语义

默认 SQLite 使用相对路径，所以数据库文件会落在当前工作目录。

例如：

- 本地直接运行：落在你执行 `cargo run` 的目录
- systemd：落在 `WorkingDirectory`
- 容器：落在容器当前工作目录
