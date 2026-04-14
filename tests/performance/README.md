# Performance Benchmarks

Issue `#120` uses `k6` as the primary benchmark runner.

## What This Covers

- `auth-login.js`: login endpoint throughput
- `auth-refresh.js`: refresh endpoint concurrency
- `folder-list.js`: folder listing latency for `100` / `1000` / `10000` file directories
- `search.js`: search latency against the seeded corpus
- `download.js`: authenticated file download throughput
- `upload-direct.js`: direct multipart upload throughput
- `upload-chunked.js`: chunked upload throughput
- `batch-move.js`: concurrent batch move operations
- `webdav-rw.js`: WebDAV concurrent read/write flow
- `soak-mixed.js`: long-running mixed workload for memory / pool observation

## Prerequisites

1. Start AsterDrive in a local or staging environment.
2. Make sure the API is reachable.
3. Install `k6`.
4. Seed benchmark data once.

## Environment Variables

These defaults are shared by `seed.mjs` and the k6 scripts:

```bash
export ASTER_BENCH_BASE_URL="http://127.0.0.1:3000"
export ASTER_BENCH_USERNAME="bench_user"
export ASTER_BENCH_PASSWORD="bench-pass-1234"
export ASTER_BENCH_EMAIL="bench_user@example.com"
export ASTER_BENCH_SEARCH_TERM="needle"
export ASTER_BENCH_WEBDAV_USERNAME="bench_webdav"
export ASTER_BENCH_WEBDAV_PASSWORD="bench_webdav_pass123"
```

## Seed Data

Seed root folders and fixtures:

```bash
bun tests/performance/seed.mjs
```

Useful seed knobs:

```bash
ASTER_BENCH_LIST_SIZES=100,1000,10000
ASTER_BENCH_SEED_UPLOAD_CONCURRENCY=16
ASTER_BENCH_DOWNLOAD_BYTES=5242880
```

The seed step creates:

- `bench-list-100`
- `bench-list-1000`
- `bench-list-10000`
- `bench-download`
- `bench-batch-target`
- `bench-webdav`
- a reusable WebDAV account

## Local Benchmark Commands

Login:

```bash
k6 run tests/performance/k6/auth-login.js
```

Refresh:

```bash
k6 run tests/performance/k6/auth-refresh.js
```

Folder list:

```bash
ASTER_BENCH_LIST_SIZE=100 k6 run tests/performance/k6/folder-list.js
ASTER_BENCH_LIST_SIZE=1000 k6 run tests/performance/k6/folder-list.js
ASTER_BENCH_LIST_SIZE=10000 k6 run tests/performance/k6/folder-list.js
```

Search:

```bash
k6 run tests/performance/k6/search.js
```

Download:

```bash
k6 run tests/performance/k6/download.js
```

Direct upload:

```bash
k6 run tests/performance/k6/upload-direct.js
```

Chunked upload:

```bash
k6 run tests/performance/k6/upload-chunked.js
```

Batch move:

```bash
k6 run tests/performance/k6/batch-move.js
```

WebDAV read/write:

```bash
k6 run tests/performance/k6/webdav-rw.js
```

Long soak:

```bash
ASTER_BENCH_SOAK_DURATION=24h \
ASTER_BENCH_SUMMARY_DIR=tests/performance/results \
k6 run tests/performance/k6/soak-mixed.js
```

## Collecting Summaries

If `ASTER_BENCH_SUMMARY_DIR` is set, each script writes a compact JSON summary:

```bash
mkdir -p tests/performance/results/local
ASTER_BENCH_SUMMARY_DIR=tests/performance/results/local \
k6 run tests/performance/k6/download.js
```

## Soak-Test Observation

`soak-mixed.js` only drives workload. Pair it with runtime monitoring:

- local process: `scripts/test.sh` or system tools such as `ps`, `vm_stat`, `top`
- container runtime: `scripts/monitor.sh`
- optional metrics endpoint: run the server with the `metrics` feature and scrape `/health/metrics`

Recommended soak checklist:

1. Run `soak-mixed.js` for `24h`.
2. Sample RSS / heap / CPU every `30s` to `60s`.
3. Watch p95 latency drift in the k6 summary.
4. Watch DB pool exhaustion, request retries, and cleanup backlog in logs.

