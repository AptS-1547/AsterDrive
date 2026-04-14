# 反向代理（生产环境必需）

AsterDrive 不内置 TLS 终端。  
只要你准备把站点暴露到公网、开放 WebDAV，或者接外部 Office / WOPI 服务，前面就**必须**有一层反向代理来负责：

- HTTPS 证书
- HTTP 到 HTTPS 重定向
- 大文件上传体积限制
- SSE 长连接超时和缓冲
- WebDAV 方法 / 请求头透传
- 前端静态资源缓存头

不要把 `:3000` 直接裸露到公网。  
这只适合本机或内网临时引导；正式上线请把 AsterDrive 绑定到内网地址，然后让 Caddy / Nginx / Traefik 对外暴露 `443`。

## 上线前先对齐这几个值

- `管理 -> 系统设置 -> 站点配置 -> 公开站点地址` 填成真实的 `https://` 域名，例如 `https://drive.example.com`
- 静态引导项 `auth.bootstrap_insecure_cookies` 只在纯 HTTP 首次引导时临时设成 `true`
- 正式切到 HTTPS 后，把 `auth.bootstrap_insecure_cookies` 去掉，并确认运行时 `auth_cookie_secure` 已恢复为开启
- 代理层不要拦掉 WebDAV 的 `PROPFIND`、`MOVE`、`COPY`、`LOCK`、`UNLOCK`
- 代理层不要覆盖缩略图接口返回的 `ETag` / `Cache-Control`

本文默认：

- AsterDrive 监听 `127.0.0.1:3000`
- WebDAV 前缀使用默认值 `/webdav`
- 域名是 `drive.example.com`

如果你改了监听地址、域名或 WebDAV 前缀，把下面配置里的对应值一起改掉。

## 关键路径速查

| 用途 | 路径 |
| --- | --- |
| 前端页面 / 管理后台 / 分享页 | `/` |
| API | `/api/v1/` |
| SSE 存储变更流 | `/api/v1/auth/events/storage` |
| WOPI 回调 | `/api/v1/wopi/` |
| WebDAV | `/webdav/` |
| 前端构建资源 | `/assets/` |
| 内置静态资源 | `/static/` |
| PDF.js 资源 | `/pdfjs/` |

## Caddy

Caddy 最省事，开箱就能处理 HTTPS 和 HTTP 到 HTTPS 跳转。

```caddyfile
drive.example.com {
    encode zstd gzip

    @frontend_assets path /assets/*
    header @frontend_assets Cache-Control "public, max-age=31536000, immutable"

    @embedded_static path /static/* /pdfjs/*
    header @embedded_static Cache-Control "public, max-age=86400"

    reverse_proxy 127.0.0.1:3000 {
        # SSE 需要尽快 flush，不要让代理层攒着不发
        flush_interval -1
    }
}
```

这份配置已经满足：

- 自动 HTTPS
- 自动 HTTP 到 HTTPS 跳转
- SSE 立即刷出
- WebDAV / WOPI / 普通 API 全站透传

补充说明：

- Caddy 默认不会像 Nginx 那样主动卡一个很小的请求体上限；如果你自己额外加了 `request_body` 限制，记得同步放开
- 缩略图接口本身会返回 `ETag` 和 `must-revalidate`，这里不要再额外改写成强缓存

## Nginx

Nginx 需要你自己处理 HTTPS、重定向、上传大小和 SSE。

```nginx
map $http_upgrade $connection_upgrade {
    default upgrade;
    ''      close;
}

server {
    listen 80;
    server_name drive.example.com;
    return 301 https://$host$request_uri;
}

server {
    listen 443 ssl http2;
    server_name drive.example.com;

    ssl_certificate     /etc/letsencrypt/live/drive.example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/drive.example.com/privkey.pem;

    # 大文件上传不要被代理层截断
    client_max_body_size 0;

    proxy_http_version 1.1;
    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto $scheme;
    proxy_set_header Upgrade $http_upgrade;
    proxy_set_header Connection $connection_upgrade;
    proxy_request_buffering off;
    proxy_read_timeout 3600s;
    proxy_send_timeout 3600s;
    send_timeout 3600s;

    location = /api/v1/auth/events/storage {
        proxy_pass http://127.0.0.1:3000;
        proxy_buffering off;
        proxy_cache off;
        add_header X-Accel-Buffering no always;
    }

    location ^~ /assets/ {
        proxy_pass http://127.0.0.1:3000;
        expires 1y;
        add_header Cache-Control "public, max-age=31536000, immutable" always;
    }

    location ^~ /static/ {
        proxy_pass http://127.0.0.1:3000;
        expires 1d;
        add_header Cache-Control "public, max-age=86400" always;
    }

    location ^~ /pdfjs/ {
        proxy_pass http://127.0.0.1:3000;
        expires 1d;
        add_header Cache-Control "public, max-age=86400" always;
    }

    location / {
        proxy_pass http://127.0.0.1:3000;
    }
}
```

这份配置里最容易漏的点就是：

- `client_max_body_size 0`
- `proxy_request_buffering off`
- SSE 专门关掉 `proxy_buffering`
- `X-Forwarded-Proto` 必须保留成 `https`

如果你单独给 `/webdav/` 做 location，也不要加 `limit_except` 去限制方法；否则 Finder、Windows、rclone 一类客户端会直接坏掉。

## Traefik

Traefik 更适合 Docker / Compose 场景。  
它分成两部分：

- Traefik 自己的静态配置：负责 entrypoint、HTTPS 和超时
- AsterDrive 容器的 labels：负责 Host 路由和转发端口

### `traefik.yml`

```yaml
entryPoints:
  web:
    address: ":80"
    http:
      redirections:
        entryPoint:
          to: websecure
          scheme: https
  websecure:
    address: ":443"
    transport:
      respondingTimeouts:
        readTimeout: 0s
        writeTimeout: 0s
        idleTimeout: 3600s

providers:
  docker:
    exposedByDefault: false

certificatesResolvers:
  letsencrypt:
    acme:
      email: ops@example.com
      storage: /letsencrypt/acme.json
      httpChallenge:
        entryPoint: web
```

`readTimeout: 0s` 这一类设置很关键。  
不然大文件上传和 SSE 很容易在代理层先超时。

### `docker-compose.yml` labels

```yaml
services:
  asterdrive:
    image: ghcr.io/apts-1547/asterdrive:latest
    labels:
      - traefik.enable=true

      - traefik.http.routers.asterdrive.rule=Host(`drive.example.com`)
      - traefik.http.routers.asterdrive.entrypoints=websecure
      - traefik.http.routers.asterdrive.tls=true
      - traefik.http.routers.asterdrive.tls.certresolver=letsencrypt
      - traefik.http.routers.asterdrive.service=asterdrive

      - traefik.http.routers.asterdrive-assets.rule=Host(`drive.example.com`) && (PathPrefix(`/assets/`) || PathPrefix(`/static/`) || PathPrefix(`/pdfjs/`))
      - traefik.http.routers.asterdrive-assets.entrypoints=websecure
      - traefik.http.routers.asterdrive-assets.tls=true
      - traefik.http.routers.asterdrive-assets.tls.certresolver=letsencrypt
      - traefik.http.routers.asterdrive-assets.priority=100
      - traefik.http.routers.asterdrive-assets.middlewares=asterdrive-static-cache
      - traefik.http.routers.asterdrive-assets.service=asterdrive

      - traefik.http.middlewares.asterdrive-static-cache.headers.customresponseheaders.Cache-Control=public, max-age=86400

      - traefik.http.services.asterdrive.loadbalancer.server.port=3000
```

Traefik 默认会补上常见的 `X-Forwarded-*` 头。  
你真正要注意的是：

- `web` 要跳到 `websecure`
- `websecure` 的超时别太短
- 不要再给 WebDAV 或缩略图路由套一层会覆盖响应头的 middleware

如果你想把 `/assets/` 做成更激进的 `immutable` 缓存，建议单独再拆一个 router；别顺手把所有 `/api/v1/*` 都改成强缓存，那是给自己找麻烦。

## WebDAV 代理时不要漏掉什么

只要代理层是“整站透传”，一般没事。  
出问题通常发生在你自己手动加了额外限制：

- 限制了 `PROPFIND`、`LOCK`、`UNLOCK`
- 把 `Authorization` 或 `Destination` 一类头删掉了
- 把 `/webdav/` 改成了别的前缀，但客户端地址没一起改

如果你改了 `[webdav].prefix = "/dav"`，那代理层和客户端地址也都要一起跟着改成 `/dav/`。

## WOPI / Office 回调的额外要求

如果你接的是 OnlyOffice、Collabora 或其他 WOPI 服务，再多确认两件事：

- `public_site_url` 必须是用户真实访问的 HTTPS 域名
- 外部 Office 服务必须能访问到 `https://你的域名/api/v1/wopi/...`

最常见的错误现象就是：

- 打开方式按钮能显示，但点开后加载失败
- Office 页面能打开，但读不到文件
- 能打开，却保存不回 AsterDrive

## 缩略图缓存不要自作聪明

AsterDrive 的缩略图接口已经返回了：

- `ETag`
- `Cache-Control: public/private, max-age=0, must-revalidate`

所以代理层应该做的是：

- 保留这些响应头
- 允许浏览器用 `If-None-Match` 走 304 重新验证

而不是：

- 把缩略图一把改成 `immutable`
- 去掉 `ETag`
- 用 CDN 强行缓存成几小时不更新

## 上线后最少验收一次

1. 浏览器能通过 `https://你的域名/` 正常登录
2. `管理 -> 系统设置 -> 公开站点地址` 显示的是 `https://` 域名
3. 上传一个大文件，确认不会被代理层截断
4. 打开两个浏览器标签页，确认文件变更能通过 SSE 刷新出来
5. 如果启用了 WebDAV，用真实客户端连一次
6. 如果启用了 WOPI，用真实 Office 文件试开并保存一次
