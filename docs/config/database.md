# 数据库配置

`[database]` 决定 AsterDrive 连接哪个数据库，以及启动时数据库连接要重试几次。

```toml
[database]
url = "sqlite://asterdrive.db?mode=rwc"
pool_size = 10
retry_count = 3
```

## 先选数据库类型

- SQLite：单机、NAS、个人或小团队部署最省心
- PostgreSQL：已经有现成 PostgreSQL，或者希望接入现有运维体系
- MySQL：已经在用 MySQL，想保持统一

第一次部署，大多数场景都可以先用 SQLite。

## 这些选项怎么理解

| 选项 | 默认值 | 作用 |
| --- | --- | --- |
| `url` | `"sqlite://asterdrive.db?mode=rwc"` | 数据库连接字符串 |
| `pool_size` | `10` | 连接池大小 |
| `retry_count` | `3` | 启动阶段数据库连接失败时的重试次数 |

## 常见写法

### SQLite

```toml
url = "sqlite://asterdrive.db?mode=rwc"
```

Docker 里更常见的写法是：

```toml
url = "sqlite:///data/asterdrive.db?mode=rwc"
```

### PostgreSQL

```toml
url = "postgres://user:password@localhost:5432/asterdrive"
```

### MySQL

```toml
url = "mysql://user:password@localhost:3306/asterdrive"
```

## 启动时会自动做什么

每次启动时，AsterDrive 都会：

1. 建立数据库连接
2. 自动更新数据库结构
3. 然后继续启动服务

所以大多数部署不需要再手动执行迁移命令。

## 换数据库后端时不要直接改 `url`

如果你只是做同一种数据库里的正常升级，直接启动服务就会自动补齐 schema。  
但如果你要把数据从 SQLite 换到 PostgreSQL 或 MySQL，不能只改 `database.url` 然后重启。

原因很简单：

- 启动时自动迁移只负责“更新目标库结构”
- 它不会把旧数据库里的业务数据自动搬过去

这类场景要先用 [运维 CLI](/deployment/ops-cli) 里的 `database-migrate` 把数据迁过去，再切换生产实例。

## SQLite 的路径语义

默认 SQLite 使用相对路径时，会相对于 `data/config.toml` 所在目录解析。

例如：

- 本地直接运行：默认落在 `./data/asterdrive.db`
- systemd：默认落在 `WorkingDirectory/data/asterdrive.db`
- Docker：如果你写成 `sqlite:///data/asterdrive.db?mode=rwc`，数据库会落在 `/data`

长期部署时，建议把 SQLite 放到固定目录或持久化卷里。

## 什么时候需要改 `pool_size` 和 `retry_count`

- 单机、小团队：通常保持默认
- 外部数据库启动较慢：可以适当提高 `retry_count`
- 并发较高、数据库本身也允许更多连接：再考虑提高 `pool_size`

## 对应环境变量

```bash
ASTER__DATABASE__URL="sqlite:///data/asterdrive.db?mode=rwc"
ASTER__DATABASE__POOL_SIZE=10
ASTER__DATABASE__RETRY_COUNT=3
```
