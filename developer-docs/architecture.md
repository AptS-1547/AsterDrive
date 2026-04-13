# AsterDrive 架构概览

本文描述的是当前仓库已经落地的实现，不是早期设计草图。

如果你是第一次进入这个仓库，建议按下面的顺序读：

1. 先看“给新开发者的 60 秒版本”
2. 再看“从哪里开始看代码”
3. 然后看“一个请求如何流转”
4. 最后再回头看具体的数据模型和链路细节

## 给新开发者的 60 秒版本

- AsterDrive 目前是一个单体应用：一个 Rust 进程同时提供 REST API、健康检查、WebDAV，并负责把前端页面直接服务出去
- 元数据主要在数据库里，文件内容主要在存储驱动里；两者通过 `files` / `file_blobs` / `file_versions` 等表串起来
- 个人空间和团队空间共用同一条文件主链路，只是 route 层通过 `WorkspaceStorageScope` 切换作用域
- 绝大多数功能改动都落在这条主线上：
  `src/api/routes/*` -> `src/services/*` -> `src/db/repository/*` / `src/storage/*`
- 前端代码在 `frontend-panel/`，但生产运行时仍由后端统一托管
- 团队相关入口主要在 `src/api/routes/teams.rs` 与 `src/api/routes/team_*`；团队空间里的文件操作仍复用 `files` / `folders` / `upload` / `share` / `trash` 这些 service
- WebDAV 不是普通 REST 路由的一个小分支，而是一套单独的接入路径，主要代码在 `src/webdav/`
- 配置分两类：
  - 静态配置：`data/config.toml` + 环境变量，影响进程启动和基础设施
  - 运行时配置：数据库里的 `system_config`，影响 WebDAV 开关、回收站保留期、版本数上限等业务行为
    也包括默认配额、公开站点地址、预览应用注册表、后台调度周期、团队归档保留期、CORS、注册/邮件/头像和 WOPI 等配置；完整单一数据源是 `src/config/definitions.rs`

## 从哪里开始看代码

按你要解决的问题，优先看这些位置：

| 你想回答的问题 | 先看哪里 | 为什么 |
| --- | --- | --- |
| 服务怎么启动、路由怎么挂上去 | `src/main.rs`、`src/runtime/startup.rs`、`src/api/mod.rs` | 这里决定了启动顺序、路由注册顺序、后台任务和全局中间件 |
| 一个 REST 接口怎么实现 | `src/api/routes/*` 对应文件 | 这里负责请求参数、鉴权包装、响应格式 |
| 团队工作空间为什么和个人空间长得几乎一样 | `src/api/routes/teams.rs`、`src/api/routes/team_*`、`src/services/workspace_storage_service.rs` | route 层只换作用域，真正的文件 / 文件夹 / 上传语义还是复用同一套 service |
| 权限、配额、锁、回收站、版本这类业务规则在哪 | `src/services/*` | 业务规则主要集中在 service 层，不应散落在 route 或 repo 里 |
| 数据是怎么查和写的 | `src/db/repository/*` | 仓库当前把数据库访问封装在 repo 层 |
| 文件内容怎么落盘 / 上 S3 | `src/storage/*` | 存储驱动抽象和具体实现都在这里 |
| WebDAV 请求为什么和 REST 不一样 | `src/webdav/*` | WebDAV 走单独的文件系统适配和锁系统 |
| 前端页面和调用关系 | `frontend-panel/src/*`、`frontend-panel/src/services/*` | 页面逻辑和 API 调用都在前端子项目里 |
| 表结构或字段怎么演进 | `migration/`、`src/entities/*` | 迁移与实体定义必须一起看 |

如果你要追一个具体功能，通常最省时间的方式是：

1. 先从 `src/api/routes/xxx.rs` 找入口
2. 再跳到对应 `src/services/xxx_service.rs`
3. 最后看 repo / storage / webdav 细节

## 系统边界

当前仓库同时提供五类入口：

- REST API：`/api/v1/*`
- 健康检查：`/health*`
- 短期内容直链：`/d/{token}/{filename}` 与 `/pv/{token}/{filename}`
- WebDAV：默认挂载在 `/webdav`
- 前端页面：管理面板与公开分享页由后端直接服务

另有三类只在特定构建里提供的辅助入口：

- `/swagger-ui` 与 `/api-docs/openapi.json`：仅 `debug_assertions + openapi feature` 构建时注册
- `/health/memory`：仅 `debug_assertions + openapi feature` 构建时注册
- `/health/metrics`：仅启用 `metrics` feature 时注册

## 一个请求如何流转

### 普通 REST 请求

一个典型 REST 请求大致会经历下面这些步骤：

1. 进入 `actix-web` 应用，由 `src/api/mod.rs` 注册出来的 route 命中具体 handler
2. 经过全局中间件，例如压缩、Request ID、CORS
3. 如果是受保护接口，再经过路由级的 JWT 鉴权和限流
4. `src/api/routes/*` 里的 handler 解析参数，尽量只做很薄的一层 HTTP 适配
5. `src/services/*` 执行业务规则，例如权限检查、锁检查、配额检查、版本处理、分享范围校验
   如果是团队工作空间，还会额外做团队成员身份和团队配额 / 策略组校验
6. `src/db/repository/*` 负责数据库访问；如果涉及文件内容，再进入 `src/storage/*`
7. route 层把结果包成统一 JSON 响应，或者直接返回流式内容

例外也要记住：

- 下载、缩略图、分享下载、WebDAV 响应、Prometheus 指标不会走统一 JSON 包装
- 直接下载链接 `/d/{token}/{filename}` 与预览链接 `/pv/{token}/{filename}` 都不会走统一 JSON 包装
- `GET /api/v1/auth/events/storage` 返回的是 SSE `text/event-stream`，也不是普通 JSON
- 前端页面路由是最后注册的兜底路由，所以 API / WebDAV 必须先于它注册

### WebDAV 请求

WebDAV 不是走 `src/api/routes/*`，而是这样进入系统：

1. 命中 `src/webdav/mod.rs` 注册的挂载前缀
2. 检查运行时开关 `webdav_enabled`
3. 做 Basic 或 Bearer 认证
4. 为本次请求构造带用户上下文的 `AsterDavFs`
5. 为本次请求构造数据库锁系统
6. 交给 `dav-server` 处理标准 WebDAV 方法

所以如果你在排查 WebDAV 行为，不要先去 REST route 里找。

## 分层结构

```text
┌────────────────────────────────────┐
│ 接入层                                                      │
│  - React 管理面板 / 公开分享页                                │
│  - REST API (actix-web)                                    │
│  - WebDAV / DeltaV 补充实现 (dav-server + 自定义拦截)         │
├────────────────────────────────────┤
│ 应用层                                                      │
│  - 路由、OpenAPI、统一响应、错误码                             │
│  - JWT / Cookie 认证、Request ID、CORS、限流                 │
├────────────────────────────────────┤
│ 业务层                                                      │
│  - auth / team / file / folder / upload / share / trash    │
│  - version / lock / property / policy / config / audit     │
│  - workspace_storage                                        │
│  - webdav_account / webdav_service / thumbnail             │
├────────────────────────────────────┤
│ 基础设施层                                                   │
│  - SeaORM + migration                                      │
│  - StorageDriver(Local / S3)                               │
│  - CacheBackend(Memory / Redis / Noop)                     │
├────────────────────────────────────┤
│ 数据层                                                      │
│  - users / teams / team_members / folders / files          │
│  - file_blobs / file_versions / shares / upload_sessions   │
│  - upload_session_parts / user_profiles / mail_outbox      │
│  - webdav_accounts / entity_properties / resource_locks    │
│  - contact_verification_tokens                             │
│  - storage_policies / storage_policy_groups / system_config│
└────────────────────────────────────┘
```

这个分层在仓库里的一个实用判断标准是：

- route 层处理 HTTP
- service 层处理业务语义
- repo 层处理数据库查询
- storage 层处理二进制内容

如果你发现某个复杂业务判断写在 route 层，通常就是代码气味。

## 改动应该落在哪一层

新开发者最容易踩的坑，是把代码改在“不该承载这类语义”的地方。可以按这个规则判断：

| 你要改的东西 | 优先落点 |
| --- | --- |
| 新增 REST 接口、调整请求体 / 响应体 | `src/api/routes/*` |
| 权限、配额、锁、回收站、版本、分享范围等业务规则 | `src/services/*` |
| 新增查询、分页、过滤条件 | `src/db/repository/*` |
| 本地 / S3 读写行为、预签名 URL、对象路径规则 | `src/storage/*` |
| WebDAV 协议方法、锁、路径解析、DeltaV | `src/webdav/*` |
| 表字段、索引、默认值 | `migration/` + `src/entities/*` |
| 页面交互、状态管理、前端 API 调用 | `frontend-panel/src/*` |

## 关键模块

| 模块 | 当前职责 |
| --- | --- |
| `src/main.rs` | 进程入口、HTTP 服务器、全局中间件、后台任务启动、优雅退出 |
| `src/runtime/startup.rs` | 数据库连接、迁移、默认策略、默认策略组、默认运行时配置、缓存、缩略图 worker |
| `src/api/` | REST 路由、OpenAPI、统一响应、认证与中间件 |
| `src/services/` | 业务规则集中层，处理权限、团队成员关系、配额、去重、版本、回收站等语义 |
| `src/db/repository/` | 面向实体的数据库访问封装 |
| `src/storage/` | 存储抽象，封装本地与 S3 兼容后端 |
| `src/webdav/` | WebDAV 文件系统、认证、数据库锁、DeltaV 最小子集 |
| `src/cli.rs` | CLI 根入口；当前子命令包括 `serve`、`doctor`、`config`、`database-migrate` |
| `frontend-panel/` | React 19 + Vite 前端，打包产物由后端服务 |
| `migration/` | SeaORM 迁移，启动时自动执行 |
| `tests/` | 集成测试，覆盖 API 契约和关键行为 |

## 启动与配置

### 启动流程

当前启动顺序大致是：

1. 安装自定义 panic hook
2. 加载 `.env`
3. 加载 `data/config.toml`；若不存在则在当前工作目录下的 `data/` 自动生成
4. 初始化日志
5. 连接数据库并执行全部迁移
6. 若系统里还没有任何存储策略，自动创建默认本地策略 `Local Default`，目录为 `data/uploads`
7. 写入认证 cookie 运行时引导值：根据静态配置里的 `bootstrap_insecure_cookies` 初始化 `auth_cookie_secure`
8. 确保存储策略组已种好；如果缺默认组会自动补，历史用户缺 `policy_group_id` 时也会回填到默认组
9. 确保默认运行时配置存在
10. 初始化运行时快照、策略快照、驱动注册中心与缓存后端
11. 启动缩略图后台 worker
12. 注册 REST、健康检查、WebDAV 与前端路由
13. 启动后台周期任务（周期由运行时配置驱动；下面是默认值）：
    - 每 5 秒：派发到期邮件 outbox
    - 每 5 秒：派发可执行的 `background_task`
    - 每小时：清理过期上传 session
    - 每小时：清理过期的已完成上传 session
    - 每小时：清理过期回收站条目
    - 每小时：清理超过保留期的已归档团队
    - 每小时：清理过期资源锁
    - 每小时：清理过期审计日志
    - 每小时：清理已过期任务记录及其临时产物
    - 每小时：清理已过期 WOPI session
    - 每 6 小时：全表校正 Blob 引用计数并清理孤儿 Blob，也会重试清理那些 `ref_count = 0` 但对象上次没删掉的 Blob

### 配置分层

新开发者需要先分清这两类配置：

#### 静态配置

静态配置来自：

- `data/config.toml`
- 环境变量 `ASTER__...`

它们主要控制：

- 监听地址和端口
- 数据库连接
- WebDAV 挂载前缀
- 缓存后端
- 日志

这类配置在进程启动前就要确定。

#### 运行时配置

运行时配置在数据库 `system_config` 表里，主要通过管理员接口管理。
定义单一数据源在 `src/config/definitions.rs`，接口上的 schema 视图来自这里。

它们覆盖的范围比下面这份示例列表更广，当前除了存储 / WOPI / CORS，还包含注册开关、头像上传限制、邮件 SMTP 与模板、任务保留期、分页上限等键。下面只列一批最常见项：

- `webdav_enabled`
- `trash_retention_days`
- `team_archive_retention_days`
- `max_versions_per_file`
- `audit_log_enabled`
- `audit_log_retention_days`
- `default_storage_quota`
- `public_site_url`
- `auth_cookie_secure`
- `cors_enabled`
- `cors_allowed_origins`
- `cors_allow_credentials`
- `cors_max_age_secs`
- `gravatar_base_url`
- `frontend_preview_apps_json`
- `wopi_access_token_ttl_secs`
- `wopi_lock_ttl_secs`
- `wopi_discovery_cache_ttl_secs`
- `mail_outbox_dispatch_interval_secs`
- `background_task_dispatch_interval_secs`
- `maintenance_cleanup_interval_secs`
- `blob_reconcile_interval_secs`

这类配置更接近业务开关，而不是基础设施开关。

## 核心数据模型

### 文件与 Blob 分离

- `files` 是用户可见的文件记录
- `file_blobs` 是实际内容；只有 local 显式开启 `content_dedup` 时，上传路径才会按 `sha256 + policy_id` 去重；其余路径都会创建独立 Blob
- 多个 `files` 与 `file_versions` 可以引用同一个 Blob

这让系统能同时支持：

- 内容去重
- 低成本复制
- 历史版本
- 缩略图按 Blob 复用

如果你第一次改文件相关逻辑，先记住一句话：
“用户看到的是 `files`，真正的内容复用靠的是 `file_blobs`。”

### 存储策略

策略保存在数据库，而非 `data/config.toml`。解析顺序如下：

```text
文件夹 policy_id -> 工作空间绑定的策略组规则
```

更具体一点：

- 个人空间：没有目录覆盖时，按 `users.policy_group_id` 命中的策略组规则选策略
- 团队空间：没有目录覆盖时，按 `teams.policy_group_id` 命中的策略组规则选策略
- 系统默认策略组主要用于新用户 / 新团队初始化和补种，不是每次上传时的直接兜底分支

策略决定：

- 驱动类型：`local` 或 `s3`
- 根目录或对象前缀
- 单文件大小限制
- 分片大小 `chunk_size`
- 是否通过 `options` 启用 local `content_dedup` 或选择 S3 上传方式

这意味着：

- “文件存哪”是业务数据，不是部署时写死的配置
- 同一个系统里可以同时存在多套存储后端

### 个人空间与团队空间

- 个人空间和团队空间共用同一套 `file_service` / `folder_service` / `upload_service` / `share_service` / `trash_service`
- route 层通过 `WorkspaceStorageScope::{Personal, Team}` 指明当前操作落在哪个工作空间
- 团队空间额外多了三层约束：
  - 必须是团队成员
  - 配额按团队统计，不再按个人统计
  - 默认存储策略来自团队绑定的 `policy_group_id`

### 回收站与软删除

- 普通删除会写 `deleted_at`，进入回收站
- 文件夹删除会递归标记子文件和子文件夹
- 恢复时若原父目录已不存在，会自动恢复到根目录
- 物理删除由显式 purge 或后台保留期清理触发
- purge 事务内先删除 `files` / `file_versions` 等业务记录，并递减 Blob 引用计数
- 真实对象和缩略图的清理由事务后执行，只有对象确认已经不存在时，才会删除 `file_blobs` 元数据
- 如果对象删除失败，`file_blobs` 会保留，通常表现为 `ref_count = 0` 的待清理 Blob，避免数据库先于存储“失忆”
- 真正执行对象删除前，会先用一次数据库级 CAS 抢占清理权，短暂把 `ref_count` 置为 `-1`；如果对象删除或校验失败，再恢复回 `0`，留给后续 maintenance 重试

所以删除相关需求要先分清：

- 你要的是“软删除”
- 还是“从回收站恢复”
- 还是“物理清除并回收 Blob / 配额”

### 版本与锁

- 历史版本保存在 `file_versions`
- 覆盖写入时，旧 Blob 会转入历史版本
- `resource_locks` 是锁的事实来源
- `files.is_locked` / `folders.is_locked` 只是同步出来的快速状态位

也就是说，`is_locked` 更像缓存状态位，不是最终真相。

### 分享

- 分享可指向文件或文件夹，也可以来自个人空间或团队空间
- 支持密码、过期时间、下载次数限制
- 公开 API 位于 `/api/v1/s/{token}/*`
- 预览直链最终落在 `/pv/{token}/{filename}`，常见入口是 `/api/v1/files/{id}/preview-link` 与 `/api/v1/s/{token}/preview-link`
- 公开页面路由为 `/s/:token`
- 目录分享现在已经支持继续浏览子目录

### WebDAV 专用账号

- Basic Auth 使用独立表 `webdav_accounts`
- 每个账号可以限制到某个根文件夹
- Bearer JWT 也可直接访问 WebDAV，但不受 `root_folder_id` 限制

## 上传链路

新开发者最容易把三类事情混在一起：

- 新建空文件：`POST /api/v1/files/new`
- 上传新文件：`/api/v1/files/upload*`
- 覆盖现有文件内容：`PUT /api/v1/files/{id}/content`

它们的语义不一样，代码入口也不一样。

### REST 上传

上传协商由 `POST /api/v1/files/upload/init` 返回四种模式：

- `direct`：客户端回退到普通 `multipart/form-data` 直传
- `chunked`：客户端按 `chunk_size` 上传分片，再调用 `complete`
- `presigned`：仅 S3 策略可用，客户端先把文件 `PUT` 到 `presigned_url`，再调用 `complete`
- `presigned_multipart`：仅 S3 策略可用，客户端先为每个 part 申请 URL，再调用 `complete`

这些模式最终都会进入统一的存储逻辑：

1. 解析生效存储策略
2. 校验大小限制与用户配额
3. 只有 local 显式开启 `content_dedup` 时，`direct` / `chunked` / 覆盖写入 / 空文件这些路径才会计算 SHA-256
4. 只有拿到 SHA-256 的本地上传路径才会按 `hash + policy_id` 做 Blob 去重；local 默认关闭 `content_dedup`
5. 所有 S3 路径（`relay_stream` / `presigned` / `presigned_multipart`）都不会做 Blob 去重
6. 创建文件记录
7. 更新用户已用空间

如果你在追上传问题，优先看：

- `src/api/routes/files.rs`
- `src/services/upload_service.rs`
- `src/services/file_service.rs`
- `src/storage/*`

### 覆盖写入与历史版本

当前文件被覆盖时：

1. 检查锁状态
2. 当前 Blob 进入 `file_versions`
3. 文件切换到新 Blob
4. 超过 `max_versions_per_file` 的最老版本会被自动清理

REST 普通上传不会按文件名覆盖已有文件；WebDAV `PUT` 才是当前最主要的覆盖入口。

## WebDAV 与 DeltaV

仓库里的 WebDAV 支持分成两层理解：

- 标准 WebDAV：交给 `dav-server`
- DeltaV 最小补充：仓库自己拦截实现

当前额外补的 DeltaV 子集是：

- `REPORT` 的 `DAV:version-tree`
- `VERSION-CONTROL`
- `OPTIONS` 里追加 `DAV: version-control`

这些功能直接复用现有的 `file_versions` 表。

当前不是完整 DeltaV 服务器，只是最小可用子集。

## 前端交付方式

- 构建期：`build.rs` 会检查 `frontend-panel/dist`
- 若前端未构建，构建脚本将生成回退页，提醒先执行前端构建
- 运行期：后端优先读取 `./frontend-panel/dist`
- 若磁盘上没有对应文件，再回退到嵌入资源
- 匿名页启动时会先请求 `/api/v1/public/branding`，因此品牌文案和公开站点地址不需要硬编码在前端构建产物里

因此：

- 生产镜像通常走嵌入资源
- 本地调试时可以直接用磁盘上的前端产物覆盖嵌入页面

## 可观测性与辅助入口

- `/health`：存活检查
- `/health/ready`：就绪检查，包含数据库 `ping`
- `/health/memory`：堆统计，仅 `debug_assertions + openapi feature` 构建可用
- `/health/metrics`：Prometheus 指标，仅 `metrics` feature 构建可用
- `/swagger-ui`：仅 `debug_assertions + openapi feature` 构建注册
- `/api-docs/openapi.json`：仅 `debug_assertions + openapi feature` 构建注册

如果你在本地调 API，带 `openapi` feature 的 debug 构建会更方便。

## 常见开发任务看哪里

- 登录、注册、JWT、Cookie：`src/api/routes/auth.rs`、`src/services/auth_service.rs`
- 团队与团队工作空间：`src/api/routes/teams.rs`、`src/api/routes/team_*`、`src/services/team_service.rs`、`src/services/workspace_storage_service.rs`
- 用户资料与头像：`src/services/profile_service.rs`
- 文件列表、重命名、移动、复制：`src/services/file_service.rs`、`src/services/folder_service.rs`
- 回收站：`src/services/trash_service.rs`
- 分享与公开页：`src/services/share_service.rs`、`src/api/routes/share_public.rs`
- 实时文件树刷新与 SSE：`src/api/routes/auth.rs`、`src/services/storage_change_service.rs`、`frontend-panel/src/hooks/useStorageChangeEvents.ts`
- 搜索：`src/services/search_service.rs`、`src/db/repository/search_repo.rs`
- 存储策略与 S3：`src/services/policy_service.rs`、`src/storage/s3.rs`
- WebDAV：`src/webdav/*`
- 前端 API 封装：`frontend-panel/src/services/*`

## 当前已知问题

- `allowed_types` 字段已在策略模型中落库，但当前 REST API 没有管理它，也没有在上传链路里实际执行类型限制
