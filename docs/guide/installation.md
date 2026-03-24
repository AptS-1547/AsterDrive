# 安装部署

这篇文档帮助你选安装方式，并完成第一次可用部署。AsterDrive 面向自托管场景，最常见的两种方式是 Docker 和 systemd。

## 先选一种方式

| 方式 | 适合谁 |
| --- | --- |
| Docker | NAS、单机、小团队、自带容器环境 |
| systemd / 直接运行二进制 | 云主机、物理机、希望自己管理工作目录和日志 |

## 环境准备

- 想从源码构建：需要 Rust、Bun，以及一台能编译前端和后端的机器
- 想直接跑 Docker：需要 Docker 或 Docker Compose
- 想长期对外提供服务：建议准备域名和 HTTPS 反向代理
- 如果要使用 S3 或 MinIO：准备好对象存储地址、桶、访问密钥

## 方式一：Docker

最简单的启动方式如下：

```bash
docker run -d \
  --name asterdrive \
  -p 3000:3000 \
  -e ASTER__SERVER__HOST=0.0.0.0 \
  -e ASTER__DATABASE__URL="sqlite:///data/asterdrive.db?mode=rwc" \
  -v asterdrive-data:/data \
  ghcr.io/apts-1547/asterdrive:latest
```

这个命令会把数据库和默认本地上传目录都放进 `/data`，适合第一次试用。

如果你还要保留自己的 `config.toml`，可以额外挂载：

```bash
-v $(pwd)/config.toml:/config.toml:ro
```

更完整的示例见 [Docker 部署](/deployment/docker)。

## 方式二：从源码构建并运行

```bash
git clone https://github.com/AptS-1547/AsterDrive.git
cd AsterDrive

cd frontend-panel
bun install
bun run build
cd ..

cargo build --release
./target/release/aster_drive
```

如果你只是本地快速验证，也可以直接执行：

```bash
cargo run
```

## 首次启动会自动完成什么

服务第一次成功启动后，会自动完成这些动作：

- 生成 `config.toml`
- 创建默认 SQLite 数据库
- 创建默认本地上传目录 `data/uploads`
- 执行数据库迁移
- 创建默认本地存储策略 `Local Default`
- 初始化系统设置

之后在浏览器打开：

```text
http://127.0.0.1:3000
```

第一个创建的账号会自动成为管理员。

## 本地 HTTP 测试要注意什么

如果你本地直接用 `http://127.0.0.1:3000` 或其他纯 HTTP 地址访问，建议在 `config.toml` 里显式设置：

```toml
[auth]
cookie_secure = false
```

等你正式放到 HTTPS 域名后，再改回：

```toml
[auth]
cookie_secure = true
```

## 部署后先验证这几项

- 可以正常打开首页并登录
- 可以创建文件夹并上传文件
- 可以把文件移到回收站并恢复
- 管理后台可以打开
- `GET /health` 和 `GET /health/ready` 返回正常
- 如果你启用了 WebDAV，客户端可以成功连接

## 工作目录会影响默认路径

如果你直接运行二进制或使用 systemd，当前工作目录会影响：

- `config.toml` 的位置
- 默认 SQLite 数据库的位置
- 默认本地上传目录 `data/uploads` 的位置

所以部署前请先决定好你想把数据放在哪里，再决定从哪个目录启动服务。

## 继续阅读

- [快速开始](/guide/getting-started)
- [部署概览](/deployment/)
- [Docker 部署](/deployment/docker)
- [systemd 部署](/deployment/systemd)
- [配置概览](/config/)
| `[webdav].payload_limit` | `ASTER__WEBDAV__PAYLOAD_LIMIT` |

运行示例：

```bash
export ASTER__SERVER__HOST=0.0.0.0
export ASTER__SERVER__PORT=3000
export ASTER__DATABASE__URL="postgres://aster:secret@127.0.0.1:5432/asterdrive"
export ASTER__WEBDAV__PREFIX="/webdav"

./target/release/aster_drive
```

同一套命名规则也适用于 Docker 和 Compose。
