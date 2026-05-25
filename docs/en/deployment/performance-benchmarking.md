# Performance Benchmarking and Load Testing

::: tip What this page covers
AsterDrive's built-in k6 load testing scripts: login, listing, search, upload and download, WebDAV, mixed traffic, and long-running stability tests.
This page **does not provide a universal capacity number**. The goal is to produce reproducible results on your own machine, database, and storage, then use them for regression comparison before and after version changes.
At the end of this page, there is an Apple M2 Pro / SQLite smoke baseline as a comparison starting point, but it **does not represent a production capacity limit**.
:::

AsterDrive's performance benchmark scripts are under `tests/performance/` in the repository.

The goal of these benchmarks is not to provide a "universal capacity number". Instead, they help you produce reproducible results on your own machine, database, and storage policy, then use those results as regression comparisons before and after upgrades.

## Benchmark Scope

Current benchmarks cover the core scenarios listed in issue `#120`:

- login and refresh concurrency
- file listing queries (`100` / `1000` / `10000` file directories)
- search queries
- concurrent file downloads
- concurrent direct uploads
- concurrent chunked uploads
- batch move concurrency
- WebDAV read/write concurrency
- staged mixed workload ramp (watch latency/failure rate as concurrency increases)
- long-running mixed workload soak test

## Tooling

- Main benchmark: `k6`
- Data seeding: `bun tests/performance/seed.mjs`
- Long-running observation: combine with `scripts/monitor.sh`, system process metrics, or `/health/metrics`

## Prepare the Environment

1. Start the service in a production-like way. `cargo run --profile release-performance` is recommended.
2. Point it at an independent database and independent local storage directory.
3. Enable `ASTER__AUTH__BOOTSTRAP_INSECURE_COOKIES=true` for convenient local HTTP benchmarking.
4. Install `k6`.
5. Run seed once first.

Example:

```bash
export ASTER_BENCH_BASE_URL="http://127.0.0.1:3000"
export ASTER_BENCH_USERNAME="bench_user"
export ASTER_BENCH_PASSWORD="bench-pass-1234"
export ASTER_BENCH_EMAIL="bench_user@example.com"
export ASTER_BENCH_SEARCH_TERM="needle"
export ASTER_BENCH_WEBDAV_USERNAME="bench_webdav"
export ASTER_BENCH_WEBDAV_PASSWORD="bench_webdav_pass123"

bun tests/performance/seed.mjs
```

## Running Locally

Performance script details are in [`tests/performance/README.md`](https://github.com/AptS-1547/AsterDrive/blob/master/tests/performance/README.md).

Common commands:

```bash
k6 run tests/performance/k6/auth-login.js
k6 run tests/performance/k6/auth-refresh.js

ASTER_BENCH_LIST_SIZE=100 k6 run tests/performance/k6/folder-list.js
ASTER_BENCH_LIST_SIZE=1000 k6 run tests/performance/k6/folder-list.js
ASTER_BENCH_LIST_SIZE=10000 k6 run tests/performance/k6/folder-list.js

k6 run tests/performance/k6/search.js
k6 run tests/performance/k6/download.js
k6 run tests/performance/k6/upload-direct.js
k6 run tests/performance/k6/upload-chunked.js
k6 run tests/performance/k6/batch-move.js
k6 run tests/performance/k6/webdav-rw.js
ASTER_BENCH_MIXED_RAMP_STAGES=1:20s,8:30s,32:30s,64:45s,0:15s \
k6 run tests/performance/k6/mixed-ramp.js
```

The `ASTER_BENCH_MIXED_RAMP_STAGES` format is `target_vus:duration`, for example `32:30s`.

To write results to disk:

```bash
mkdir -p tests/performance/results/local
ASTER_BENCH_SUMMARY_DIR=tests/performance/results/local \
k6 run tests/performance/k6/download.js
```

Download, upload, WebDAV, and `mixed-ramp.js` summaries now include byte counters. You can use `count` / `rate` directly to inspect effective throughput, instead of only looking at single-request latency such as `http_req_duration`, which does not fully represent effective throughput.

## SQLite Search Validation

If your deployment backend is SQLite, confirm two things before running search benchmarks:

1. `SQLite search acceleration` is `ok` in `doctor`.
2. `EXPLAIN QUERY PLAN` shows the `VIRTUAL TABLE INDEX` for `files_name_fts` / `folders_name_fts`.

Example:

```bash
./aster_drive doctor \
  --database-url "sqlite:///var/lib/asterdrive/data/asterdrive.db?mode=rwc" \
  --output-format human

sqlite3 /var/lib/asterdrive/data/asterdrive.db "
EXPLAIN QUERY PLAN
SELECT files.id, files.name, file_blobs.size
FROM files_name_fts
JOIN files ON files_name_fts.rowid = files.id
JOIN file_blobs ON file_blobs.id = files.blob_id
WHERE files_name_fts MATCH '\"needle\"'
  AND files.deleted_at IS NULL
  AND files.user_id = 1
  AND files.team_id IS NULL
ORDER BY files.name ASC
LIMIT 50 OFFSET 0;
"
```

The important thing is not the absolute number, but planner output like:

- `SCAN files_name_fts VIRTUAL TABLE INDEX ...`
- `SEARCH files USING INTEGER PRIMARY KEY ...`

If you see ordinary full-table `SCAN` on `files` / `folders`, those results are not suitable as a search benchmark baseline. First investigate the SQLite runtime and migration state.

## Soak Test

`soak-mixed.js` only generates sustained mixed traffic. What you actually need to watch is service process RSS, CPU, heap usage, latency drift, and connection pool behavior.

Recommended combination:

```bash
ASTER_BENCH_SOAK_DURATION=24h \
ASTER_BENCH_SUMMARY_DIR=tests/performance/results/soak \
k6 run tests/performance/k6/soak-mixed.js
```

Open another terminal for observation:

```bash
./scripts/monitor.sh 30 /tmp/asterdrive-soak.csv
```

If you deploy in a container, run the script with the same name inside the container.

For long-running stability tests, focus on:

- whether p95 keeps rising between 6 and 24 hours
- whether RSS / heap only grows and does not drop
- whether logs show database connection pool exhaustion, retries, or cleanup backlog
- whether upload and download throughput degrades noticeably over time

## Manual CI Smoke

The repository includes a manual workflow that does not block normal PR / Push:

- File: `.github/workflows/performance.yml`
- Trigger: `workflow_dispatch` in GitHub Actions

It:

1. builds frontend and backend
2. starts a local release-performance service
3. runs lightweight seed
4. executes a short smoke benchmark set
5. uploads summary artifacts

This workflow only checks that "scripts still run and major paths did not regress". It is not formal capacity validation.

## Local Smoke Baseline Example

The data below is a smoke baseline from `2026-04-15` on a local development machine. It mainly provides a comparison sample for script acceptance and future version regression checks. It does not represent a production environment capacity limit and should not be directly extrapolated into deployment guidance.

Runtime environment:

- Date: `2026-04-15`
- Host: Apple M2 Pro / 32 GB / macOS 15.7.4 / `arm64`
- Binary: `target/release-performance/aster_drive`
- Database: SQLite
- Storage: local filesystem

Core results:

| Scenario | Measurement | Avg | p95 | Rate |
| --- | --- | --- | --- | --- |
| Login | `auth-login.js` | `97.27 ms` | `111.71 ms` | `61.57 req/s` |
| Folder list 100 | `folder-list.js` | `4.68 ms` | `6.28 ms` | `1216.62 req/s` |
| Folder list 1000 | `folder-list.js` | `4.96 ms` | `5.62 ms` | `1154.71 req/s` |
| Folder list 10000 | `folder-list.js` | `11.93 ms` | `13.12 ms` | `490.28 req/s` |
| Search | `search.js` | `13.24 ms` | `14.09 ms` | `445.35 req/s` |
| Download 5 MiB | `download.js` | `5.37 ms` | `6.61 ms` | `733.75 req/s` |
| Direct upload 1 MiB | `upload-direct.js` | `3.80 ms` | `9.30 ms` | `715.24 req/s` |
| Chunked upload 10 MiB | flow metric | `61.91 ms` | `74.00 ms` | single flow sample |
| Batch move 10 files | flow metric | `13.12 ms` | `21.91 ms` | single flow sample |
| WebDAV PUT 64 KiB | `webdav-rw.js` | `52.81 ms` | `65.15 ms` | single flow sample |
| WebDAV GET 64 KiB | `webdav-rw.js` | `50.60 ms` | `54.45 ms` | single flow sample |
