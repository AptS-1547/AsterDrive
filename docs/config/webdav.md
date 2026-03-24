# WebDAV 配置

WebDAV 相关配置分成两部分：

- `config.toml` 中的 `[webdav]`
- 管理后台里的 `webdav_enabled`

## 静态配置

```toml
[webdav]
prefix = "/webdav"
payload_limit = 10737418240
```

## 字段说明

| 字段 | 类型 | 默认值 | 说明 |
| --- | --- | --- | --- |
| `prefix` | string | `"/webdav"` | WebDAV 路径前缀，修改后客户端地址也要一起修改 |
| `payload_limit` | usize | `10737418240` | WebDAV 上传体积硬上限，默认 10 GiB |

## 运行时开关

管理员在系统设置里关闭 `webdav_enabled` 后，WebDAV 会停止对外提供服务。

## 用户一般怎么用

最常见的做法是：

1. 在 `设置 -> WebDAV` 创建一个专用账号
2. 给它指定用户名和密码
3. 需要时限制到某个根目录
4. 把地址、用户名和密码填进 Finder、Windows 或 rclone

推荐优先使用 WebDAV 专用账号，而不是直接复用网页登录会话。

## 默认地址

```text
https://你的域名/webdav/
```

如果你把 `prefix` 改成了 `/dav`，那客户端地址也要改成：

```text
https://你的域名/dav/
```

## 反向代理注意事项

如果 WebDAV 放在反向代理后面，请确保代理层不会丢失：

- `Authorization`
- WebDAV 方法
- 常见 WebDAV 请求头，如 `Depth`、`Destination`、`Overwrite`、`If`、`Lock-Token`、`Timeout`

完整示例见 [反向代理部署](/deployment/proxy)。

## 上传大小

如果你预计通过 WebDAV 上传大文件，请同步检查：

- `webdav.payload_limit`
- 反向代理的上传大小限制
- 存储策略里的单文件大小限制
