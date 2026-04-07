# WebDAV 配置

WebDAV 相关设置分成两部分：

- `config.toml` 里的 `[webdav]`
- `管理 -> 系统设置` 里的 WebDAV 开关

## `config.toml` 里的静态配置

```toml
[webdav]
prefix = "/webdav"
payload_limit = 10737418240
```

这两项改完后都需要重启服务。

## 各字段的作用

| 字段 | 默认值 | 作用 |
| --- | --- | --- |
| `prefix` | `"/webdav"` | WebDAV 路径前缀，改完后客户端地址也要一起改 |
| `payload_limit` | `10737418240` | WebDAV 上传体积硬上限，默认 10 GiB |

## 后台里的运行时开关

管理员在系统设置里关闭 WebDAV 后，WebDAV 会立刻停止对外提供服务。

## 普通用户一般怎么用

最常见的做法是：

1. 在左侧 `WebDAV` 页面创建一个专用账号
2. 给它指定用户名和密码
3. 需要时限制到根目录下某个文件夹
4. 把地址、用户名和密码填进 Finder、Windows 或 rclone

推荐优先使用 WebDAV 专用账号，而不是直接复用网页登录密码。

## 默认地址

```text
https://你的域名/webdav/
```

如果你把 `prefix` 改成了 `/dav`，那客户端地址也要改成：

```text
https://你的域名/dav/
```

## WebDAV 上传大小要看三处

如果你预计通过 WebDAV 上传大文件，要同时检查：

- `webdav.payload_limit`
- 反向代理的上传大小限制
- 存储策略里的单文件大小限制

三者里只要有一个更小，最终就按更小的那个为准。

## 反向代理时不要丢这些内容

如果 WebDAV 放在反向代理后面，请确保代理层不会丢失：

- `Authorization`
- `Depth`
- `Destination`
- `Overwrite`
- `If`
- `Lock-Token`
- `Timeout`
- `PROPFIND`、`MOVE`、`COPY`、`LOCK`、`UNLOCK` 等 WebDAV 方法

完整示例见 [反向代理部署](/deployment/proxy)。
