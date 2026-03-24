# Docker 部署

Docker 适合 NAS、单机和小团队部署。推荐把数据库和默认上传目录一起放到 `/data`，这样持久化最简单。

## 推荐启动命令

```bash
docker run -d \
  --name asterdrive \
  -p 3000:3000 \
  -e ASTER__SERVER__HOST=0.0.0.0 \
  -e ASTER__DATABASE__URL="sqlite:///data/asterdrive.db?mode=rwc" \
  -v asterdrive-data:/data \
  -v $(pwd)/config.toml:/config.toml:ro \
  ghcr.io/apts-1547/asterdrive:latest
```

这个启动方式会让：

- 配置文件位于 `/config.toml`
- 数据库位于 `/data/asterdrive.db`
- 默认本地上传目录位于 `/data/uploads`

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

## 配置文件怎么准备

常见做法有两种：

- 在容器外先准备好 `config.toml`，再只读挂载进去
- 或先让服务在持久化工作目录里启动一次，利用自动生成逻辑产出默认配置，再回头修改

第一次部署时，最值得先确认的是这几项：

- `auth.jwt_secret` 是否已经固定
- 如果暂时是本地 HTTP 测试，`auth.cookie_secure` 是否已改成 `false`
- WebDAV 路径和上传大小是否符合预期

## 从源码构建镜像

```bash
docker build -t asterdrive .
```

## 启动后先检查

1. 打开 `http://你的主机:3000`
2. 创建第一个管理员账号
3. 上传一个测试文件
4. 检查 `/health` 和 `/health/ready`
5. 如果要用 WebDAV，再做一次客户端真实连接测试
