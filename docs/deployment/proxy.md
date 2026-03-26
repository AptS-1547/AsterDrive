# 反向代理

正式对外提供服务时，建议在 AsterDrive 前面放一个反向代理，用来处理：

- HTTPS
- 域名
- 大文件上传
- WebDAV 客户端接入

## WebDAV 代理时必须保留的内容

如果启用了 WebDAV，请确认代理层不会丢失：

- `Authorization`
- `Depth`
- `Destination`
- `Overwrite`
- `If`
- `Lock-Token`
- `Timeout`
- 各类 WebDAV 方法：`PROPFIND`、`MOVE`、`COPY`、`LOCK`、`UNLOCK`

如果这些头或方法被拦掉，桌面客户端通常会出现登录失败、移动失败、锁异常或上传失败。

## 上传大小和超时

上传是否经过反向代理，取决于当前存储策略：

- `direct` / `chunked`：上传流量直接经过 AsterDrive 与代理层
- `presigned` / `presigned_multipart`：浏览器会直接把文件或分片发到对象存储，代理层和 AsterDrive 只参与协商与完成阶段

如果你经常上传大文件，反向代理最好先放开自己的请求体限制，例如在 Nginx 里：

```nginx
client_max_body_size 0;
```

真正的限制仍然来自：

- WebDAV：`webdav.payload_limit`
- 文件落盘：存储策略 `max_file_size`

## Caddy

```txt
drive.example.com {
    reverse_proxy 127.0.0.1:3000
}
```

Caddy 适合快速起步。如果你没有特殊代理需求，先用这个最省事。

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

## 额外提醒

- 如果你修改了 WebDAV 前缀，代理路径和客户端地址都要一起改
- 公开分享页、主站页面和静态资源都由同一个 AsterDrive 服务返回，不需要额外再拆一套静态站点
- 如果你启用了 S3 / MinIO 直传，大文件主流量可能不会经过你的反向代理，但对象存储本身仍然需要配置浏览器上传所需的 CORS
