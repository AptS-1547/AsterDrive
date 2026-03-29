# 配置总览

管理员日常会在三个地方改配置:

- `config.toml`: 决定服务怎么启动
- `管理 -> 系统设置`: 决定 WebDAV、回收站、历史版本、审计日志这类全站行为
- `管理 -> 存储策略`: 决定文件真正存到哪里、怎么上传

先分清这三层，后面就不容易改错地方。

## 先判断你要改哪一层

| 你想做什么 | 去哪里改 |
| --- | --- |
| 改监听地址、端口、临时目录 | [服务器](/config/server) |
| 改数据库 | [数据库](/config/database) |
| 改登录密钥或 Cookie 行为 | [登录与会话](/config/auth) |
| 改文件真正存放的位置 | [存储策略](/config/storage) |
| 改 WebDAV 路径或上传体积上限 | [WebDAV](/config/webdav) |
| 改回收站、历史版本、新用户默认配额、审计日志 | [系统设置](/config/runtime) |
| 想限制公网访问频率 | [访问限流](/config/rate-limit) |
| 改缓存或日志输出方式 | [缓存](/config/cache)、[日志](/config/logging) |

## `config.toml` 在哪里

首次启动时，如果当前工作目录里还没有 `config.toml`，AsterDrive 会自动生成一份默认配置。

只想改少数几项时，不需要把整份默认配置全部抄出来。  
`config.toml` 里只写你要覆盖的字段即可。

配置优先级:

```text
ASTER__ 环境变量 > config.toml > 内置默认值
```

环境变量使用双下划线 `__` 表示层级，例如:

```bash
ASTER__SERVER__PORT=8080
ASTER__DATABASE__URL="postgres://user:pass@localhost/asterdrive"
ASTER__WEBDAV__PREFIX=/dav
```

## `config.toml` 里有哪些分区

| 分区 | 作用 |
| --- | --- |
| [server](/config/server) | 监听地址、端口、线程数、临时目录 |
| [database](/config/database) | 数据库连接、连接池、启动重试 |
| [auth](/config/auth) | 登录密钥、会话有效期、Cookie 安全设置 |
| [cache](/config/cache) | 内存缓存 / Redis / 关闭缓存 |
| [logging](/config/logging) | 日志级别、格式、输出文件与轮转 |
| [webdav](/config/webdav) | WebDAV 路径前缀和上传体积上限 |
| [rate_limit](/config/rate-limit) | 登录、公开分享和一般访问的限流规则 |

## 系统设置里最常改什么

管理员在后台最常改的是:

- WebDAV 开关
- 回收站保留天数
- 单文件历史版本数量
- 新用户默认配额
- 审计日志开关和保留天数
- Gravatar 头像地址

详情见 [系统设置](/config/runtime)。

## 存储策略不在 `config.toml` 里

存储策略在 `管理 -> 存储策略` 页面里维护，不写在 `config.toml` 里。

它决定:

- 文件真正存到哪里
- 默认上传策略是什么
- 用户会走本地存储还是 S3 / MinIO
- 单文件大小和分片大小限制

详情见 [存储策略](/config/storage)。

## 路径一定要想清楚

如果你使用相对路径，当前工作目录会影响:

- `config.toml` 的位置
- SQLite 数据库文件的位置
- 本地上传目录的位置
- 临时目录 `data/.tmp` 和 `data/.uploads` 的位置

例如:

- 本地直接运行: 跟你执行命令的目录有关
- systemd: 跟 `WorkingDirectory` 有关
- Docker 官方镜像: 默认相对路径会落到容器里的 `/data`

长期部署时，如果你不想以后被工作目录影响，数据库路径、本地存储路径和临时目录最好都写成绝对路径。
