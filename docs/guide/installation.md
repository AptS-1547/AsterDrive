# 部署手册

这一篇讲怎么把 AsterDrive 真正部署起来，覆盖 Docker、systemd 和直接运行二进制三种场景。

如果你只是想试用一下、不打算长期跑，[快速开始](./getting-started) 的一条 `docker run` 就够了。这一篇是给你**真要上线**用的——比快速开始多了反向代理、HTTPS、数据持久化、跨数据库这些事。

AsterDrive 的部署目标很简单：

- 把服务稳定跑起来
- 把数据放在可靠的位置
- 让浏览器上传、分享、WebDAV 和在线预览 / 编辑在你的网络环境里正常工作

网页、公开分享页、管理后台和 WebDAV 都由同一个 AsterDrive 服务提供，**不需要另外部署一套前端站点**——这是设计上的取舍：少一个服务，少一处出错。

::: tip 如果你想做主控 + 从节点
先把主控实例按这一篇跑稳，再去看 [远程节点](./remote-nodes) 那一章。  
多节点不是“另一种安装命令”，而是另一层架构选择，拆开讲更不容易把人绕晕。
:::

## 先选部署方式

| 方式            | 适合谁                                  |
| --------------- | --------------------------------------- |
| Docker          | NAS、家用服务器、小团队、已经有容器环境 |
| systemd         | 云主机、物理机、想长期稳定运行          |
| 直接运行二进制  | 本机试用、临时验证                      |

第一次部署，优先选 Docker。  
长期运行在 Linux 服务器上，优先选 systemd。

## 上线前先确认这几件事

### 数据准备

重启和升级后必须保留的内容至少包括：

- `data/config.toml`
- 数据库文件，或者外部数据库的连接信息
- 本地上传目录

服务运行时还会使用临时目录：

- `data/.tmp`
- `data/.uploads`

这两个目录通常不需要备份，但要保证本地磁盘有足够空间。

::: details 为什么 config.toml 也要备份
`config.toml` 里有 `jwt_secret`——这是签登录 token 的密钥。

如果你不备份它，重启后密钥重新生成，**所有用户的现有登录都会立刻失效**——他们需要重新登录。这不是数据丢失，但用户体验会很糟。

正式部署务必把 `config.toml` 一起备份。
:::

### 访问方式

正式上线时，**必须**通过反向代理提供 HTTPS，并在代理层给站点页面补上一条可用的基线 `Content-Security-Policy`。不要把整站 CSP 直接改成全站 `sandbox`。同时保持：

```toml
[auth]
bootstrap_insecure_cookies = false
```

如果你只是本机或内网 HTTP 首次引导，可以临时设成：

```toml
[auth]
bootstrap_insecure_cookies = true
```

这只会影响第一次初始化时浏览器 Cookie 是否允许在纯 HTTP 下发送。  
一旦数据库里已经有 `auth_cookie_secure` 这个运行时设置，再改静态引导项不会自动回写旧值。

别把 `:3000` 长期直接暴露到公网。  
浏览器页面、WebDAV、分享页和 WOPI 都走同一个服务，正式部署应该统一挂在反向代理后面。

### 注册策略

当前版本默认允许用户在登录页自行注册，但管理员可以在后台关闭：

```text
管理 -> 系统设置 -> 用户管理 -> 允许公开注册新用户
```

如果你打算直接把站点暴露到公网，先确认：

- 是不是要保留公开注册
- 邮件投递是否已经配好
- `公开站点地址` 是否已经填成真实域名

否则用户就可能能注册、能申请重置密码，却收不到正确的邮件链接。

### WebDAV

如果你要让 Finder、Windows 资源管理器、rclone 或同步工具接入，部署时就要一起考虑：

- WebDAV 路径
- 反向代理（要放行 `PROPFIND` / `MKCOL` / `MOVE` / `COPY` / `LOCK` / `UNLOCK` 这些方法，nginx 默认只放行 GET/POST/HEAD）
- 上传大小限制

### 在线预览 / WOPI

如果你准备把 Office 文件交给外部服务打开，还要一起确认：

- `公开站点地址` 是否已经填成真实域名
- `站点配置 -> 预览应用` 是否已经配置好对应打开方式
- 如果外部 Office 服务和 AsterDrive 不在同一个来源，`网络访问` 是否已经放行那个域名

### 文件落点

如果文件继续放本地磁盘，部署最简单。  
如果文件要放到 S3 / MinIO，请提前准备：

- Endpoint
- Bucket
- Access Key / Secret Key
- 如果要使用浏览器直传，再准备对象存储的浏览器上传放行规则（CORS）

## Docker 部署

Docker 最适合首次试跑和日常维护。

```bash
docker run -d \
  --name asterdrive \
  -p 3000:3000 \
  -e ASTER__SERVER__HOST=0.0.0.0 \
  -e ASTER__DATABASE__URL="sqlite:///data/asterdrive.db?mode=rwc" \
  -v asterdrive-data:/data \
  ghcr.io/apts-1547/asterdrive:latest
```

如果当前还是纯 HTTP 测试环境，再额外加上：

```bash
-e ASTER__AUTH__BOOTSTRAP_INSECURE_COOKIES=true
```

更完整的挂载方式、升级方式和卷规划见 [Docker 部署](/deployment/docker)。

## systemd 部署

systemd 适合长期运行的 Linux 服务器。

这类部署最重要的是两件事：

- 先定好 `WorkingDirectory`
- 再决定配置文件、数据库、上传目录和临时目录放哪

完整示例见 [systemd 部署](/deployment/systemd)。

## 直接运行二进制

如果你已经拿到 `aster_drive` 可执行文件，直接运行即可：

```bash
./aster_drive
```

纯 HTTP 测试环境可以这样临时启动：

```bash
ASTER__AUTH__BOOTSTRAP_INSECURE_COOKIES=true ./aster_drive
```

如果你后面准备把另一台 AsterDrive 接成远程存储后端，直接看 [远程节点](./remote-nodes)。

## 数据库选哪个

我们默认用 SQLite，理由前面 [快速开始](./getting-started) 讲过——零运维、单文件备份、试用门槛低。

但 **SQLite 不是万能的**。下面三种情况建议直接上 PostgreSQL：

- 多人长期协作，并发写入高
- 计划部署多个 AsterDrive 实例共享同一份数据
- 数据量预计会到几百 GB 或上千万文件

切换很简单：用 `aster_drive database-migrate` 把 SQLite 数据搬到 PostgreSQL，配置改一下连接串即可。详见 [运维 CLI](/deployment/ops-cli)。

我们对 PostgreSQL 和 MySQL 一视同仁——大部分功能在三个后端上行为一致。少数差异（比如 MySQL 的某些 ALTER 锁表）会在 [升级与版本迁移](/deployment/upgrade) 里明确标注。

## 需要离线检查或迁移时

同一个 `aster_drive` 二进制里还带了运维子命令，适合这些场景：

- 新部署后先跑一轮离线检查
- 后台暂时进不去，直接查看或修改系统设置
- 把 SQLite 迁到 PostgreSQL / MySQL

最常见的三类命令是：

- `doctor`：默认检查数据库和关键运行时配置；加 `--deep` 可继续核对存储计数、Blob 引用、对象清单和目录树一致性，`--fix` 可修复部分计数漂移
- `config`：离线查看、校验、设置、导入或导出系统设置
- `database-migrate`：跨数据库后端搬迁业务数据

具体命令和使用顺序看 [运维 CLI](/deployment/ops-cli)。

## 首次启动后会自动完成什么

第一次成功启动后，AsterDrive 会自动完成：

- 生成默认 `data/config.toml`
- 连接数据库并自动更新数据库结构
- 创建默认本地存储策略 `Local Default`
- 创建默认策略组 `Default Policy Group`
- 初始化系统设置默认项
- 启动邮件派发、后台任务派发、周期清理和底层文件一致性检查任务

之后在浏览器打开：

```text
http://服务器地址:3000
```

第一个创建出来的账号会自动成为管理员。

后续普通用户如果通过公开注册创建账号，需要完成邮箱激活后才能登录。

## 部署后先验收这些项

完整验收清单见 [首次启动检查](/deployment/runtime-behavior#启动后马上检查这些项)。

部署完最少跑通这几项就算服务能用：

- 首页可以正常打开并登录
- 可以创建文件夹并上传文件
- 管理后台可以打开
- `GET /health` 和 `GET /health/ready` 返回正常

如果你启用了 WebDAV、外部 Office / WOPI、邮件等额外能力，按 [首次启动检查](/deployment/runtime-behavior#启动后马上检查这些项) 列表对应章节再各跑一遍。

## 下一步该看哪里

- 想挂 HTTPS、Caddy、Nginx 或 Traefik：看 [反向代理](/deployment/reverse-proxy)
- 想在命令行里做部署检查、离线配置或跨库迁移：看 [运维 CLI](/deployment/ops-cli)
- 想确认默认目录、默认策略和后台任务是否按预期创建：看 [首次启动检查](/deployment/runtime-behavior)
- 想改数据库、WebDAV、日志或系统设置：看 [配置说明](/config/)
- 想做完整备份和恢复：看 [备份与恢复](/deployment/backup)
- 升级到新版本：看 [升级与版本迁移](/deployment/upgrade)
