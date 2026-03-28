# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [v0.0.1-alpha.10] - 2026-03-29

### Release Highlights

- 新增**用户个人资料系统**：支持自定义展示名称、头像上传、Gravatar 及来源切换，并支持自定义 Gravatar 镜像地址
- 文件列表引入**虚拟滚动**，网格视图和表格视图均使用 `@tanstack/react-virtual`，大数据量下渲染性能显著提升
- 新增**视频预览增强**：集成 Artplayer 播放器，支持动态宽高比计算与自定义视频浏览器
- 代码编辑器从 `@monaco-editor/react` 迁移至原生 `monaco-editor`，按需懒加载语言支持，构建产物体积大幅优化
- 设置页拆分为**个人资料**与**界面偏好**两个独立路由分区，导航更清晰
- 错误页面重构：区分生产/开发环境，生产环境隐藏调试信息
- 图标库从 `@devicon/react` 迁移至 `react-devicons`，统一使用 original 变体
- 新增路由过渡动画（View Transitions API），页面切换体验更流畅
- 禁止删除内置系统存储策略，新增 S3 参数变更检测与强制保存确认

### Added

- **用户个人资料系统**
  - 新增 `user_profiles` 数据库表及两次 migration
  - `profile_service` 完整实现：展示名称编辑（最大 64 字符）、头像上传（自动裁剪为正方形 + WebP 编码，512px/1024px 两档）、Gravatar 及来源切换
  - 新增 API 端点：`PATCH /auth/profile`、`POST /auth/profile/avatar/upload`、`PUT /auth/profile/avatar/source`、`GET /auth/profile/avatar/{size}`
  - 前端 `UserAvatarImage` 组件，支持 sm/md/lg/xl 四种尺寸
  - 新增 `ProfileSettingsView` 个人资料设置页：展示名称编辑、头像管理、只读用户名/邮箱展示
  - 新增 `gravatar_base_url` 运行时配置，支持自定义 Gravatar 镜像（如 Cravatar）
- **文件列表虚拟滚动**
  - `FileGrid` 和 `FileTable` 引入 `@tanstack/react-virtual` 虚拟滚动
  - 网格视图响应式列数（2-6 列），overscan 优化滚动流畅度
- **视频预览增强**
  - 新增 `VideoPreview` 组件，基于 Artplayer 播放器，支持动态宽高比计算
  - 新增 `CustomVideoBrowserPreview`，支持外部视频源的自定义浏览器
  - 视频浏览器配置模块 `video-browser-config.ts`
- **界面设置页**
  - 新增 `InterfaceSettingsView`：主题模式、色板、语言、视图模式统一管理
- **路由过渡动画**
  - 导航链接集成 View Transitions API，页面切换更流畅
- **运行时配置模块**
  - 新增 `frontend-panel/src/config/runtime.ts`，统一管理环境变量与开发模式标识
- **策略保护与变更检测**
  - 内置系统存储策略（ID=1）禁止删除
  - Admin 策略编辑新增 S3 参数变更检测与强制保存确认对话框

### Changed

- **Monaco 编辑器迁移**
  - 从 `@monaco-editor/react` 迁移至原生 `monaco-editor`
  - 新增 `monaco-environment.ts` 按需懒加载语言支持
  - `MonacoCodeEditor` 替代旧的编辑器组件
- **设置页路由重构**
  - 设置页拆分为 `/settings/profile` 和 `/settings/interface` 两个路由分区
  - 原 `ThemeSwitcher` / `LanguageSwitcher` 独立组件移入设置页内
- **错误页面重构**
  - 全面重写 `ErrorPage`，卡片式布局 + 状态码徽章 + 恢复建议
  - 生产环境隐藏堆栈跟踪等调试信息
- **动画性能优化**
  - 文件卡片/表格过渡动画从 300ms 缩短至 150ms，移除 scale 变换
  - Tooltip 动画时长调整为 100ms
- **图标库迁移**
  - 从 `@devicon/react` 迁移至 `react-devicons`
  - 语言图标统一使用 original 变体
- **Vite 构建拆分优化**
  - `manualChunks` 策略增强：vendor-react / vendor-router / vendor-i18n / vendor-react-icons / vendor-devicons 等
  - Base UI 拆分为 vendor-ui-forms / vendor-ui-overlays / vendor-ui-controls
  - 预览专属 chunks：preview-data / preview-xml
  - PWA workbox 排除未使用的 Monaco worker 文件
- **分享页面体验优化**
  - 新增所有者信息展示（名称/邮箱）与拖拽预览支持
  - 文件分享卡片新增预览按钮
- **文件预览统一加载状态**
  - 新增 `PreviewLoadingState` 组件，统一各预览器的加载态展示
  - 文件预览对话框优化高度自适应与视频尺寸计算
- **HeaderControls 增强**
  - 顶栏控件集成用户头像与展示名称

### Fixed

- 修复存储策略零值字段处理及用户列表头像显示问题
- 修复策略连接测试逻辑
- 修复网络错误后无法重新发起身份校验请求的问题
- 修复 Vue 图标显示及配额单元格样式问题

### Breaking Changes

- **API**：`GET /api/v1/auth/me` 响应体新增 `profile` 字段，含 `display_name`、`avatar`（source / url_512 / url_1024 / version）
- **API**：Admin 用户相关端点响应体新增用户资料信息
- **Frontend**：设置页路由从 `/settings` 拆分为 `/settings/profile` 和 `/settings/interface`
- **Frontend**：`ThemeSwitcher` / `LanguageSwitcher` 独立组件已移除，功能整合至 `InterfaceSettingsView`

---

**统计数据**：
- 147 files changed, 7,340 insertions(+), 1,484 deletions(-)
- 21 commits

## [v0.0.1-alpha.9] - 2026-03-28

### Release Highlights

- 新增**服务端用户偏好持久化**（主题、色板、视图模式、排序、语言），支持多设备自动同步
- 新增**"我的分享"页面**，支持分享状态追踪（active / expired / exhausted / deleted）与分页管理
- 文件和文件夹列表新增**分享与锁定状态标识**，一眼区分资源状态
- 集成 **devicon 语言图标**，代码预览与文件类型图标全面升级
- **拖放交互增强**：文件夹树支持跨组件拖拽、防止文件夹拖入自身或后代目录
- **i18n 命名空间拆分**：common → core / errors / validation / offline + 按需加载 share / settings / webdav
- **大规模前后端测试覆盖补充**，新增 4000+ 行单元测试 + 集成测试

### Added

- **服务端用户偏好持久化**
  - 新增 `PATCH /api/v1/auth/preferences` 端点
  - 支持主题模式、色板、视图模式、排序、语言等偏好
  - 前端 debounce 同步，多设备登录自动同步
  - 数据库 migration: users.config JSON 字段
- **"我的分享"页面**
  - 新增 `/my-shares` 路由，支持分享列表浏览与管理
  - 后端 `ShareStatus` 枚举（active / expired / exhausted / deleted）
  - `MyShareInfo` DTO 含资源名称、状态、剩余下载次数等
- **文件/文件夹状态标识**
  - 列表和网格视图新增分享状态与锁定状态图标
  - `FileItemStatusIndicators` 组件
- **devicon 语言图标集成**
  - 新增 `language-icon.tsx` 组件，基于 devicon 图标库
  - 代码预览文件类型图标升级
  - 新增 CMap 提取脚本，PDF 中文显示支持
- **拖放增强**
  - 文件夹树支持拖拽到文件浏览器
  - 防止文件夹拖入自身或后代目录
  - 拖放逻辑提取到 `lib/dragDrop.ts` 公共模块
- **代码预览 minimap**
  - TextCodePreview 启用 minimap 功能
- **分享查找索引**
  - migration 新增 share 表查询索引，优化 token 和 resource 查询性能

### Changed

- **审计动作类型安全**
  - 审计日志从字符串字面量重构为 `AuditAction` 枚举
- **路由层逻辑下沉**
  - auth、share_public、files、folders、batch 等路由层业务逻辑下沉至 service 层
- **i18n 命名空间拆分**
  - `common` 拆分为 `core`、`errors`、`validation`、`offline`
  - 新增 `settings`、`share`、`webdav` 独立命名空间
  - 初始加载与延迟加载分层优化
- **错误日志分级**
  - 5xx → `tracing::error`，4xx → `tracing::warn`
  - 静默忽略的错误统一替换为 warn 日志
- **前端公共模块提取**
  - `ToolbarBar` 通用工具栏组件
  - `AdminTableList` 通用管理后台列表组件
  - 多个 hooks / utils 去重
- **admin 用户更新优化**
  - 合并为单次批量修改（role + status + quota）
  - 补充审计日志
- **分享页面布局重构**
  - 提取 `ShareTopBar`、`ToolbarBar` 通用组件

### Fixed

- 修复分享下载链接使用相对路径导致下载失败的问题
- 修复复制操作中 null 目标路径未正确解析为根目录的问题
- 修复 fire-and-forget 操作中静默忽略的错误（改为 warn 日志）
- 修复前端非空断言导致的潜在运行时错误
- 修复布局滚动区域样式问题
- 消除多处无障碍访问问题

### Breaking Changes

- **API**：`GET /api/v1/shares` 响应体从 `share::Model` 改为 `MyShareInfo` 分页对象，包含 `status` 枚举、`resource_name`、`remaining_downloads` 等新字段
- **API**：`GET /api/v1/auth/me` 响应体从 `UserInfo` 改为 `MeResponse`，新增 `preferences` 字段
- **API**：新增 `PATCH /api/v1/auth/preferences` 端点
- **Frontend**：i18n 命名空间 `common` 已拆分为 `core` / `errors` / `validation` / `offline`，自定义前端需同步更新翻译引用

---

**统计数据**：
- 291 files changed, 28,047 insertions(+), 2,216 deletions(-)
- 24 commits

## [v0.0.1-alpha.8] - 2026-03-27

### Release Highlights

- 管理后台新增**管理员创建用户**能力，适合自托管场景下集中管理账号
- 多个管理接口与用户侧列表统一为 **offset 分页结构**，大数据量场景下体验更稳、前后端类型更一致
- 文件拖拽体验升级：新增**自定义拖拽预览**，文件夹树支持**拖拽悬停自动展开**
- PWA 启动体验优化：新增**离线启动降级页**，并在登录后预热常用路由资源
- 分享访问边界与 WebDAV 账号管理补强，公开访问、路径展示与权限校验更可靠

### Added

- **管理员创建用户**
  - 后端新增 `POST /api/v1/admin/users`
  - 管理后台支持直接创建用户，无需依赖用户自行注册
- **管理后台用户详情面板**
  - 用户详情查看与编辑体验升级
  - 角色、状态、配额等信息改为统一保存交互
- **拖拽体验增强**
  - 文件卡片与列表行新增自定义拖拽预览
  - 文件夹树支持拖拽悬停自动展开，移动到深层目录更顺手
- **PWA 启动增强**
  - 新增离线启动降级页面
  - 登录后预热常用路由资源，改善安装态和弱网场景体验
- **统一分页基础结构**
  - 新增通用 `LimitOffsetQuery` / `OffsetPage<T>` 分页结构
  - 管理接口与部分用户接口统一接入 offset 分页

### Changed

- **管理后台列表统一分页**
  - 用户、策略、分享、配置、锁、审计日志、用户策略列表统一切换到 offset 分页返回
- **用户侧部分列表统一分页**
  - `/api/v1/shares` 与 `/api/v1/webdav-accounts` 改为分页对象返回
- **管理后台布局重构**
  - 顶栏、页面容器、说明文案与控件尺寸做了一轮统一整理
- **WebDAV 账号路径构建优化**
  - 通过批量路径构建减少重复查询，路径展示更稳定
- **依赖与构建配置更新**
  - 升级部分前后端依赖
  - 新增性能构建 profile，并适配新版 `sha2` Digest API

### Fixed

- 修复分享公开访问中的多个边界问题，包括过期分享、越界访问、已删除子文件 / 子目录访问等情况
- 修复重复活跃分享创建未被正确拦截的问题
- 修复 WebDAV 账号 root folder 校验与禁用账号测试相关边界问题
- 修复 PWA 离线启动时无缓存用户场景下的启动流程问题
- 补强审计日志、分享、WebDAV 相关测试覆盖与权限边界验证

### Breaking Changes

- **API**：多个列表接口的响应结构已从数组调整为分页对象：
  - `/api/v1/shares`
  - `/api/v1/webdav-accounts`
  - 多个 `/api/v1/admin/*` 列表接口
- 依赖旧数组响应格式的自定义前端、脚本或第三方客户端需要同步适配

---

**统计数据**：
- 87 files changed, 6,021 insertions(+), 1,783 deletions(-)
- 15 commits

## [v0.0.1-alpha.7] - 2026-03-26

### Release Highlights

- 文件列表新增多字段排序，并升级为基于 cursor 的分页，深目录和大文件夹浏览更顺手
- 前端接入 PWA，支持更新提示与离线登录态保持，弱网/断网场景体验更稳
- 文件夹树状态管理重构，引入按需加载与祖先路径恢复，目录导航性能明显改善
- 新增文件/文件夹详情信息对话框，快速查看大小、类型、时间、锁状态和子项数量
- 回收站批量恢复与批量清理链路重构，减少事务和 DB 往返，删除与清空操作更高效
- 上传面板引入虚拟滚动，预览错误态与重试入口统一，大量任务和异常场景下前端更稳定

### Added

- **文件列表排序与分页能力增强**
  - 文件列表支持按 `name` / `size` / `created_at` / `updated_at` / `type` 排序
  - 前端新增排序菜单，支持升序 / 降序切换
  - 文件列表分页升级为 cursor 模式，支持 `file_after_value` + `file_after_id`
- **PWA 支持**
  - 前端接入 `vite-plugin-pwa`
  - 支持 manifest、service worker 注册与新版本更新提示
- **离线登录态保持**
  - `authStore` 缓存用户信息，网络异常时保留现有登录态
- **文件/文件夹详情信息对话框**
  - 文件支持查看大小、MIME、创建/修改时间、锁状态、blob id
  - 文件夹支持查看创建/修改时间、锁状态、策略 id 与子项数量
- **文件夹祖先路径接口**
  - 新增 `/folders/{id}/ancestors`，用于恢复深层目录导航路径

### Changed

- **文件夹树状态管理重构**
  - 前端文件夹树改为按需加载，减少一次性加载整棵树的压力
  - 深层目录进入时可正确恢复祖先路径与树展开状态
- **回收站批量链路重构**
  - 批量恢复、批量清理与递归清理逻辑统一走批处理路径
  - 减少事务次数与数据库往返
- **上传面板性能优化**
  - 引入虚拟滚动，优化大量上传任务场景下的渲染性能
- **前端资源加载优化**
  - i18n 改为按需加载
  - Vite 构建拆分优化，配合 PWA 缓存策略改进加载体验

### Fixed

- 排序切换后文件列表状态不同步的问题，切换排序时会正确重置列表并重新加载
- 文件预览错误态不一致的问题，统一错误展示与重试入口
- 分享内容列表与主文件列表能力不一致的问题，补齐排序与 cursor 分页链路
- 缩略图生成重复入队与高负载下体验不稳定的问题，增加去重与重试优化
- 回收站批量恢复 / 清理过程中的部分边界问题，避免重复处理和漏处理

### Breaking Changes

- **API**：文件列表查询不再使用 `file_offset`，改为 cursor 分页参数 `file_after_value` + `file_after_id`
- **API**：文件列表相关接口新增 `sort_by` 与 `sort_order` 查询参数，旧调用方需要同步适配

---

**统计数据**：
- 91 files changed, 4,209 insertions(+), 1,477 deletions(-)
- 18 commits

## [v0.0.1-alpha.6] - 2026-03-25

### Release Highlights

- 文件列表、回收站、分享页面全面支持分页 + 前端无限滚动，告别一次加载全量数据
- 缩略图改为后台异步生成，接口返回 202 让前端轮询重试，解决大量文件上传后的内存峰值问题
- 回收站永久删除批量优化，N 个文件由 ~12N 次 DB 查询降至 ~10 次
- 新增剪贴板操作（Ctrl+C/X/V）与 F2 重命名快捷键
- 新增四档限流中间件（auth/public/api/write）、空文件创建接口、用户状态缓存

### Added

- **分页系统**
  - 后端新增 `FolderListQuery` 分页参数（`folder_limit/offset`、`file_limit/offset`），默认 folder_limit=200, file_limit=100
  - 文件夹列表、回收站列表、分享内容列表三个接口全面支持分页
  - 响应体新增 `folders_total` / `files_total` 字段
  - 前端 `fileStore` 新增 `loadMoreFiles` + IntersectionObserver 无限滚动
  - TrashPage、ShareViewPage 同步接入分页及无限滚动
  - 文件夹树与目标文件夹选择弹窗传入 `file_limit: 0` 仅加载文件夹
- **缩略图异步后台生成**
  - `thumbnail_service::get_or_enqueue()` — 缩略图不存在时入队后台生成，返回 202 + `Retry-After: 2`
  - `AppState.thumbnail_tx` 独立 tokio worker 顺序消费队列，HashSet 去重防止同一 blob 重复处理
  - WebDAV fs/file/handler 全链路透传 thumbnail channel
  - 前端 `useBlobUrl` 收到 202 自动按 `Retry-After` 间隔重试（最多 5 次）
- **限流中间件**
  - `RateLimitConfig` 四档限流（auth/public/api/write），默认关闭，支持按需启用
  - `AsterIpKeyExtractor` — 429 响应返回统一 JSON 格式并携带 `Retry-After` 头
  - 各路由通过 `Condition` 按 tier 挂载 Governor 限流中间件
- **空文件创建接口**
  - `POST /api/v1/files/new` 创建 0 字节空文件，支持 blob 去重与文件名冲突自动重命名
  - 前端 `CreateFileDialog` 组件，支持文件浏览器内直接创建空文件
- **剪贴板操作与重命名快捷键**
  - `fileStore` 新增 `clipboardCopy` / `clipboardCut` / `clipboardPaste` / `clearClipboard`
  - `useKeyboardShortcuts` 新增 Ctrl+C/X/V 剪贴板快捷键与 F2 重命名快捷键
  - FileGrid / FileTable 新增 `onRename` 回调
- **回收站批量操作 Repo 函数**
  - `file_repo::delete_many` / `delete_blobs` / `decrement_blob_ref_counts`
  - `folder_repo::delete_many` / `find_all_children` / `find_all_files_in_folder`
  - `property_repo::delete_all_for_entities`、`version_repo::delete_all_by_file_ids`

### Changed

- **回收站批量清理重构**
  - `file_service::batch_purge` — 单次事务处理所有 DB 操作，事务后并行物理清理
  - `webdav_service::recursive_purge_folder` 改为先递归收集再批量清理
  - `trash_service::purge_all` 优先批量处理顶层文件夹，再批量清理顶层散文件
- **用户状态缓存**
  - auth 中间件引入用户状态缓存（TTL=30s），减少每次请求查 DB
  - admin 禁用用户时主动失效缓存
- **前端组件**
  - `ScrollArea` 改为 `forwardRef`，ref 指向 Viewport 元素支持 IntersectionObserver
  - 前端空文件创建改为调用新接口，移除 multipart FormData 逻辑
- **代码格式化**
  - 统一 rustfmt 格式化全项目代码，拆分过长链式调用与函数参数

### Fixed

- 移除 `purge` 中对 `is_locked` 的检查，回收站内文件不应受锁限制
- 回收站列表改为 SQL 级顶层删除项过滤分页，移除内存 HashSet 过滤逻辑
- `recursive_purge_folder` 改用 `find_all_children`（不过滤 deleted_at），修复漏掉已软删除子目录的问题

---

**统计数据**：
- 72 files changed, 2,844 insertions(+), 318 deletions(-)
- 6 commits

## [v0.0.1-alpha.5] - 2026-03-25

### Release Highlights

- S3 上传流程大幅简化：去掉 SHA256 回读和 copy_object，直接以 `files/{uuid}` 作为最终存储路径，降低延迟和流量消耗
- 上传幂等重试：upload_session 记录 file_id，重复 complete 直接返回已有文件，新增 Assembling 中间状态（HTTP 202）防止前端轮询卡死
- 日志轮转：支持按天自动轮转 + 保留历史文件数量配置（`enable_rotation` / `max_backups`）
- 前端设置页和 WebDAV 账号页用 SettingsScaffold 组件重构，统一卡片式布局
- 前端类型统一从生成的 API schema 导出，消除手写重复定义
- 文件流式响应性能优化，减少内存占用

### Added

- **上传幂等重试**
  - upload_sessions 表新增 `file_id` 列（migration），完成后记录关联文件 ID
  - 重复 complete 请求：session 已完成 → 直接返回已有文件；正在处理 → 返回 HTTP 202（ErrorCode 3011）
  - assembly 失败自动标记 session 为 Failed，防止前端无限重试
  - `generate_upload_id()` 碰撞检测，最多重试 5 次
- **日志轮转**
  - `LoggingConfig` 新增 `enable_rotation`（默认 true）和 `max_backups`（默认 5）
  - 基于 tracing_appender rolling 按天轮转，自动清理超出数量的历史日志
  - 轮转失败自动 fallback 到 stdout 并输出警告
- **前端 SettingsScaffold 组件**
  - `SettingsPageIntro` / `SettingsSection` / `SettingsRow` / `SettingsIcon` 复用组件
  - 统一卡片式布局，支持 action slot 和自定义内容区

### Changed

- **S3 上传流程简化**
  - presigned / multipart 上传不再回读 S3 对象做 SHA256，改用 `s3-{upload_id}` 占位 hash
  - 不再 copy_object 到内容寻址路径，直接以 `files/{upload_id}` 为最终 key
  - 去除 S3 临时对象删除步骤（不再有临时→正式的两步操作）
- **前端页面重构**
  - SettingsPage 用 SettingsScaffold 重写，代码量大幅减少
  - WebdavAccountsPage 重构精简，统一布局风格
  - 前端类型统一从 `api.generated.ts` 导出，`types/api.ts` 仅做 re-export
  - searchService / fileService / uploadService 改用生成的类型定义
- **macOS 临时目录清理**
  - `cleanup_temp_dir` 增加重试机制（最多 3 次 + 50ms 间隔），处理 Spotlight 造成的 ENOTEMPTY
- **文件流式响应**
  - `file_service` 优化流式响应性能，减少内存占用

### Fixed

- 修正 PDF 预览头部信息区域缩进格式
- 修复目录上传工具函数的边界处理

---

**统计数据**：
- 24 files changed, 1,045 insertions(+), 950 deletions(-)
- 5 commits

## [v0.0.1-alpha.4] - 2026-03-25

### Release Highlights

- 支持 S3 分片直传（presigned_multipart）及断点续传，提升大文件上传性能和稳定性
- 重构回收站页面及功能，新增批量操作与拖拽删除功能
- 文件预览新增内嵌 PDF 预览，支持分页、缩放、旋转及下载
- 重构 WebDAV 账号管理页面，升级 UI 并完善国际化文案
- 优化文件夹树缓存与交互，提高初始加载和操作响应速度
- 设置页面改为响应式卡片布局，增强国际化支持
- 大幅重构用户文档站点组织，迁移 API 与架构文档至 developer-docs
- 多项安全加固，包括 Cookie Secure 标志、上传权限校验及并发更新防护
- 性能优化和 bug 修复，包括上传流程、文件树交互及前端状态管理  

### Added

- presigned_multipart 上传模式批量预取签名、上传和状态持久化
- 拖拽、快捷键、批量选择至回收站功能
- react-pdf集成，内置 PDF 预览窗口和工具栏
- 目录上传支持，前端拖拽/选择目录解析及后端相对路径递归创建
- 审计日志清理及多项后台任务panic-safe封装
- upload panel 进度条及分组显示  

### Changed

- 文档站重构，聚焦用户视角，优化导航和结构
- 文件浏览器视图初始加载性能优化
- 重写上传相关 hooks，移除冗余代码与无用接口
- 将 iframe sandbox 限制提升安全性，限制脚本执行

### Fixed

- 修复 token 刷新失败后前端清理登录状态问题
- 修正文件大小信息多处不一致与版本回归错误
- 修复重名文件自动后缀问题
- 修复上传状态互相覆盖与可能的并发冲突
- 修正回收站路径过滤及回收站详情与同步问题  

### Breaking Changes

- API /api/v1/auth/login 请求字段由 username 调整为 identifier


## [v0.0.1-alpha.3] - 2026-03-24

### Release Highlights

**预览、上传与认证体验全面升级！** 从文件预览、登录流程到上传任务面板，这一版把前后端体验一起往前拽了一大截。

- **认证流程重构** — 支持用户名 / 邮箱统一登录，并新增首次初始化管理员引导
- **统一文件预览系统** — 支持 Markdown、JSON、XML、CSV/TSV、媒体与代码预览
- **分享能力增强** — 公开文件可直接预览，文件夹分享支持下载其中的文件
- **上传体验升级** — 新增上传任务面板、并发上传、分片重试与状态追踪
- **版本恢复重构** — 回退时裁剪后续历史版本，并完善 blob 清理与回归测试
- **前端体验优化** — 登录页、文件浏览器、TopBar、提示通知与国际化整体打磨

### Added

- **认证与初始化流程**
  - 新增 `/api/v1/auth/check`，根据输入自动判断登录 / 注册 / 首次初始化路径
  - 新增 `/api/v1/auth/setup`，支持系统首次启动时创建管理员账号
  - 登录支持邮箱或用户名作为统一标识符
- **新文件预览体系**
  - 统一 `FilePreviewDialog` 作为预览入口
  - 新增 Markdown、JSON、XML、CSV/TSV、文本代码等多种预览器
  - 支持 Open With 模式切换、能力判断与未保存修改离开确认
- **分享增强**
  - 公开分享文件页支持直接预览
  - 文件夹分享新增子文件公开下载能力
  - 分享元信息补充 `mime_type` 与 `size`
- **上传任务面板**
  - 新增 `UploadPanel` / `UploadTaskItem`
  - direct / chunked / presigned 三种上传模式统一进任务列表
  - 支持并发上传、分片重试、状态跟踪与完成后保留任务
- **文件尺寸冗余字段**
  - `files` 表新增 `size` 字段
  - migration 回填历史数据，为列表展示和接口返回提供稳定大小信息
- **骨架屏与品牌资源优化**
  - 新增文件网格 / 表格 / 树等骨架组件
  - 重构 logo SVG 结构并优化登录页、TopBar 的品牌展示

### Changed

- **登录页**
  - 重构为双栏品牌布局 + 多步骤认证交互
  - 支持自动检查账号状态、动态切换登录 / 注册 / 初始化模式
  - 优化表单校验、过渡动画与退出动画
- **文件浏览器**
  - 批量移动 / 复制改为目标目录选择对话框
  - 批量操作结果改为更友好的详细提示
  - 版本历史弹窗改为受控模式，并补全恢复 / 删除确认交互
- **通知与国际化**
  - Toast 改为右下角出现，支持右滑关闭
  - 批量操作、错误提示、版本历史等文案统一接入中英文翻译
- **版本恢复语义**
  - 恢复到某个版本时，删除该版本及之后的历史版本
  - 恢复逻辑改为事务化处理，并在提交后做 blob 引用清理
- **后台周期任务**
  - 上传清理、回收站清理、锁清理、审计日志清理统一纳入 `runtime/tasks.rs`
  - 周期任务增加 panic-safe 包装，避免单个任务异常打死整个循环
- **错误处理**
  - 引入 `MapAsterErr`，统一错误上下文映射，减少重复样板

### Fixed

- 修复公开分享页被登录态检查误伤并跳转到 `/login` 的问题
- 修复 token 刷新失败后的前端会话状态清理逻辑
- 修复版本恢复后历史列表与 blob 清理不一致的问题
- 修复文件大小信息在多个链路中的不一致问题
- 修复上传任务列表状态互相覆盖、不可滚动、完成即消失等体验问题
- 修复文件树拖拽到根目录时缺少操作反馈的问题

### Breaking Changes

- **API**: `/api/v1/auth/login` 请求字段由 `username` 调整为 `identifier`

---

**统计数据**：
- 139 files changed, 7,915 insertions(+), 1,786 deletions(-)
- 11 commits

## [v0.0.1-alpha.2] - 2026-03-23

### Release Highlights

**前端完整重写！** 从 PoC 级别升级到现代 UI 架构，新增国际化、主题系统、响应式布局。

- **i18n 国际化** — react-i18next，中英双语，5 个命名空间，即时切换
- **主题系统** — Light / Dark / System 三种模式 + 4 套色板（Blue / Green / Purple / Orange），CSS 变量 oklch
- **响应式布局** — 可折叠侧栏、全局顶栏、移动端 overlay
- **网格 / 列表视图** — 双视图切换，记住偏好，缩略图卡片 + 可排序表格
- **多选 + 批量操作** — 勾选框选择，底部浮动操作栏，批量删除 / 移动 / 复制
- **递归文件夹树** — 懒加载展开，替代原来的平铺列表

### Added

- **i18n 系统**
  - react-i18next + i18next-browser-languagedetector
  - 5 个命名空间：common / files / auth / admin / search
  - 中英双语完整翻译（125+ 键值对）
  - 自动检测浏览器语言，localStorage 持久化
- **主题系统**
  - `themeStore` — Light / Dark / System 模式，matchMedia 监听系统偏好
  - 4 套色板预设（blue / green / purple / orange），每套含 light + dark 变体
  - CSS 变量 oklch 色彩空间，`[data-theme]` 属性切换
  - 所有偏好存 localStorage
- **公共组件库** `components/common/`
  - ThemeSwitcher — Sun / Moon / Monitor 下拉切换
  - ColorPresetPicker — 色板圆点选择器
  - LanguageSwitcher — 中英语言下拉
  - EmptyState — 图标 + 标题 + 描述 + 操作按钮
  - LoadingSpinner — 居中旋转加载
  - ConfirmDialog — AlertDialog 封装，destructive 变体
  - ViewToggle — 网格 / 列表图标切换
  - BatchActionBar — 底部浮动栏（选择数 + 删除 / 移动 / 复制）
- **新布局组件**
  - Sidebar — 桌面可折叠（240px / 56px），移动端 overlay + 遮罩
  - TopBar — 全局顶栏：汉堡菜单 + 面包屑 + 主题 / 语言 / 用户下拉
- **文件浏览器组件**
  - FileGrid — 响应式网格（2-6 列），缩略图卡片
  - FileTable — 列表表格，可排序列头，全选勾选框
  - FileCard — 网格卡片，hover 显示勾选框
  - FileThumbnail — 提取复用，sm / lg 两种尺寸
  - FileContextMenu — 右键菜单（下载 / 分享 / 复制 / 重命名 / 锁 / 版本 / 删除）
  - CreateFolderDialog — 从 FileBrowserPage 提取
  - RenameDialog — 文件 / 文件夹重命名，自动选中文件名（不含扩展名）
- **设置页** `/settings`
  - 主题模式 + 色板选择
  - 语言切换
  - 文件浏览器默认视图模式
- **键盘快捷键**
  - Ctrl/Cmd + A — 全选
  - Escape — 取消选择
  - / 或 Ctrl/Cmd + K — 聚焦搜索
- **工具函数** `lib/format.ts`
  - `formatBytes` / `formatDate` / `formatDateAbsolute`
  - 替代 5 处重复实现

### Changed

- **AppLayout** — 重写为 TopBar + 可折叠 Sidebar + main content 三段式
- **FolderTree** — 从平铺列表重写为递归懒加载树（展开 / 折叠 / 子文件夹加载）
- **fileStore** — 完全重写，新增 viewMode / sortBy / sortOrder / selectedFileIds / selectedFolderIds
- **FileBrowserPage** — 从 267 行单体重写为 ~80 行编排器
- **PageHeader** — 简化为薄层组件，面包屑移至 TopBar
- **AdminLayout** — 加 i18n 翻译 + ThemeSwitcher / LanguageSwitcher
- **所有 11 个页面** — 全部加入 i18n 翻译，hardcoded 英文字符串归零
- **所有破坏性操作** — 统一使用 ConfirmDialog 确认
- **所有原生 `<select>`** — 统一替换为 shadcn Select 组件
- **暗色模式兼容** — Badge / 状态色全部加 `dark:` 变体

### Removed

- `FileList.tsx` — 被 FileGrid + FileTable 替代
- FileBrowserPage 中的 batch PoC 面板（手动输入 ID）— 被 BatchActionBar 替代
- 5 处重复的 `formatBytes` / `formatDate` 内联函数

### Dependencies

- 新增 `react-i18next` 16.6
- 新增 `i18next` 25.10
- 新增 `i18next-browser-languagedetector` 8.2

---

**统计数据**：
- 79 files changed, 3,632 insertions(+), 1,506 deletions(-)
- 1 commit

## [v0.0.1-alpha.1] - 2026-03-23

### Release Highlights

**AsterDrive 第一个公开版本！** 自托管云存储系统，Rust 单二进制分发，MIT 许可证。

- **完整文件管理** — 上传（直传/分片/S3 presigned）、下载、复制、移动、在线编辑、版本历史、缩略图
- **WebDAV 协议** — RFC 4918 Class 1 + LOCK，独立账号系统，数据库持久化锁，DeltaV 版本查询
- **存储策略系统** — Local + S3 双驱动，用户级/文件夹级策略覆盖，sha256 去重 + ref_count
- **分享链接** — 密码保护、过期时间、下载次数限制、缩略图支持
- **搜索 + 批量操作 + 审计日志** — 完整的后端 API，Admin 审计可追溯

### Added

- **文件管理**
  - multipart 流式上传（64KB 块 sha256，blob 去重 + ref_count）
  - 分片上传（init → chunk → complete，幂等性保证）
  - S3 presigned 直传（策略级开关，临时路径 → copy_object → 删 temp）
  - 流式下载（Content-Length，不全量缓冲）
  - 文件复制（blob 引用计数，不复制实际数据）
  - 移动 / 重命名（同名冲突检测）
  - 在线编辑（PUT /content，ETag 乐观锁 + 悲观锁检查）
  - 文件版本历史（自动保存旧版本，支持回滚）
  - 图片缩略图（WebP，按需生成，长期缓存）
- **文件夹管理**
  - 创建 / 删除 / 复制 / 移动 / 重命名
  - 递归操作（软删除、硬删除、复制均支持深层嵌套）
  - 循环检测（移动时防止 A → B → A）
- **存储系统**
  - 存储策略体系（系统默认 + 用户级 + 文件夹级覆盖）
  - Local 驱动 + S3 驱动（aws-sdk-s3）
  - 存储配额管理（用户级，管理员可调）
  - Driver Registry 热加载（策略更新后自动清理缓存）
- **认证授权**
  - JWT 双 Token（Access + Refresh），HttpOnly Cookie 存储
  - argon2 密码哈希
  - 自动 401 → refresh token 重试
  - 角色系统（admin / user），第一个注册用户自动成为管理员
- **WebDAV**
  - RFC 4918 Class 1 + LOCK 完整实现
  - Basic Auth（独立 webdav_accounts 表）+ Bearer JWT
  - DbLockSystem 数据库持久化锁（重启不丢锁，后台每小时清理过期锁）
  - root_folder_id 访问限制
  - 大文件临时文件流式处理
  - macOS 兼容（过滤 `._*` / `.DS_Store`）
  - RFC 3253 DeltaV 版本历史查询
- **分享链接**
  - 唯一 token + 密码保护（argon2）+ 过期时间 + 下载次数限制
  - 公开路由 `/s/{token}`（查看 / 验证密码 / 下载 / 文件夹浏览 / 缩略图）
  - Cookie 签名验证（SHA256，1 小时有效）
- **回收站**
  - 软删除（deleted_at 列，所有列表查询自动过滤）
  - 恢复（原文件夹已删除时自动恢复到根目录）
  - 永久删除（blob cleanup + 缩略图 + 属性 + 配额）
  - 后台自动清理（可配置保留天数，默认 7 天）
- **搜索 API**
  - GET `/api/v1/search` — 文件名 LIKE 模糊搜索 + 元数据过滤（MIME / 大小 / 日期）
  - 跨数据库兼容（LOWER() + LIKE）
  - 支持 file / folder / all 类型过滤，folder_id 限定范围，分页
- **批量操作**
  - POST `/api/v1/batch/{delete,move,copy}` — file_ids + folder_ids 混合类型
  - 每项独立执行，返回 succeeded / failed / errors 汇总
  - 100 项上限
- **审计日志**
  - audit_logs 表（action + entity + details + IP / UA）
  - Fire-and-forget 写入（不阻塞业务操作）
  - 运行时配置开关（audit_log_enabled / audit_log_retention_days）
  - Admin 查询 API（过滤 + 分页）
  - 后台自动清理过期日志
  - 覆盖：文件 / 文件夹 / 登录注册 / 分享 / 批量操作 / 配置变更
- **自定义属性**
  - entity_properties 表（entity_type + entity_id + namespace + name + value）
  - WebDAV PROPPATCH 兼容
  - REST API: GET / PUT / DELETE
- **配置系统**
  - 静态配置: `config.toml`（环境变量 ASTER__ 覆盖），首次启动自动生成
  - 运行时配置: system_config 表（Admin API 热改）
  - 配置定义单一数据源（definitions.rs），启动时 ensure_defaults
  - Schema API + 类型校验 + 前端分组渲染
- **缓存**
  - CacheBackend trait（NoopCache / MemoryCache / RedisCache）
  - CacheExt 泛型扩展（自动 serde 序列化）
  - Policy + Share 查询缓存
- **监控**
  - Prometheus 指标（`metrics` feature 门控）+ sysinfo 系统指标
  - Health / Ready 端点
- **管理后台**
  - 用户管理（角色、状态、配额、强制删除）
  - 存储策略管理（CRUD、连接测试、用户级分配）
  - 分享管理（全局列表、管理员删除）
  - WebDAV 锁管理（列表、强制释放、过期清理）
  - 系统配置管理（分类、schema、类型校验）
  - 审计日志查询
- **前端 PoC**
  - React 19 + Vite 8 + Tailwind CSS 4 + shadcn/ui + zustand
  - 文件浏览器（列表视图 + 面包屑导航 + 缩略图 + 预览 + 拖拽上传）
  - 管理后台（用户 / 策略 / 分享 / 锁 / 配置 / 审计日志）
  - 搜索页、批量操作面板
  - rust-embed 编译进单二进制
- **测试**
  - 30+ 集成测试覆盖全部核心功能
  - OpenAPI spec 自动生成（utoipa + swagger-ui）
- **API 文档**
  - utoipa 注解全部端点
  - Swagger UI（debug 构建）
  - OpenAPI JSON 自动导出

### Dependencies

- **Web**: actix-web 4.13, actix-governor 0.10
- **ORM**: sea-orm 2.0.0-rc.37（SQLite / MySQL / PostgreSQL）
- **Auth**: jsonwebtoken 10, argon2 0.5
- **Storage**: aws-sdk-s3 1.127
- **Cache**: moka 0.12, redis 1.1
- **WebDAV**: dav-server 0.11
- **API Docs**: utoipa 5.4, utoipa-swagger-ui 9.0
- **Image**: image crate（jpeg/png/gif/webp/bmp/tiff）
- **Frontend**: React 19, Vite 8, Tailwind CSS 4, shadcn/ui, zustand 5, uppy 5

---

**统计数据**：
- 287 files changed, 48,597 insertions(+)
- 66 commits
- Rust Edition 2024, MSRV 1.91.1

[Unreleased]: https://github.com/AptS-1547/AsterDrive/compare/v0.0.1-alpha.10...HEAD
[v0.0.1-alpha.10]: https://github.com/AptS-1547/AsterDrive/compare/v0.0.1-alpha.9...v0.0.1-alpha.10
[v0.0.1-alpha.9]: https://github.com/AptS-1547/AsterDrive/compare/v0.0.1-alpha.8...v0.0.1-alpha.9
[v0.0.1-alpha.8]: https://github.com/AptS-1547/AsterDrive/compare/v0.0.1-alpha.7...v0.0.1-alpha.8
[v0.0.1-alpha.7]: https://github.com/AptS-1547/AsterDrive/compare/v0.0.1-alpha.6...v0.0.1-alpha.7
[v0.0.1-alpha.6]: https://github.com/AptS-1547/AsterDrive/compare/v0.0.1-alpha.5...v0.0.1-alpha.6
[v0.0.1-alpha.5]: https://github.com/AptS-1547/AsterDrive/compare/v0.0.1-alpha.4...v0.0.1-alpha.5
[v0.0.1-alpha.4]: https://github.com/AptS-1547/AsterDrive/compare/v0.0.1-alpha.3...v0.0.1-alpha.4
[v0.0.1-alpha.3]: https://github.com/AptS-1547/AsterDrive/compare/v0.0.1-alpha.2...v0.0.1-alpha.3
[v0.0.1-alpha.2]: https://github.com/AptS-1547/AsterDrive/compare/v0.0.1-alpha.1...v0.0.1-alpha.2
[v0.0.1-alpha.1]: https://github.com/AptS-1547/AsterDrive/releases/tag/v0.0.1-alpha.1
