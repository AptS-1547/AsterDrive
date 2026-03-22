# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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

[v0.0.1-alpha.1]: https://github.com/AptS-1547/AsterDrive/releases/tag/v0.0.1-alpha.1
