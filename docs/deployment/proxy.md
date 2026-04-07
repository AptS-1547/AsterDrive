# 反向代理

正式对外提供服务时，建议在 AsterDrive 前面放一层反向代理，用来处理：

- HTTPS
- 域名
- 大文件上传
- WebDAV 客户端接入

## 先记住两件事

- 浏览器页面、公开分享页和静态资源都由同一个 AsterDrive 服务返回
- WebDAV 对代理层的请求方法和请求头要求更高，不能像普通网站一样随便裁剪

## WebDAV 代理时必须保留什么

如果启用了 WebDAV，请确认代理层不会丢失：

- `Authorization`
- `Depth`
- `Destination`
- `Overwrite`
- `If`
- `Lock-Token`
- `Timeout`
- `PROPFIND`
- `MOVE`
- `COPY`
- `LOCK`
- `UNLOCK`

如果这些头或方法被拦掉，桌面客户端通常会出现登录失败、移动失败、锁异常或上传失败。

## 上传大小和超时

上传是否经过反向代理，取决于当前存储策略：

- 本地存储或服务端参与的上传：主体流量会经过代理层
- S3 / MinIO Presigned 直传：主体流量会直接去对象存储

如果你经常上传大文件，代理层最好先放开自己的请求体限制，例如 Nginx：

```nginx
client_max_body_size 0;
```

真正的限制通常来自这几处：

- WebDAV：`webdav.payload_limit`
- 存储策略：`max_file_size`
- 反向代理：请求体大小和超时

## Caddy

```txt
drive.example.com {
    reverse_proxy 127.0.0.1:3000
}
```

如果你只是想先把站点和 HTTPS 跑起来，Caddy 是最省事的起点。

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

## 如果你打算开启应用层限流

当前版本的应用层限流按 AsterDrive 实际看到的连接来源 IP 工作。

如果所有请求进入 AsterDrive 时都只显示成代理地址，那么应用层限流会把所有用户都当成同一个来源。  
这类部署里，更稳妥的做法通常是在反向代理层限流。

## 额外提醒

- 如果你修改了 WebDAV 前缀，代理路径和客户端地址都要一起改
- 不需要另外再部署一套静态站点
- 如果你启用了 S3 / MinIO 直传，对象存储本身仍然需要配置浏览器上传所需的 CORS
