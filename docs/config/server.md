# 服务器配置

```toml
[server]
host = "127.0.0.1"
port = 3000
workers = 0
temp_dir = "data/.tmp"
upload_temp_dir = "data/.uploads"
```

这一组配置决定 AsterDrive 监听哪个地址、哪个端口，以及服务端临时文件写到哪里。

## 最常改的是哪几项

- Docker 或容器: 把 `host` 改成 `0.0.0.0`
- 需要换端口: 改 `port`
- 想把临时目录放到更大的磁盘: 改 `temp_dir` 和 `upload_temp_dir`
- 不确定线程数: 先保持 `workers = 0`

## 字段说明

| 字段 | 默认值 | 说明 |
| --- | --- | --- |
| `host` | `"127.0.0.1"` | 绑定地址；容器部署通常改成 `0.0.0.0` |
| `port` | `3000` | HTTP 监听端口 |
| `workers` | `0` | 工作线程数；`0` 表示自动选择 |
| `temp_dir` | `"data/.tmp"` | 服务端通用临时文件目录 |
| `upload_temp_dir` | `"data/.uploads"` | 分片上传和上传恢复使用的临时目录 |

## `temp_dir` 和 `upload_temp_dir` 是做什么的

这两个目录会影响本地磁盘占用。

常见用途:

- 大文件分片上传
- 上传恢复
- 某些需要先落本地临时文件的存储策略
- 部分 WebDAV 或文件处理流程

如果你经常上传大文件，或者使用 S3 的服务端暂存上传，建议把这两个目录放到容量更充足的本地磁盘。

## 最常见的写法

### 本机测试

```toml
[server]
host = "127.0.0.1"
port = 3000
workers = 0
temp_dir = "data/.tmp"
upload_temp_dir = "data/.uploads"
```

### Docker 或容器

```toml
[server]
host = "0.0.0.0"
port = 3000
workers = 0
temp_dir = "/data/.tmp"
upload_temp_dir = "/data/.uploads"
```

## 使用建议

- 大多数用户不需要手动调整 `workers`
- 长期部署时，临时目录最好用绝对路径
- 如果你已经用了反向代理，应用本身继续监听内部端口即可

## 对应环境变量

```bash
ASTER__SERVER__HOST=0.0.0.0
ASTER__SERVER__PORT=3000
ASTER__SERVER__TEMP_DIR=/data/.tmp
ASTER__SERVER__UPLOAD_TEMP_DIR=/data/.uploads
```
