# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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

[Unreleased]: https://github.com/AptS-1547/AsterDrive/compare/v0.0.1-alpha.6...HEAD
[v0.0.1-alpha.6]: https://github.com/AptS-1547/AsterDrive/compare/v0.0.1-alpha.5...v0.0.1-alpha.6
[v0.0.1-alpha.5]: https://github.com/AptS-1547/AsterDrive/compare/v0.0.1-alpha.4...v0.0.1-alpha.5
[v0.0.1-alpha.4]: https://github.com/AptS-1547/AsterDrive/compare/v0.0.1-alpha.3...v0.0.1-alpha.4
[v0.0.1-alpha.3]: https://github.com/AptS-1547/AsterDrive/compare/v0.0.1-alpha.2...v0.0.1-alpha.3
[v0.0.1-alpha.2]: https://github.com/AptS-1547/AsterDrive/compare/v0.0.1-alpha.1...v0.0.1-alpha.2
[v0.0.1-alpha.1]: https://github.com/AptS-1547/AsterDrive/releases/tag/v0.0.1-alpha.1
