# Docker 部署

当前仓库的 `Dockerfile` 是三阶段构建：

1. 构建前端
2. 构建静态链接 Rust 二进制
3. 打包到 `scratch` 镜像

## 当前镜像的重要事实

镜像里只有一个入口：

```text
/aster_drive
```

并且当前 `Dockerfile` 没有设置 `WORKDIR`，因此进程默认在 `/` 目录启动。这会直接影响默认路径：

- 配置文件：`/config.toml`
- 默认 SQLite：`/asterdrive.db`
- 默认本地存储目录：`/data/uploads`

## 推荐做法

如果你用默认本地存储，建议显式把数据库也放到 `/data` 下，这样一个卷就能持久化数据库和上传内容：

```bash
-e ASTER__DATABASE__URL="sqlite:///data/asterdrive.db?mode=rwc"
-v asterdrive-data:/data
```

## 直接运行镜像

最小示例：

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

## 配置文件生成方式

当前程序没有 `--print-config` 之类的参数。常见做法有两种：

- 参考仓库根目录的 `config.example.toml`
- 先在本地运行一次二进制，让它自动生成 `config.toml`

## 从源码构建镜像

```bash
docker build -t asterdrive .
```

也可以显式指定 feature：

```bash
docker build --build-arg CARGO_FEATURES="server" -t asterdrive .
```

## Swagger 说明

发布镜像是 `release` 构建，因此默认不会暴露 `/swagger-ui`。这是构建行为，不是镜像或代理配置问题。
