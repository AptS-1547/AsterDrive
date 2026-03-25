# 安装部署

AsterDrive 只需要启动一个服务。管理后台、文件页面、公开分享页和 WebDAV 都在这一个服务里。

## 先选部署方式

| 方式 | 适合谁 |
| --- | --- |
| Docker | NAS、家用服务器、小团队、已经有容器环境 |
| systemd | 云主机、物理机、希望长期稳定运行 |
| 直接运行二进制 | 本地试用、临时验证 |

## 部署前先确认四件事

### 数据放在哪

至少要确定这些内容放在哪个目录或卷里：

- `config.toml`
- SQLite 数据库文件
- 本地上传目录

如果你使用默认本地存储，服务第一次启动时会自动创建 `data/uploads`。

### 是否通过 HTTPS 对外访问

正式上线时，建议通过 HTTPS 提供服务，并保持：

```toml
[auth]
cookie_secure = true
```

如果你只是本机 HTTP 测试，可以暂时改成：

```toml
[auth]
cookie_secure = false
```

### 用户注册方式

当前版本默认允许新用户从登录页自行注册，未提供内置的“关闭注册”开关。部署到公网前，先确认这是否符合你的使用场景。

### 是否启用 WebDAV

如果你要给 Finder、Windows 资源管理器、rclone 或同步工具使用，请一开始就把下面几项一起考虑进去：

- WebDAV 地址
- 反向代理
- 上传大小限制

### 是否使用 S3 或 MinIO

如果文件不打算落在本地磁盘，而是放到对象存储，请先准备：

- Endpoint
- Bucket
- Access Key / Secret Key
- 浏览器直传所需的 CORS 设置

## Docker 部署

适合绝大多数首次部署。

最简启动命令：

```bash
docker run -d \
  --name asterdrive \
  -p 3000:3000 \
  -e ASTER__SERVER__HOST=0.0.0.0 \
  -e ASTER__DATABASE__URL="sqlite:///data/asterdrive.db?mode=rwc" \
  -v asterdrive-data:/data \
  ghcr.io/apts-1547/asterdrive:latest
```

如果你现在还是纯 HTTP 测试环境，额外加上 `-e ASTER__AUTH__COOKIE_SECURE=false`。

容器里最重要的持久化目录是 `/data`：

- 数据库可以放到 `/data/asterdrive.db`
- 默认本地上传目录会落到 `/data/uploads`

如果还要固定配置文件，再额外挂载一个宿主机上的 `config.toml`：

```bash
-v $(pwd)/config.toml:/config.toml:ro
```

更完整的示例见 [Docker 部署](/deployment/docker)。

## systemd 部署

适合云主机和长期运行的 Linux 机器。关键点只有两个：

- `WorkingDirectory` 要固定
- 数据库和本地上传目录最好用绝对路径，或者都放在 `WorkingDirectory` 下面

完整步骤见 [systemd 部署](/deployment/systemd)。

## 直接运行二进制

如果你已经拿到 `aster_drive` 可执行文件，直接运行即可：

```bash
./aster_drive
```

第一次试用时，如果当前还是纯 HTTP 访问，可以先这样启动：

```bash
ASTER__AUTH__COOKIE_SECURE=false ./aster_drive
```

## 首次启动后会自动完成这些动作

服务第一次成功启动后，会自动完成这些动作：

- 生成 `config.toml`
- 创建默认 SQLite 数据库
- 创建默认本地上传目录 `data/uploads`
- 执行数据库迁移
- 创建默认本地存储策略 `Local Default`
- 初始化系统设置

之后在浏览器打开：

```text
http://服务器地址:3000
```

第一个创建的账号会自动成为管理员。

## 部署后先验证这几项

- 可以正常打开首页并登录
- 可以创建文件夹并上传文件
- 可以把文件移到回收站并恢复
- 管理后台可以打开
- `GET /health` 和 `GET /health/ready` 返回正常
- 如果你启用了 WebDAV，客户端可以成功连接
