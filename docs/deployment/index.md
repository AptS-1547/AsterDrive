# 部署概览

AsterDrive 适合做成一个单服务应用来部署。网页、公开分享页和 WebDAV 都由同一个服务提供，不需要再额外拆一套静态站点。

## 推荐方式

| 方式 | 适合谁 |
| --- | --- |
| [Docker](/deployment/docker) | NAS、单机、小团队、已有容器环境 |
| [systemd](/deployment/systemd) | 云主机、物理机、长期稳定运行 |
| 直接运行二进制 | 本地测试、临时验证 |

## 部署前先确认四件事

### 1. 数据放哪

至少要确认这些内容会不会在重启或升级后保留下来：

- 配置文件
- 数据库
- 本地上传目录

### 2. 登录是否走 HTTPS

正式上线时，建议通过 HTTPS 提供服务，并保持：

```toml
[auth]
cookie_secure = true
```

### 3. WebDAV 要不要启用

如果你需要 Finder、Windows 或同步工具接入，部署时就要一起考虑：

- WebDAV 路径
- 反向代理
- 上传大小限制

### 4. 存储后端用什么

- 本地磁盘：部署最简单
- S3 / MinIO：适合对象存储场景

## 启动后会自动完成什么

只要服务成功启动，就会自动完成这些准备：

- 生成默认 `config.toml`
- 连接数据库并自动执行迁移
- 自动创建默认本地存储策略 `Local Default`
- 初始化系统设置
- 启动回收站、锁、审计日志等清理任务

## 三类配置不要混在一起

- `config.toml`：决定服务怎么启动
- 管理后台里的系统设置：决定 WebDAV、回收站、历史版本等运行行为
- 存储策略：决定文件落到哪里、怎么上传

## 上线后先验收这几项

1. 首页能正常打开并登录
2. `/health` 和 `/health/ready` 返回正常
3. 能创建文件夹并上传一个文件
4. 回收站恢复正常
5. 分享链接可以打开
6. 如果启用了 WebDAV，桌面客户端能成功连接

## 继续阅读

- [Docker 部署](/deployment/docker)
- [systemd 部署](/deployment/systemd)
- [反向代理](/deployment/proxy)
- [首次启动会发生什么](/deployment/runtime-behavior)
