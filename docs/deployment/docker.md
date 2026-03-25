# Docker 部署

Docker 适合 NAS、单机和小团队部署。最简单的做法是把数据库和默认上传目录都放到 `/data`。

## 先准备两个持久化位置

- `/data`：数据库和默认本地上传目录
- `/config.toml`：容器里的配置文件路径

## 本地或内网先试跑一遍

如果你现在还是纯 HTTP 测试，可以先直接运行：

```bash
docker run -d \
  --name asterdrive \
  -p 3000:3000 \
  -e ASTER__SERVER__HOST=0.0.0.0 \
  -e ASTER__AUTH__COOKIE_SECURE=false \
  -e ASTER__DATABASE__URL="sqlite:///data/asterdrive.db?mode=rwc" \
  -v asterdrive-data:/data \
  ghcr.io/apts-1547/asterdrive:latest
```

这样会让：

- 数据库位于 `/data/asterdrive.db`
- 默认本地上传目录位于 `/data/uploads`
- 登录 Cookie 允许在纯 HTTP 环境下使用

正式切到 HTTPS 后，把 `ASTER__AUTH__COOKIE_SECURE=false` 去掉，或者在配置文件里改回 `true`。

## 长期部署建议挂载配置文件

在宿主机准备一个 `config.toml`，只写你要覆盖的项目即可，例如：

```toml
[auth]
jwt_secret = "replace-with-your-own-random-secret"
cookie_secure = true
```

然后把它只读挂载进容器：

```bash
docker run -d \
  --name asterdrive \
  -p 3000:3000 \
  -e ASTER__SERVER__HOST=0.0.0.0 \
  -e ASTER__DATABASE__URL="sqlite:///data/asterdrive.db?mode=rwc" \
  -v asterdrive-data:/data \
  -v "$(pwd)/config.toml:/config.toml:ro" \
  ghcr.io/apts-1547/asterdrive:latest
```

## Compose 示例

```yaml
services:
  asterdrive:
    image: ghcr.io/apts-1547/asterdrive:latest
    ports:
      - "3000:3000"
    environment:
      ASTER__SERVER__HOST: 0.0.0.0
      ASTER__DATABASE__URL: sqlite:///data/asterdrive.db?mode=rwc
    volumes:
      - asterdrive-data:/data
      - ./config.toml:/config.toml:ro
    restart: unless-stopped

volumes:
  asterdrive-data:
```

## 第一次部署最值得先确认的项

- `auth.jwt_secret` 是否已经固定
- 如果暂时是纯 HTTP 测试，`auth.cookie_secure` 是否是 `false`
- WebDAV 路径是否符合预期
- 数据库和上传目录是否确实落在持久化卷里

## 查看运行状态

```bash
docker logs -f asterdrive
```

## 升级

使用 `docker compose` 时，升级通常就是：

```bash
docker compose pull
docker compose up -d
```

升级后建议刷新浏览器缓存，再检查一次上传、分享和 WebDAV。
