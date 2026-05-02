# Running Migrator CLI

## Baseline rebase policy

The current migration set is rebased into `m20260502_000001_baseline_schema`.

- Fresh installs run the new baseline directly.
- Existing alpha deployments must first run all migrations from `v0.0.1-alpha.25`.
- When a complete alpha.25 migration history is detected, AsterDrive validates key schema sentinels and rewrites only `seaql_migrations` to the new baseline stamp.
- Incomplete pre-rebase histories are rejected with an instruction to upgrade to `v0.0.1-alpha.25` first.

Do not truncate application tables for this rebase. Only migration metadata is rewritten.

- Generate a new migration file
    ```sh
    cargo run -p migration --features cli -- generate MIGRATION_NAME
    ```
- Apply all pending migrations
    ```sh
    cargo run -p migration --features cli
    ```
    ```sh
    cargo run -p migration --features cli -- up
    ```
- Apply first 10 pending migrations
    ```sh
    cargo run -p migration --features cli -- up -n 10
    ```
- Rollback last applied migrations
    ```sh
    cargo run -p migration --features cli -- down
    ```
- Rollback last 10 applied migrations
    ```sh
    cargo run -p migration --features cli -- down -n 10
    ```
- Drop all tables from the database, then reapply all migrations
    ```sh
    cargo run -p migration --features cli -- fresh
    ```
- Rollback all applied migrations, then reapply all migrations
    ```sh
    cargo run -p migration --features cli -- refresh
    ```
- Rollback all applied migrations
    ```sh
    cargo run -p migration --features cli -- reset
    ```
- Check the status of all migrations
    ```sh
    cargo run -p migration --features cli -- status
    ```
