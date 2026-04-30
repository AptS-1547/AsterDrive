# 配置总览

::: tip 这一篇先帮你分清"在哪改"
AsterDrive 的配置散在几个地方。先把这些层分清楚，之后就不会把"部署问题"塞到后台里，也不会把"用户规则"硬塞回 `config.toml`。
找到对应分层后，再翻具体页面就行，本页不需要从头读到尾。
:::

## 一共有哪几层

- **`config.toml`** —— 决定服务怎么启动：监听地址、节点模式、数据库、日志、WebDAV 前缀、限流
- **`管理 -> 系统设置`** —— 全站规则：公开站点地址、品牌、注册登录、邮件、跨域、任务调度、媒体处理、回收站、版本历史、WOPI、WebDAV 开关、审计日志
- **`管理 -> 存储策略`** —— 文件真正存到哪里、用哪种上传方式
- **`管理 -> 策略组`** —— 不同用户、团队、文件大小走哪条存储路线
- **`管理 -> 远程节点`** —— 主控怎么接 follower，以及 follower 的默认接收落点在哪里
- **反向代理 / 对象存储自己的配置** —— HTTPS、大文件上传、WebDAV 透传、S3 直传

前面几层是 AsterDrive 自己的配置；最后一层属于反向代理、对象存储和外部网络环境。

## 我想做这件事，去哪改

| 你想做什么 | 去哪里改 |
| --- | --- |
| 改监听地址、端口、线程数、临时目录、primary / follower 模式 | [服务器](/config/server) |
| 改数据库地址、连接池、启动重试 | [数据库](/config/database) |
| 固定登录签名密钥、第一次纯 HTTP 引导 | [登录与会话](/config/auth) |
| 公开站点地址、品牌、注册、Cookie、Token、调度、回收站、版本、配额、WOPI、WebDAV、审计 | [系统设置](/config/runtime) |
| 配 SMTP、发测试邮件、改邮件模版 | [邮件](/config/mail) |
| 文件存哪里、上传/下载怎么走 | [存储策略](/config/storage) |
| 不同用户/团队走哪条存储路线 | [存储策略](/config/storage) |
| 接远程 follower，配置默认接收落点 | [远程节点](/guide/remote-nodes) |
| 改 WebDAV 路径或 WebDAV 上传硬上限 | [WebDAV](/config/webdav) |
| 给公网入口加访问限流 | [访问限流](/config/rate-limit) |
| 改缓存或日志输出方式 | [缓存](/config/cache) / [日志](/config/logging) |

## `config.toml` 在哪、怎么写

首次启动时，如果当前工作目录里没有 `data/config.toml`，AsterDrive 会自动生成一份默认配置（含一段随机的 `jwt_secret`）。

::: tip 只写要覆盖的项
不需要把整份默认配置全抄出来，`config.toml` 里只写你要改的项即可，其他保留默认。
:::

配置优先级：

```text
ASTER__ 环境变量  >  config.toml  >  内置默认值
```

环境变量用双下划线 `__` 表示层级：

```bash
ASTER__SERVER__PORT=8080
ASTER__DATABASE__URL="postgres://user:pass@localhost/asterdrive"
ASTER__WEBDAV__PREFIX=/dav
```

## `config.toml` 里有哪些分区

| 分区 | 作用 |
| --- | --- |
| [server](/config/server) | 监听地址、端口、线程数、临时目录、节点模式、follower 接收根目录 |
| [database](/config/database) | 数据库连接、连接池、启动重试 |
| [auth](/config/auth) | 登录签名密钥、首次纯 HTTP 引导 |
| [cache](/config/cache) | 内存缓存 / Redis / 关闭缓存 |
| [logging](/config/logging) | 日志级别、格式、输出文件、轮转 |
| [webdav](/config/webdav) | WebDAV 路径前缀、上传体积硬上限 |
| [rate_limit](/config/rate-limit) | 登录、公开分享和一般访问的限流规则 |

## 后台系统设置当前的分组

`管理 -> 系统设置` 现在按这些分组显示：

- 站点配置
- 用户管理
- 认证与 Cookie
- 邮件投递
- 网络访问
- 运行与调度
- 存储与保留
- WebDAV
- 审计日志
- 自定义配置
- 其他

::: tip 上线前最容易踩的几项
- 对外上线前，先填 `公开站点地址`；多个公开域名逐项添加，主域名放在最前面
- 准备开放注册、找回密码或邮箱改绑前，先把邮件发通
- 纯 HTTP 测试环境才临时关闭 Cookie 的 HTTPS 要求
- 容量紧张时，缩短回收站、历史版本、任务产物保留时间
- 缩略图不符合预期时，检查 `存储与保留 -> 媒体处理`
- 需要 OnlyOffice 一类在线预览时，去 `站点配置 -> 预览应用` 调整
- 接远程节点时，enroll 成功后还要在远程节点详情里创建默认接收落点
:::

详情见 [系统设置](/config/runtime) 和 [邮件](/config/mail)。

如果后台暂时进不去，或者你想在停机窗口里离线检查、校验或批量写入系统设置，走 [运维 CLI](/deployment/ops-cli)。

## 存储策略和策略组不在 `config.toml` 里

它们在后台页面维护，分别决定：

- **存储策略** —— 文件真正存到哪里、单文件大小上限、分片大小、上传方式
- **策略组** —— 用户或团队上传时命中哪条存储策略

详情见 [存储策略](/config/storage)。

## 路径要分清是相对谁

如果你写相对路径，记住三套语义不一样：

- `data/config.toml` 的位置 —— **相对当前工作目录**
- `[database]` 和 `[server]` 里的相对路径 —— **相对 `data/config.toml` 所在目录**（也就是 `./data/`）
- 默认本地存储策略 `data/uploads` —— **相对当前工作目录**（不是 `data/` 内）

不同部署方式默认落点：

- 本地直接运行：项目目录下的 `data/`
- systemd：`WorkingDirectory/data/`
- Docker 官方镜像：容器里的 `/data`

::: warning 长期部署写绝对路径
数据库路径、本地存储路径、临时目录最好都写绝对路径——以后不会被工作目录偷偷影响。
:::
