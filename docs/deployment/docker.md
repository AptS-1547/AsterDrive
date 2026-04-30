# Docker 部署

::: tip 这一篇适合谁
NAS、单机、小团队，或者已经在用容器编排的部署。10 分钟能跑起来。
**正式上线时**前面一定要接反向代理处理 HTTPS——`3000` 端口不要直接对公网开。
:::

官方镜像默认以 **非 root 用户** 运行（UID/GID 固定为 `10001:10001`，用户名 `aster`），并内置了基于 `/health/ready` 的 `HEALTHCHECK`。

如果你把宿主机目录直接 bind mount 到 `/data`（推荐，备份和迁移更直观），**一定要先把目录创建好并把属主改成 `10001:10001`**，否则容器启动时生成 `config.toml`、SQLite 文件或临时目录都会直接报权限错误：

```bash
mkdir -p ./data
sudo chown -R 10001:10001 ./data
```

如果你用 named volume（`docker volume create` 或 compose 里的 `volumes:` 段），Docker 会自动把卷的属主设成容器内运行用户，不需要手动 chown。

容器把服务跑起来，不等于可以直接把 `3000` 端口长期暴露到公网。  
正式上线时，前面还是应该接一层反向代理来处理 HTTPS、**浏览器页面基线** `Content-Security-Policy` 等安全响应头、上传限制、WebDAV 和 WOPI。不要把整站 CSP 直接改成全站 `sandbox`。

::: tip 如果这个容器要跑成从节点
现在 follower 已经支持在启动时直接读取 bootstrap ENV 完成 enroll。  
如果你想用 Docker 把另一台 AsterDrive 接成远程节点，不要再照旧文档手动 `docker exec` 跑 `node enroll`，直接看 [Docker 部署从节点](/deployment/docker-follower)。
:::

## `/data` 里通常会有什么

如果你按上面的命令把 `./data` bind mount 到容器的 `/data`，目录里通常会看到：

- `config.toml`
- `asterdrive.db`
- `uploads/`
- `avatar/`（用户上传头像后）
- `.tmp/`
- `.uploads/`

其中：

- `config.toml`、`asterdrive.db`、`uploads/`，以及如果启用了上传头像的 `avatar/` 需要长期保留
- `.tmp/` 和 `.uploads/` 一般不用备份，但会影响本地磁盘占用

更完整的备份 / 恢复建议见 [备份与恢复](/deployment/backup)。

## 先试跑一遍

如果你现在还是纯 HTTP 测试环境，可以先直接运行：

```bash
mkdir -p ./data
sudo chown -R 10001:10001 ./data

docker run -d \
  --name asterdrive \
  -p 3000:3000 \
  -e ASTER__SERVER__HOST=0.0.0.0 \
  -e ASTER__AUTH__BOOTSTRAP_INSECURE_COOKIES=true \
  -e ASTER__DATABASE__URL="sqlite:///data/asterdrive.db?mode=rwc" \
  -v "$(pwd)/data:/data" \
  ghcr.io/apts-1547/asterdrive:latest
```

这只会在第一次初始化时把浏览器 Cookie 的 HTTPS 要求设成关闭。  
正式切到 HTTPS 后，到后台把对应系统设置改回开启，然后把这个环境变量去掉。

启动后可以用 `docker ps` 看容器状态，正常情况下会在短时间内变成 `healthy`。

## 长期部署：直接编辑宿主机上的 `config.toml`

`config.toml` 现已统一生成在 `/data/config.toml`，与数据库、上传目录位于同一卷下，**不再需要**像旧文档那样将其单独以只读方式挂载到容器中。

按上述命令将 `./data` bind mount 到 `/data` 后，第一次启动时 AsterDrive 会自动生成 `./data/config.toml`，之后可直接在宿主机上编辑该文件以覆盖默认配置，例如：

```toml
[auth]
jwt_secret = "replace-with-your-own-random-secret"
bootstrap_insecure_cookies = false

[server]
temp_dir = "/data/.tmp"
upload_temp_dir = "/data/.uploads"
```

修改完成后，重启容器即可生效。

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
      - ./data:/data
      - /etc/localtime:/etc/localtime:ro
    restart: unless-stopped
```

第一次执行 `docker compose up -d` 之前，请先按顶部说明执行 `mkdir -p ./data && sudo chown -R 10001:10001 ./data`，将宿主机目录准备好。否则容器内的 `aster` 用户（UID/GID `10001`）将无法写入，导致启动失败。

## 第一次部署最值得先确认的项

- `auth.jwt_secret` 是否已经固定
- 如果暂时是纯 HTTP 测试，是否只在首次引导时设置了 `bootstrap_insecure_cookies = true`
- 切到 HTTPS 后，后台系统设置里的 Cookie 安全开关是否已经改回开启
- 反向代理是否已经给浏览器页面补上基线 `Content-Security-Policy` 响应头
- 如果站点对外访问，`公开站点地址` 是否已经填成真实 `https://` 来源；多个公开域名逐项添加，主域名放在最前面
- 如果要开放注册、找回密码或邮箱改绑，测试邮件是否已经发通
- 数据库、上传目录和临时目录是否都落在 bind mount 的 `./data` 目录里，没有遗漏写到容器内层
- 默认策略组是否已经创建
- 如果启用了外部 Office / WOPI 打开方式，至少用一个真实 Office 文件试开并保存一次
- 如果以后要走 S3 / MinIO，是否已经计划好对象存储浏览器上传放行规则和密钥管理
- 如果这台实例实际要跑成 `follower`，是否已经按 [Docker 部署从节点](/deployment/docker-follower) 配好长期 `start_mode` 和一次性 bootstrap ENV

## 查看运行状态

```bash
docker logs -f asterdrive
```

## 升级

如果使用上面的 Compose 示例：

```bash
docker compose pull
docker compose up -d
```

如果是用 `docker run` 直接运行的，步骤一致——拉取新镜像、停止旧容器、用同一条命令再次启动即可（bind mount 的 `./data` 不会受影响）：

```bash
docker pull ghcr.io/apts-1547/asterdrive:latest
docker rm -f asterdrive
# 再次执行前面"先试跑一遍"里的 docker run 命令
```

升级完成后，建议重新打开浏览器页面，重新检查登录、上传、分享、策略组、WebDAV 以及当前正在使用的外部打开方式。
