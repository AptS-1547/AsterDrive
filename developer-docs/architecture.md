# AsterDrive 架构概览

本文描述的是当前仓库已经落地的实现，不是早期设计草图。

## 系统边界

当前仓库同时提供四类入口：

- REST API：`/api/v1/*`
- 健康检查：`/health*`
- WebDAV：默认挂载在 `/webdav`
- 前端页面：管理面板与公开分享页由后端直接服务

另有两个只在特定构建里提供的辅助入口：

- `/swagger-ui` 与 `/api-docs/openapi.json`：仅启用 `debug` 构建时注册
- `/health/metrics`：仅启用 `metrics` feature 时注册

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
│  - auth / file / folder / upload / share / trash           │
│  - version / lock / property / policy / config             │
│  - webdav_account / webdav_service / thumbnail             │
├────────────────────────────────────┤
│ 基础设施层                                                   │
│  - SeaORM + migration                                      │
│  - StorageDriver(Local / S3)                               │
│  - CacheBackend(Memory / Redis / Noop)                     │
├────────────────────────────────────┤
│ 数据层                                                      │
│  - users / folders / files / file_blobs                    │
│  - file_versions / shares / upload_sessions                │
│  - webdav_accounts / entity_properties / resource_locks    │
│  - storage_policies / user_storage_policies / system_config│
└────────────────────────────────────┘
```

## 关键模块

| 模块 | 当前职责 |
| --- | --- |
| `src/main.rs` | 启动顺序、HTTP 服务器、后台清理任务、优雅退出 |
| `src/api/` | REST 路由、OpenAPI、统一响应、认证与中间件 |
| `src/services/` | 业务规则集中层，处理权限、配额、去重、版本、回收站 |
| `src/storage/` | 存储抽象，封装本地与 S3 兼容后端 |
| `src/webdav/` | WebDAV 文件系统、认证、数据库锁、DeltaV 补充实现 |
| `src/db/repository/` | 面向实体的数据库访问封装 |
| `frontend-panel/` | React 19 + Vite 前端，打包产物嵌入后端 |
| `migration/` | SeaORM 迁移，启动时自动执行 |

## 启动流程

1. 安装自定义 panic hook。
2. 加载 `config.toml`；若不存在则在当前工作目录自动生成。
3. 初始化日志。
4. 连接数据库并执行全部迁移。
5. 若系统里还没有任何存储策略，自动创建默认本地策略 `Local Default`，目录为 `data/uploads`。
6. 初始化驱动注册中心与缓存后端。
7. 注册 REST、健康检查、WebDAV 与前端路由。
8. 启动每小时执行一次的后台任务：
   - 清理过期上传 session
   - 清理过期回收站条目
   - 清理过期资源锁
   - 清理过期审计日志

## 核心数据模型

### 文件与 Blob 分离

- `files` 是用户可见的文件记录
- `file_blobs` 是实际内容，按 `sha256 + policy_id` 去重
- 多个 `files` 与 `file_versions` 可以引用同一个 Blob

这让系统能同时支持：

- 内容去重
- 低成本复制
- 历史版本
- 缩略图按 Blob 复用

### 存储策略

策略保存在数据库，而非 `config.toml`。解析顺序如下：

```text
文件夹 policy_id -> 用户默认策略 -> 系统默认策略
```

策略决定：

- 驱动类型：`local` 或 `s3`
- 根目录或对象前缀
- 单文件大小限制
- 分片大小 `chunk_size`
- 是否通过 `options` 启用 S3 预签名上传

### 回收站与软删除

- 普通删除会写 `deleted_at`，进入回收站
- 文件夹删除会递归标记子文件和子文件夹
- 恢复时若原父目录已不存在，会自动恢复到根目录
- 物理删除由显式 purge 或后台保留期清理触发

### 版本与锁

- 历史版本保存在 `file_versions`
- 覆盖写入时，旧 Blob 会转入历史版本
- `resource_locks` 是锁的事实来源
- `files.is_locked` / `folders.is_locked` 只是同步出来的快速状态位

### 分享

- 分享可指向文件或文件夹
- 支持密码、过期时间、下载次数限制
- 公开 API 位于 `/api/v1/s/{token}/*`
- 公开页面路由为 `/s/:token`

### WebDAV 专用账号

- Basic Auth 使用独立表 `webdav_accounts`
- 每个账号可以限制到某个根文件夹
- Bearer JWT 也可直接访问 WebDAV，但不受 `root_folder_id` 限制

## 上传链路

### REST 上传

上传协商由 `POST /api/v1/files/upload/init` 返回三种模式：

- `direct`：客户端回退到普通 `multipart/form-data` 直传
- `chunked`：客户端按 `chunk_size` 上传分片，再调用 `complete`
- `presigned`：仅 S3 策略可用，客户端先把文件 `PUT` 到 `presigned_url`，再调用 `complete`

三种模式最终都会进入统一的存储逻辑：

1. 解析生效存储策略
2. 校验大小限制与用户配额
3. 计算 SHA-256
4. 按 `hash + policy_id` 做 Blob 去重
5. 创建或覆盖文件记录
6. 更新用户已用空间

### 覆盖写入与历史版本

当前文件被覆盖时：

1. 检查锁状态
2. 当前 Blob 进入 `file_versions`
3. 文件切换到新 Blob
4. 超过 `max_versions_per_file` 的最老版本会被自动清理

REST 普通上传不会按文件名覆盖已有文件；WebDAV `PUT` 才是当前最主要的覆盖入口。

## WebDAV 与 DeltaV

WebDAV 请求处理流程：

1. 读取运行时开关 `webdav_enabled`
2. 身份认证（ Basic 或 Bearer ）
3. 为本次请求构造带用户上下文的 `AsterDavFs`
4. 为本次请求构造数据库锁系统
5. 提交至 `dav-server` 处理标准 WebDAV 方法

因为 `dav-server` 本身不支持 RFC3253 DeltaV，仓库额外拦截并实现了最小子集：

- `REPORT` 的 `DAV:version-tree`
- `VERSION-CONTROL`
- `OPTIONS` 里追加 `DAV: version-control`

这些功能会直接复用现有的 `file_versions` 表。

## 前端交付方式

- 构建期：`build.rs` 会检查 `frontend-panel/dist`
- 若前端未构建，构建脚本将生成回退页，提醒先执行前端构建
- 运行期：后端优先读取 `./frontend-panel/dist`，找不到时再回退到嵌入资源

因此：

- 生产镜像通常走嵌入资源
- 本地调试时可以直接用磁盘上的前端产物覆盖嵌入页面

## 可观测性

- `/health`：存活检查
- `/health/ready`：就绪检查，包含数据库 `ping`
- `/health/memory`：堆统计
- `/health/metrics`：Prometheus 指标，仅 `metrics` feature 构建可用
- `/swagger-ui`：仅 `debug` 构建注册

## 已知问题

- `cli` feature 仍在 `Cargo.toml` 中，但仓库当前没有可直接使用的 CLI 子命令入口
- 文件夹与文件的 `PATCH` 请求目前无法通过传 `null` 的方式移动回根目录
- 文件夹复制不会保留源目录上的 `policy_id`
- `allowed_types` 字段已在策略模型中落库，但当前 REST API 没有管理它，也没有在上传链路里实际执行类型限制
