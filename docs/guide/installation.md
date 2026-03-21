# 安装部署

这一页说明如何构建、部署并完成 AsterDrive 的首次初始化，只写当前仓库已经确认存在的行为。

## 环境要求

- Rust `1.91.1+`
- Bun，用于构建 `frontend-panel/`
- Node.js `24+`，仅在你想复用当前 Docker 前端构建阶段时需要
- SQLite、MySQL 或 PostgreSQL
- Docker，如果你要使用仓库里的 `Dockerfile`
- 反向代理，如果你要提供 HTTPS、域名访问或 WebDAV 桌面客户端接入

补充说明：

- 当前前端构建命令是 `bun run build`，不是 `bun build`
- S3 兼容存储例如 MinIO、AWS S3 是可选项，后续通过存储策略配置

## 从源码构建

先构建前端，再构建 Rust 后端。

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

发布产物位于：

```text
target/release/aster_drive
```

如果 `frontend-panel/dist/` 不存在，后端仍然可以启动并提供 API，但嵌入的 Web 页面只会显示回退页。

## 首次启动

第一次成功启动不只是监听 `3000` 端口，还会自动完成下面这些初始化。

- 如果当前工作目录没有 `config.toml`，自动生成一份
- 如果继续使用默认数据库地址，自动创建 SQLite 数据库
- 自动执行全部数据库迁移
- 如果系统里还没有任何存储策略，自动创建默认本地策略 `Local Default`
- 默认本地策略的存储目录是 `data/uploads`
- 自动向 `system_config` 写入默认运行时配置
- 第一个注册用户自动成为 `admin`
- 新用户会自动分配当前默认存储策略

默认访问地址：

```text
http://127.0.0.1:3000
```

注册第一个用户：

```bash
curl -X POST http://127.0.0.1:3000/api/v1/auth/register \
  -H "Content-Type: application/json" \
  -d '{"username":"admin","email":"admin@example.com","password":"change-this-password"}'
```

登录并保存 Cookie：

```bash
curl -X POST http://127.0.0.1:3000/api/v1/auth/login \
  -H "Content-Type: application/json" \
  -c cookies.txt \
  -d '{"username":"admin","password":"change-this-password"}'
```

## 当前工作目录会影响默认路径

AsterDrive 当前默认使用相对路径。

- `config.toml` 读取自 `./config.toml`
- 默认 SQLite URL 是 `sqlite://asterdrive.db?mode=rwc`
- 默认本地存储策略写入 `data/uploads`

这意味着你从哪个目录启动服务，就会影响配置文件、SQLite 文件和本地上传目录最终落在哪里。

## Docker 部署

仓库提供多阶段 `Dockerfile`，最终产物是单个 `scratch` 镜像。

从源码构建镜像：

```bash
docker build -t asterdrive .
```

使用持久化卷启动：

```bash
docker run -d \
  --name asterdrive \
  -p 3000:3000 \
  -e ASTER__SERVER__HOST=0.0.0.0 \
  -e ASTER__DATABASE__URL="sqlite:///data/asterdrive.db?mode=rwc" \
  -v asterdrive-data:/data \
  asterdrive
```

当前镜像有几个关键事实：

- 进程默认从 `/` 目录启动
- 如果不覆盖路径，`config.toml` 会生成在 `/config.toml`
- 默认 SQLite 文件会落在 `/asterdrive.db`
- 默认本地存储策略仍然使用 `data/uploads`，在容器里会解析为 `/data/uploads`

在容器里，通常更推荐把数据库和上传目录都放到 `/data`。
如果你希望静态配置在容器替换后仍然保留，可以改用环境变量或显式挂载 `/config.toml`。

## 反向代理

生产环境应把 AsterDrive 放在反向代理后面。

反向代理主要负责：

- HTTPS
- 域名接入
- 大文件上传
- WebDAV 客户端接入

### Caddy

Caddy 适合快速起步，通常不需要额外处理 WebDAV 方法和请求头。

```text
drive.example.com {
    reverse_proxy 127.0.0.1:3000
}
```

### Nginx

Nginx 需要显式放开 body 限制，并保留 WebDAV 相关请求头。

```nginx
server {
    listen 443 ssl http2;
    server_name drive.example.com;

    ssl_certificate     /path/to/fullchain.pem;
    ssl_certificate_key /path/to/privkey.pem;

    client_max_body_size 0;

    location / {
        proxy_pass http://127.0.0.1:3000;
        proxy_http_version 1.1;
        proxy_request_buffering off;
        proxy_buffering off;

        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }

    location /webdav/ {
        proxy_pass http://127.0.0.1:3000;
        proxy_http_version 1.1;
        proxy_request_buffering off;
        proxy_buffering off;

        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_set_header Authorization $http_authorization;
        proxy_set_header Depth $http_depth;
        proxy_set_header Destination $http_destination;
        proxy_set_header Overwrite $http_overwrite;
        proxy_set_header If $http_if;
        proxy_set_header Lock-Token $http_lock_token;
        proxy_set_header Timeout $http_timeout;
    }
}
```

如果你修改了 `[webdav].prefix`，代理路径和所有客户端挂载地址也要同步修改。

## 环境变量覆盖

静态配置支持通过 `ASTER__` 前缀环境变量覆盖。

优先级：

```text
环境变量 > config.toml > 内置默认值
```

常见映射：

| `config.toml` 键 | 环境变量 |
| --- | --- |
| `[server].host` | `ASTER__SERVER__HOST` |
| `[server].port` | `ASTER__SERVER__PORT` |
| `[database].url` | `ASTER__DATABASE__URL` |
| `[auth].jwt_secret` | `ASTER__AUTH__JWT_SECRET` |
| `[webdav].prefix` | `ASTER__WEBDAV__PREFIX` |
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
