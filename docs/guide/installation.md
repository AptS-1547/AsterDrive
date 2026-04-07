# 部署手册

AsterDrive 只需要启动一个服务。
浏览器页面、公开分享页、管理后台和 WebDAV 都由这个服务提供，不需要再单独部署前端站点。

## 先选部署方式

| 方式 | 适合谁 |
| --- | --- |
| Docker | NAS、家用服务器、小团队、已经有容器环境 |
| systemd | 云主机、物理机、想长期稳定运行 |
| 直接运行二进制 | 本机试用、临时验证 |

第一次部署，优先选 Docker。  
长期在 Linux 服务器上运行，优先选 systemd。

## 部署前先确认这五件事

### 1. 数据放在哪里

重启和升级后要保留下来的内容至少包括:

- `config.toml`
- 数据库文件，或者外部数据库的连接信息
- 本地上传目录

服务运行时还会使用临时目录:

- `data/.tmp`
- `data/.uploads`

这两个目录通常不需要备份，但需要保证本地磁盘有足够空间。

### 2. 是否通过 HTTPS 对外访问

正式上线时，建议通过 HTTPS 提供服务，并保持:

```toml
[auth]
bootstrap_insecure_cookies = false
```

如果你只是本机或内网 HTTP 首次引导，可以临时改成:

```toml
[auth]
bootstrap_insecure_cookies = true
```

### 3. 用户是否允许自行注册

当前版本默认允许用户在登录页自行注册，暂时没有内置的“关闭注册”开关。
如果你准备直接把服务开放到公网，先确认这是否符合你的使用场景。

### 4. 是否启用 WebDAV

如果你要给 Finder、Windows 资源管理器、rclone 或同步工具使用，部署时就要一起考虑:

- WebDAV 地址
- 反向代理
- 上传大小限制

### 5. 文件是落本地，还是落到 S3 / MinIO

如果文件要放到对象存储，请提前准备:

- Endpoint
- Bucket
- Access Key / Secret Key
- 如果要用浏览器直传，再准备对象存储的 CORS 设置

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

如果你现在还是纯 HTTP 测试环境，额外加上:

```bash
-e ASTER__AUTH__BOOTSTRAP_INSECURE_COOKIES=true
```

默认情况下，`/data` 会承载:

- 数据库
- 本地上传目录
- 服务端临时目录

更完整的写法见 [Docker 部署](/deployment/docker)。

## systemd 部署

systemd 适合长期运行的 Linux 服务器。  
这类部署最重要的是先定好 `WorkingDirectory`，再决定配置文件、数据库和上传目录放哪。

完整步骤见 [systemd 部署](/deployment/systemd)。

## 直接运行二进制

如果你已经拿到 `aster_drive` 可执行文件，直接运行即可:

```bash
./aster_drive
```

纯 HTTP 测试环境可以这样临时启动:

```bash
ASTER__AUTH__BOOTSTRAP_INSECURE_COOKIES=true ./aster_drive
```

## 首次启动后会自动完成什么

服务第一次成功启动后，会自动完成:

- 生成默认 `config.toml`
- 连接数据库并自动更新数据库结构
- 创建默认本地存储策略 `Local Default`
- 创建默认策略组 `Default Policy Group`
- 创建默认上传目录和临时目录
- 初始化系统设置

之后在浏览器打开:

```text
http://服务器地址:3000
```

第一个创建出来的账号会自动成为管理员。

## 部署后先验证这几项

- 可以正常打开首页并登录
- 登录页能正确进入“登录 / 注册 / 创建管理员”流程
- 可以创建文件夹并上传文件
- 可以把文件移到回收站并恢复
- 管理后台可以打开
- `管理 -> 存储策略` 和 `管理 -> 策略组` 都能正常打开
- `GET /health` 和 `GET /health/ready` 返回正常
- 如果启用了 WebDAV，客户端可以成功连接
