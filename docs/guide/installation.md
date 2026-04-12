# 部署手册

AsterDrive 的部署重点很简单：

- 把服务稳定跑起来
- 把数据放在可靠的位置
- 让浏览器上传、分享和 WebDAV 在你的网络环境里正常工作

网页、公开分享页、管理后台和 WebDAV 都由同一个 AsterDrive 服务提供，不需要另外部署一套前端站点。

## 先选部署方式

| 方式 | 适合谁 |
| --- | --- |
| Docker | NAS、家用服务器、小团队、已经有容器环境 |
| systemd | 云主机、物理机、想长期稳定运行 |
| 直接运行二进制 | 本机试用、临时验证 |

第一次部署，优先选 Docker。  
长期运行在 Linux 服务器上，优先选 systemd。

## 上线前先确认这几件事

### 数据准备

重启和升级后必须保留的内容至少包括：

- `config.toml`
- 数据库文件，或者外部数据库的连接信息
- 本地上传目录

服务运行时还会使用临时目录：

- `data/.tmp`
- `data/.uploads`

这两个目录通常不需要备份，但要保证本地磁盘有足够空间。

### 访问方式

正式上线时，建议通过 HTTPS 提供服务，并保持：

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
- 反向代理
- 上传大小限制

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

## 首次启动后会自动完成什么

第一次成功启动后，AsterDrive 会自动完成：

- 生成默认 `config.toml`
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

- 首页可以正常打开并登录
- 登录页能正确进入“登录 / 注册 / 创建管理员”流程
- 如果要开放注册、找回密码或邮箱改绑，测试邮件已经能收到
- 可以创建文件夹并上传文件
- 文件可以删除到回收站并恢复
- 管理后台可以打开
- `管理 -> 存储策略` 和 `管理 -> 策略组` 都能正常打开
- `GET /health` 和 `GET /health/ready` 返回正常
- 如果启用了 WebDAV，客户端可以成功连接

## 下一步该看哪里

- 想挂 HTTPS、Caddy 或 Nginx：看 [反向代理](/deployment/proxy)
- 想确认默认目录、默认策略和后台任务是否按预期创建：看 [首次启动检查](/deployment/runtime-behavior)
- 想改数据库、WebDAV、日志或系统设置：看 [配置说明](/config/)
