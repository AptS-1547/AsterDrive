# 运维 CLI

::: tip 这一篇覆盖什么
`aster_drive` 可执行文件除了启动服务，还带一组命令行子命令：`doctor`（部署检查）、`config`（离线系统设置）、`database-migrate`（跨数据库迁移）。
**日常改设置优先走管理后台**——这一篇适合的是"后台进不去"、"想纳入脚本"、"要换数据库后端"这类场景。
:::

AsterDrive 现在自带一组命令行工具，适合下面这些场景：

- 服务已经部署好，但你想先离线检查一遍数据库和关键设置
- 管理后台暂时进不去，需要直接查看或修改系统设置
- 准备把 SQLite 迁到 PostgreSQL 或 MySQL，或者反过来迁回去
- 想把检查结果交给脚本、CI 或运维平台处理

这些命令都还是同一个 `aster_drive` 可执行文件。  
直接运行 `./aster_drive` 是启动服务；带子命令时，就是执行运维操作。

## 先准备数据库地址

最常见的写法：

```text
sqlite:///var/lib/asterdrive/data/asterdrive.db?mode=rwc
postgres://user:password@127.0.0.1:5432/asterdrive
mysql://user:password@127.0.0.1:3306/asterdrive
```

如果你用的是官方 Docker 容器，最省事的做法通常是先进入容器，再跑这些命令：

```bash
docker exec -it asterdrive sh
```

这样 SQLite 路径、挂载卷和容器里的实际文件位置不会搞混。

## 部署检查：`doctor`

第一次部署完成后，或者准备上线前，最值得先跑一次：

```bash
./aster_drive doctor \
  --database-url "sqlite:///var/lib/asterdrive/data/asterdrive.db?mode=rwc"
```

默认模式会检查这些最容易出问题的地方：

- 数据库能不能连上
- 数据库里还有没有待执行迁移
- 如果后端是 SQLite，`FTS5 + trigram tokenizer` 的搜索加速能力是否可用，相关 FTS 表 / 触发器是否齐全
- 运行时系统设置能不能正常读出
- `公开站点地址` 是否为空或格式不对
- `公开站点地址` 是否仍然是 `http://`，导致正式上线缺少 HTTPS
- 邮件投递配置是否完整
- 预览应用配置是否能正常解析
- 默认存储策略和默认策略组是否已经准备好

如果你希望把 `warn` 也当成失败处理，可以加：

```bash
./aster_drive doctor \
  --database-url "sqlite:///var/lib/asterdrive/data/asterdrive.db?mode=rwc" \
  --strict
```

需要给脚本处理时，再补一个输出格式：

```bash
./aster_drive doctor \
  --output-format json \
  --database-url "sqlite:///var/lib/asterdrive/data/asterdrive.db?mode=rwc"
```

最适合拿它来做这些事：

- 新部署后的首轮验收
- 升级后补一轮健康检查
- 改完 `公开站点地址`、邮件或预览应用后，确认没有把配置改坏
- 确认默认 SQLite 部署真的带上了搜索加速，而不是悄悄退回全表扫描

如果你怀疑库里已经有“数据和存储不一致”的问题，可以再跑深度检查：

```bash
./aster_drive doctor \
  --database-url "sqlite:///var/lib/asterdrive/data/asterdrive.db?mode=rwc" \
  --deep
```

`--deep` 额外会做这些检查：

- `storage-usage`：核对 `users.storage_used` / `teams.storage_used` 和文件、历史版本实际占用
- `blob-ref-counts`：核对 `file_blobs.ref_count` 与 `files` / `file_versions` 的真实引用数
- `storage-objects`：扫描每个存储策略下的对象路径，找出缺失 Blob、未追踪对象和孤儿缩略图
- `folder-tree`：检查缺失父目录、跨工作空间父目录和目录循环引用

如果你只想跑其中一部分，可以直接缩小范围：

```bash
./aster_drive doctor \
  --database-url "sqlite:///var/lib/asterdrive/data/asterdrive.db?mode=rwc" \
  --scope blob-ref-counts,storage-objects
```

如果你只想检查某个存储策略，可以再加：

```bash
./aster_drive doctor \
  --database-url "sqlite:///var/lib/asterdrive/data/asterdrive.db?mode=rwc" \
  --scope storage-objects \
  --policy-id 3
```

发现计数漂移时，可以让 CLI 直接修：

```bash
./aster_drive doctor \
  --database-url "sqlite:///var/lib/asterdrive/data/asterdrive.db?mode=rwc" \
  --deep \
  --fix
```

这里要注意四件事：

- `--scope` 只影响 deep checks，不会关闭数据库连接、迁移、运行时配置这些基础检查
- `--policy-id` 只作用于 `blob-ref-counts` 和 `storage-objects`；`storage-usage` 与 `folder-tree` 仍按全库核对
- `--fix` 目前只会修复 `storage_used` 和 `file_blobs.ref_count` 两类计数，不会自动删对象或改目录结构
- 深度扫描会按数据库批次和对象存储分页执行，但它只校验路径级存在性，不会读取对象内容或做 checksum

## 离线系统设置：`config`

平时改设置，还是优先走 `管理 -> 系统设置`。  
`config` 更适合下面这些情况：

- 后台暂时进不去
- 维护窗口里不想开网页操作
- 想批量导出、校验或导入系统设置

先看当前有哪些项：

```bash
./aster_drive config \
  --database-url "sqlite:///var/lib/asterdrive/data/asterdrive.db?mode=rwc" \
  list
```

只看某一项：

```bash
./aster_drive config \
  --database-url "sqlite:///var/lib/asterdrive/data/asterdrive.db?mode=rwc" \
  get \
  --key public_site_url
```

先校验，再落库：

```bash
./aster_drive config \
  --database-url "sqlite:///var/lib/asterdrive/data/asterdrive.db?mode=rwc" \
  validate \
  --key public_site_url \
  --value https://drive.example.com

./aster_drive config \
  --database-url "sqlite:///var/lib/asterdrive/data/asterdrive.db?mode=rwc" \
  set \
  --key public_site_url \
  --value https://drive.example.com
```

`public_site_url` 支持多个公开来源，一行一个。命令行里可以用 shell 的换行字符串：

```bash
./aster_drive config \
  --database-url "sqlite:///var/lib/asterdrive/data/asterdrive.db?mode=rwc" \
  set \
  --key public_site_url \
  --value $'https://drive.example.com\nhttps://panel.example.com'
```

批量导入时，输入文件可以是下面两种 JSON 之一：

```json
[
  { "key": "public_site_url", "value": "https://drive.example.com\nhttps://panel.example.com" },
  { "key": "auth_cookie_secure", "value": "true" }
]
```

```json
{
  "configs": [
    { "key": "public_site_url", "value": "https://drive.example.com\nhttps://panel.example.com" },
    { "key": "auth_cookie_secure", "value": "true" }
  ]
}
```

导入示例：

```bash
./aster_drive config \
  --database-url "sqlite:///var/lib/asterdrive/data/asterdrive.db?mode=rwc" \
  import \
  --input-file ./runtime-config.json
```

导出现有配置时，可以这样做：

```bash
./aster_drive config \
  --database-url "sqlite:///var/lib/asterdrive/data/asterdrive.db?mode=rwc" \
  --output-format pretty-json \
  export
```

导出结果更适合审阅、备份或交给脚本处理。  
如果你打算重新导入，先把它整理成上面那种“键值数组”或 `{"configs": [...]}` 结构，再交给 `import`。

如果你只是想确认某个值是否合法，优先用 `validate`，别上来就 `set`。

## 跨数据库迁移：`database-migrate`

这个命令是给“换数据库后端”用的。  
它不是日常启动时的自动 schema 迁移，而是把现有业务数据从一个数据库搬到另一个数据库。

最常见的场景：

- SQLite 迁到 PostgreSQL
- SQLite 迁到 MySQL
- PostgreSQL 和 MySQL 之间做后端切换

推荐顺序：

1. 先做一次 `--dry-run`
2. 准备停机窗口，避免源库在迁移过程中继续写入
3. 正式执行迁移
4. 看到 `ready_to_cutover = true` 后，再把生产实例切到新数据库

先试跑：

```bash
./aster_drive database-migrate \
  --source-database-url "sqlite:///var/lib/asterdrive/data/asterdrive.db?mode=rwc" \
  --target-database-url "postgres://user:password@127.0.0.1:5432/asterdrive_new" \
  --dry-run
```

正式执行：

```bash
./aster_drive database-migrate \
  --source-database-url "sqlite:///var/lib/asterdrive/data/asterdrive.db?mode=rwc" \
  --target-database-url "postgres://user:password@127.0.0.1:5432/asterdrive_new"
```

只做目标库校验：

```bash
./aster_drive database-migrate \
  --source-database-url "sqlite:///var/lib/asterdrive/data/asterdrive.db?mode=rwc" \
  --target-database-url "postgres://user:password@127.0.0.1:5432/asterdrive_new" \
  --verify-only
```

这个命令当前会自动处理这些事：

- 检查源库和目标库的迁移状态
- 自动把目标库 schema 补到当前版本
- 按固定顺序复制业务表
- 做行数、唯一约束和外键校验
- 在目标库里写入 checkpoint，命令中断后可以用同一条命令继续跑

用它时要记住三件事：

- 源库必须先是“当前 schema”，有待执行迁移就会直接拒绝
- 迁移过程中不要继续往源库写新数据
- 只有报告里的 `ready_to_cutover = true`，才说明目标库已经达到可切换状态

## 什么时候优先看这页

- 部署完成，但还没放心上线
- 后台打不开，又急着查配置
- 准备从 SQLite 切到 PostgreSQL / MySQL
- 想把检查动作做成脚本
