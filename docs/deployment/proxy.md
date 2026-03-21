# 反向代理

生产环境通常应在 AsterDrive 前面放一个反向代理，用来处理：

- HTTPS
- 域名
- 大文件上传
- WebDAV 客户端接入

## 代理时需要保留的内容

如果启用了 WebDAV，请确认代理层不会丢失：

- `Authorization`
- `Depth`
- `Destination`
- `Overwrite`
- `If`
- `Lock-Token`
- `Timeout`
- 各类 WebDAV 方法：`PROPFIND`、`MOVE`、`COPY`、`LOCK`、`UNLOCK`

## 上传大小

代理层要先取消自己的 body 限制，例如在 Nginx 里：

```nginx
client_max_body_size 0;
```

真正的限制仍然来自：

- 普通 REST：后端固定 payload 限制
- WebDAV：`webdav.payload_limit`
- 文件落盘：存储策略 `max_file_size`

## Caddy

```txt
drive.example.com {
    reverse_proxy 127.0.0.1:3000
}
```

Caddy 默认会透传大部分头和方法，适合快速起步。

## Nginx

```nginx
server {
    listen 443 ssl http2;
    server_name drive.example.com;

    ssl_certificate     /path/to/cert.pem;
    ssl_certificate_key /path/to/key.pem;

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

## 当前代码相关的注意事项

- `/swagger-ui` 只在 `debug` 构建存在；如果上游是发布镜像，没有这个路径是正常行为
- `/s/:token`、`/assets/*` 和其余前端页面都由同一个后端服务返回，不需要再额外拆分静态站点
