# 性能基准与压测

AsterDrive 的性能基准脚本放在仓库里的 `tests/performance/`。

这套基准的目标不是给出“万能容量数字”，而是让你在自己的机器、自己的数据库、自己的存储策略上跑出可复现的结果，然后把这些结果作为版本升级前后的回归对照。

## 基准范围

当前基准覆盖 issue `#120` 里列出的核心场景：

- 登录与 refresh 并发
- 文件列表查询（`100` / `1000` / `10000` 文件目录）
- 搜索查询
- 并发文件下载
- 并发 direct 上传
- 并发 chunked 上传
- 批量移动并发
- WebDAV 读写并发
- staged mixed workload ramp（看延迟/失败率随并发抬升）
- 长稳 mixed workload soak 测试

## 工具选择

- 主基准：`k6`
- 数据预热：`bun tests/performance/seed.mjs`
- 长稳观测：配合 `scripts/monitor.sh`、系统进程指标或 `/health/metrics`

## 先准备环境

1. 用接近生产的方式启动服务，推荐 `cargo run --profile release-performance`
2. 指向一个独立的数据库和独立的本地存储目录
3. 打开 `ASTER__AUTH__BOOTSTRAP_INSECURE_COOKIES=true`，方便本地 HTTP 压测
4. 安装 `k6`
5. 先跑一次 seed

示例：

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

## 本地跑法

性能脚本说明见 [`tests/performance/README.md`](https://github.com/AptS-1547/AsterDrive/blob/master/tests/performance/README.md)。

常用命令：

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

`ASTER_BENCH_MIXED_RAMP_STAGES` 的格式是 `target_vus:duration`，比如 `32:30s`。

如果你要把结果落盘：

```bash
mkdir -p tests/performance/results/local
ASTER_BENCH_SUMMARY_DIR=tests/performance/results/local \
k6 run tests/performance/k6/download.js
```

现在下载、上传、WebDAV 和 `mixed-ramp.js` 的 summary 里都会带字节计数器，可以直接拿 `count` / `rate` 看有效吞吐，不用只看 `http_req_duration` 这种会把你带沟里的单请求延迟。

## SQLite 搜索验证

如果你的部署后端是 SQLite，跑搜索压测前先确认两件事：

1. `doctor` 里 `SQLite search acceleration` 是 `ok`
2. `EXPLAIN QUERY PLAN` 能看到 `files_name_fts` / `folders_name_fts` 的 `VIRTUAL TABLE INDEX`

示例：

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

你想看到的重点不是绝对数字，而是类似下面这种规划器输出：

- `SCAN files_name_fts VIRTUAL TABLE INDEX ...`
- `SEARCH files USING INTEGER PRIMARY KEY ...`

如果你看到的是对 `files` / `folders` 的普通全表 `SCAN`，那就别拿这台实例的搜索压测结果当真，先把 SQLite 运行时和迁移状态查明白。

## 长稳测试

`soak-mixed.js` 只负责持续制造混合流量；真正要看的是服务进程的 RSS、CPU、堆占用、延迟漂移和连接池行为。

推荐组合：

```bash
ASTER_BENCH_SOAK_DURATION=24h \
ASTER_BENCH_SUMMARY_DIR=tests/performance/results/soak \
k6 run tests/performance/k6/soak-mixed.js
```

另开一个观测终端：

```bash
./scripts/monitor.sh 30 /tmp/asterdrive-soak.csv
```

如果你是容器部署，就在容器里跑同名脚本。

长稳测试建议重点看：

- 6 小时到 24 小时内 p95 是否持续抬升
- RSS / heap 是否单向增长且不回落
- 日志里是否出现数据库连接池耗尽、重试或清理积压
- 上传和下载吞吐是否随着时间明显退化

## 手动 CI Smoke

仓库带了一个不会参与日常 PR/Push 阻塞的手动 workflow：

- 文件：`.github/workflows/performance.yml`
- 触发方式：GitHub Actions 里的 `workflow_dispatch`

它会：

1. 构建前端和后端
2. 本地起一个 release-performance 服务
3. 跑轻量 seed
4. 执行一组短时 smoke benchmark
5. 上传 summary artifact

这套 workflow 只负责“脚本还能跑、主要路径没烂掉”，不是正式容量验证。

## 本地 Smoke 基线示例

下面这组数据是 `2026-04-15` 在一台本地开发机上跑出来的 smoke baseline，主要用来给脚本验收和后续版本回归提供一个对照样本，不代表生产环境容量上限，也不应直接外推成部署建议。

运行环境：

- 日期：`2026-04-15`
- 主机：Apple M2 Pro / 32 GB / macOS 15.7.4 / `arm64`
- 二进制：`target/release-performance/aster_drive`
- 数据库：SQLite
- 存储：本地文件系统

核心结果：

| 场景 | 口径 | Avg | p95 | 速率 |
| --- | --- | --- | --- | --- |
| Login | `auth-login.js` | `97.27 ms` | `111.71 ms` | `61.57 req/s` |
| Folder list 100 | `folder-list.js` | `4.68 ms` | `6.28 ms` | `1216.62 req/s` |
| Folder list 1000 | `folder-list.js` | `4.96 ms` | `5.62 ms` | `1154.71 req/s` |
| Folder list 10000 | `folder-list.js` | `11.93 ms` | `13.12 ms` | `490.28 req/s` |
| Search | `search.js` | `13.24 ms` | `14.09 ms` | `445.35 req/s` |
| Download 5 MiB | `download.js` | `5.37 ms` | `6.61 ms` | `733.75 req/s` |
| Direct upload 1 MiB | `upload-direct.js` | `3.80 ms` | `9.30 ms` | `715.24 req/s` |
| Chunked upload 10 MiB | flow metric | `61.91 ms` | `74.00 ms` | 单次 flow 样本 |
| Batch move 10 files | flow metric | `13.12 ms` | `21.91 ms` | 单次 flow 样本 |
| WebDAV PUT 64 KiB | `webdav-rw.js` | `52.81 ms` | `65.15 ms` | 单次 flow 样本 |
| WebDAV GET 64 KiB | `webdav-rw.js` | `50.60 ms` | `54.45 ms` | 单次 flow 样本 |
