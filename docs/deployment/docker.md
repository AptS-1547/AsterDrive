# Docker 部署

Docker 适合 NAS、单机和小团队部署。  
最省心的做法是把数据库、上传目录和临时目录都放到 `/data`，这样升级容器时最不容易丢数据。

容器把服务跑起来，不等于可以直接把 `3000` 端口长期暴露到公网。  
正式上线时，前面还是应该接一层反向代理来处理 HTTPS、上传限制、WebDAV 和 WOPI。

## `/data` 里通常会有什么

如果你使用官方镜像并把数据库与临时目录都指向 `/data`，卷里通常会看到：

- `asterdrive.db`
- `uploads/`
- `.tmp/`
- `.uploads/`

其中：

- `asterdrive.db` 和 `uploads/` 需要长期保留
- `.tmp/` 和 `.uploads/` 一般不用备份，但会影响本地磁盘占用

## 先试跑一遍

如果你现在还是纯 HTTP 测试环境，可以先直接运行：

```bash
docker run -d \
  --name asterdrive \
  -p 3000:3000 \
  -e ASTER__SERVER__HOST=0.0.0.0 \
  -e ASTER__AUTH__BOOTSTRAP_INSECURE_COOKIES=true \
  -e ASTER__DATABASE__URL="sqlite:///data/asterdrive.db?mode=rwc" \
  -v asterdrive-data:/data \
  ghcr.io/apts-1547/asterdrive:latest
```

这只会在第一次初始化时把浏览器 Cookie 的 HTTPS 要求设成关闭。  
正式切到 HTTPS 后，到后台把对应系统设置改回开启，然后把这个环境变量去掉。

## 长期部署建议挂载配置文件

在宿主机准备一个 `config.toml`，只写你要覆盖的项目即可，例如：

```toml
[auth]
jwt_secret = "replace-with-your-own-random-secret"
bootstrap_insecure_cookies = false

[server]
temp_dir = "/data/.tmp"
upload_temp_dir = "/data/.uploads"
```

然后把它只读挂载进容器：

```bash
docker run -d \
  --name asterdrive \
  -p 3000:3000 \
  -e ASTER__SERVER__HOST=0.0.0.0 \
  -e ASTER__DATABASE__URL="sqlite:///data/asterdrive.db?mode=rwc" \
  -v asterdrive-data:/data \
  -v "$(pwd)/config.toml:/data/config.toml:ro" \
  ghcr.io/apts-1547/asterdrive:latest
```

如果不挂载 `config.toml`，容器第一次启动时也会自动生成一份默认配置到 `/data/config.toml`，但你还是应该把它纳入卷管理，不适合长期完全放任在容器层里。

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
      - ./config.toml:/data/config.toml:ro
    restart: unless-stopped

volumes:
  asterdrive-data:
```

## 第一次部署最值得先确认的项

- `auth.jwt_secret` 是否已经固定
- 如果暂时是纯 HTTP 测试，是否只在首次引导时设置了 `bootstrap_insecure_cookies = true`
- 切到 HTTPS 后，后台系统设置里的 Cookie 安全开关是否已经改回开启
- 如果站点对外访问，`公开站点地址` 是否已经填成真实域名
- 如果要开放注册、找回密码或邮箱改绑，测试邮件是否已经发通
- 数据库、上传目录和临时目录是否确实落在持久化卷里
- 默认策略组是否已经创建
- 如果启用了外部 Office / WOPI 打开方式，至少用一个真实 Office 文件试开并保存一次
- 如果以后要走 S3 / MinIO，是否已经计划好对象存储浏览器上传放行规则和密钥管理

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

升级后建议重新打开浏览器页面，再检查一次登录、上传、分享、策略组、WebDAV，以及你正在使用的外部打开方式。
