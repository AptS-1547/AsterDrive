# Database Configuration

::: tip This page covers `[database]`
Which database to connect to, how large the connection pool should be, and how many startup retries to attempt. Not sure which one to choose for a first deployment? Use SQLite. It is easy to change later.
:::

```toml
[database]
url = "sqlite://asterdrive.db?mode=rwc"
pool_size = 10
retry_count = 3
```

## Choose a Database Type First

- **SQLite** - Best for single-node, NAS, personal, or small-team deployments. It has the least operational overhead.
- **PostgreSQL** - Use it if you already run PostgreSQL or want to integrate with an existing operations stack.
- **MySQL** - Use it if you already use MySQL and want to keep the stack consistent.

::: tip Use SQLite for the first deployment
SQLite is enough for most scenarios. When the deployment grows, you can switch with AsterDrive's built-in cross-database migration tool instead of being locked into the initial choice.
:::

## Options

| Option | Default | Purpose |
| --- | --- | --- |
| `url` | `"sqlite://asterdrive.db?mode=rwc"` | Database connection string |
| `pool_size` | `10` | Connection pool size |
| `retry_count` | `3` | Number of retries when the database connection fails during startup |

## Common Examples

### SQLite

```toml
url = "sqlite://asterdrive.db?mode=rwc"
```

A more common Docker example:

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

## What Happens During Startup

On every startup, AsterDrive will:

1. Open the database connection
2. Apply migrations automatically to update the schema
3. Continue starting the service

**So daily upgrades do not require you to run migration commands manually.**

## Do Not Change `url` Directly When Switching Database Backends

::: warning Upgrading within the same database type is OK. Switching across database types is not.
- Normal upgrades within the same database type - restart directly, and the schema will be completed automatically
- Switching from SQLite to PostgreSQL/MySQL - **do not just change `url` and restart**

Startup migrations only "update the target database schema"; they do not move business data from the old database. For this scenario, first use the `database-migrate` command in the [operations CLI](/en/deployment/ops-cli) to migrate the data, then switch the production instance.
:::

## SQLite Path Semantics

When SQLite uses a relative path, it is resolved relative to the directory containing `data/config.toml`.

| Deployment Method | Default Location |
| --- | --- |
| Run locally | `./data/asterdrive.db` |
| systemd | `WorkingDirectory/data/asterdrive.db` |
| Docker (`sqlite:///data/asterdrive.db?mode=rwc`) | `/data` inside the container |

For long-running deployments, write the SQLite path as a fixed directory or mount it on a persistent volume to avoid surprises from working directory changes.

## How to Tune `pool_size` and `retry_count`

- Single-node or small-team deployment: keep the defaults
- External database starts slowly, such as when orchestration starts the DB later than AsterDrive: increase `retry_count`
- High concurrency and the database itself allows more connections: then consider increasing `pool_size`

## Environment Variables

```bash
ASTER__DATABASE__URL="sqlite:///data/asterdrive.db?mode=rwc"
ASTER__DATABASE__POOL_SIZE=10
ASTER__DATABASE__RETRY_COUNT=3
```
