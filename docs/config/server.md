# 服务器配置

`[server]` 这一组决定 AsterDrive 监听哪个地址、哪个端口，以及服务端临时文件写到哪里。

```toml
[server]
host = "127.0.0.1"
port = 3000
workers = 0
temp_dir = "data/.tmp"
upload_temp_dir = "data/.uploads"
```

## 什么时候需要改这组配置

- Docker 或容器部署：把 `host` 改成 `0.0.0.0`
- 端口被占用：改 `port`
- 想把临时目录放到更大的磁盘：改 `temp_dir` 和 `upload_temp_dir`
- 不确定线程数：先保持 `workers = 0`

## 各字段的作用

| 字段 | 默认值 | 作用 |
| --- | --- | --- |
| `host` | `"127.0.0.1"` | 监听地址；容器部署通常改成 `0.0.0.0` |
| `port` | `3000` | HTTP 监听端口 |
| `workers` | `0` | 工作线程数；`0` 表示自动按 CPU 数量决定 |
| `temp_dir` | `"data/.tmp"` | 服务端通用临时文件目录 |
| `upload_temp_dir` | `"data/.uploads"` | 分片上传和上传恢复使用的临时目录 |

## `temp_dir` 和 `upload_temp_dir` 有什么影响

这两个目录会直接影响本地磁盘占用。  
最常见的用途包括：

- 大文件分片上传
- 上传恢复
- 本地存储的临时拼装
- 少数需要先经过服务端临时处理的上传路径

如果你经常上传大文件，建议把这两个目录放到容量更充足的本地磁盘。

## 常见写法

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

- 大多数部署不需要手动调整 `workers`
- 长期部署时，临时目录最好用绝对路径
- 如果前面已经有反向代理，应用本身继续监听内部端口即可

## 对应环境变量

```bash
ASTER__SERVER__HOST=0.0.0.0
ASTER__SERVER__PORT=3000
ASTER__SERVER__TEMP_DIR=/data/.tmp
ASTER__SERVER__UPLOAD_TEMP_DIR=/data/.uploads
```
