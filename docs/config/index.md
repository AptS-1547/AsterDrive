# 配置概览

AsterDrive 的配置可以分成三类来看：

- `config.toml`：决定服务怎么启动
- 管理后台里的系统设置：决定回收站、版本、WebDAV 这类运行行为
- 存储策略：决定文件存到哪里、怎么上传

首次启动时，如果当前工作目录不存在 `config.toml`，服务会自动生成一份默认配置。

## 优先级

```text
环境变量 (ASTER__ 前缀) > config.toml > 内置默认值
```

环境变量使用双下划线 `__` 表示层级：

```bash
ASTER__SERVER__PORT=8080
ASTER__DATABASE__URL="postgres://user:pass@localhost/asterdrive"
ASTER__WEBDAV__PREFIX=/dav
```

## `config.toml` 里有哪些分区

| 分区 | 作用 |
| --- | --- |
| [server](/config/server) | 监听地址、端口、工作线程 |
| [database](/config/database) | 数据库连接、连接池、启动重试 |
| [auth](/config/auth) | 登录密钥、会话有效期、Cookie 安全设置 |
| [cache](/config/cache) | 内存缓存 / Redis / 关闭缓存 |
| [logging](/config/logging) | 日志级别、格式、输出文件 |
| [webdav](/config/webdav) | WebDAV 路径前缀和上传体积上限 |

## 管理后台里的系统设置

管理员可以在后台直接调整这些常见设置：

| 设置项 | 作用 |
| --- | --- |
| 默认用户配额 | 新用户注册后默认能使用多少空间 |
| WebDAV 开关 | 是否允许 WebDAV 访问 |
| 回收站保留天数 | 已删除项目保留多久 |
| 历史版本数量 | 单个文件最多保留多少个旧版本 |
| 审计日志开关 | 是否记录关键操作 |
| 审计日志保留天数 | 审计日志保留多久 |

详情见 [系统设置](/config/runtime)。

## 三类配置该怎么分工

| 类型 | 放什么 | 典型示例 |
| --- | --- | --- |
| `config.toml` | 影响启动和登录的参数 | 监听地址、数据库 URL、JWT 密钥、Cookie 安全设置 |
| 系统设置 | 允许管理员在线调整的业务开关 | WebDAV 开关、回收站保留期、版本保留数 |
| 存储策略 | 文件写到哪里，以及怎么上传 | 本地目录、S3、分片大小、是否开启直传 |

## 默认配置示例

下面这份是当前版本会生成的默认配置结构：

```toml
[server]
host = "127.0.0.1"
port = 3000
workers = 0

[database]
url = "sqlite://asterdrive.db?mode=rwc"
pool_size = 10
retry_count = 3

[auth]
jwt_secret = "<首次启动自动生成>"
access_token_ttl_secs = 900
refresh_token_ttl_secs = 604800
cookie_secure = true

[cache]
enabled = true
backend = "memory"
redis_url = ""
default_ttl = 3600

[logging]
level = "info"
format = "text"
file = ""

[webdav]
prefix = "/webdav"
payload_limit = 10737418240
```

## 路径语义

如果你使用相对路径，当前工作目录会影响：

- `config.toml` 的位置
- 默认 SQLite 的位置
- 默认本地上传目录的位置

部署时请始终先确定工作目录，再决定挂载方案。

## 继续阅读

- [服务器](/config/server)
- [登录与会话](/config/auth)
- [存储策略](/config/storage)
- [系统设置](/config/runtime)
- [WebDAV](/config/webdav)
