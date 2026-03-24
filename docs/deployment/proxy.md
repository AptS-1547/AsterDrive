# 反向代理

生产环境通常应在 AsterDrive 前面放一个反向代理，用来处理：

- HTTPS
- 域名
- 大文件上传
- WebDAV 客户端接入

## 代理时要保留什么

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

代理层要先取消自己的上传大小限制，例如在 Nginx 里：

注意四种上传方式对代理层的压力不同：

- `direct` / `chunked`：上传流量直接经过 AsterDrive 与代理层
- `presigned` / `presigned_multipart`：浏览器会直接把文件或分片发到对象存储，代理层和 AsterDrive 只参与协商与完成阶段

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

## 额外提醒

- 如果你修改了 WebDAV 前缀，代理路径和客户端地址都要一起改
- 公开分享页、主站页面和静态资源都由同一个 AsterDrive 服务返回，不需要额外再拆静态站点
