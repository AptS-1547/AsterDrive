# 安装

## 环境要求

常见安装方式有两类：

- 直接使用预编译二进制或容器镜像
- 从源码构建完整前后端

源码构建环境：

- Rust `1.91.1+`
- `bun`，用于前端与文档构建

## 从源码构建

```bash
git clone https://github.com/AptS-1547/AsterDrive.git
cd AsterDrive

cd frontend-panel
bun install
bun run build
cd ..

cargo build --release
```

构建产物位于：

```text
target/release/aster_drive
```

## 前端未构建时的行为

后端构建阶段会检查 `frontend-panel/dist`。

- 如果存在，产物会被嵌入二进制
- 如果不存在，`build.rs` 会生成一个回退页

所以即使没有完整前端，服务仍可启动并提供 API，只是首页会提示先构建前端。

## 运行时前端覆盖

服务运行时会优先读取当前工作目录下的：

```text
./frontend-panel/dist
```

只有找不到时才回退到嵌入资源。这个行为适合本地调试，也意味着部署时如果工作目录里意外放了这套目录，页面会优先走磁盘版本。

## 构建文档站点

文档站点位于 `docs/`：

```bash
cd docs
bun install
bun run docs:build
```

## OpenAPI 规范导出

前端消费的静态 OpenAPI 规范可通过测试导出：

```bash
cargo test --test generate_openapi
```

输出路径：

```text
frontend-panel/generated/openapi.json
```

## Docker / OCI 镜像

仓库提供多阶段构建的 `Dockerfile`，最终镜像基于 `scratch`。

```bash
docker pull ghcr.io/apts-1547/asterdrive:latest
```

镜像说明与挂载方式见 [Docker 部署](/deployment/docker)。
