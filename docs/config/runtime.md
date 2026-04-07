# 系统设置

系统设置是管理员在后台直接维护的全站选项。  
和 `config.toml` 不同，这些设置通常不需要改文件，也不需要重启服务。

## 这里管理什么

这里负责全站运行规则，例如:

- 认证 Cookie 是否必须通过 HTTPS 发送
- Access / Refresh Token 的有效期
- WebDAV 是否开启
- 回收站保留多久
- 历史版本保留多少个
- 团队归档保留多久
- 新用户默认能用多少空间
- 是否记录审计日志
- Gravatar 头像从哪里加载
- 是否允许指定来源跨域访问

## 当前内置设置

| 设置项 | 默认值 | 说明 |
| --- | --- | --- |
| `auth_cookie_secure` | `true` | 是否只允许浏览器通过 HTTPS 发送认证和分享验证 Cookie |
| `auth_access_token_ttl_secs` | `900` | Access Token 有效期，单位秒 |
| `auth_refresh_token_ttl_secs` | `604800` | Refresh Token 有效期，单位秒 |
| `webdav_enabled` | `true` | 控制 WebDAV 是否可用 |
| `max_versions_per_file` | `10` | 单个文件最多保留多少个历史版本 |
| `trash_retention_days` | `7` | 回收站项目保留天数 |
| `team_archive_retention_days` | `7` | 已归档团队保留天数 |
| `default_storage_quota` | `0` | 新用户默认配额，`0` 表示不限制 |
| `audit_log_enabled` | `true` | 是否记录审计日志 |
| `audit_log_retention_days` | `90` | 审计日志保留天数 |
| `gravatar_base_url` | `https://www.gravatar.com/avatar` | Gravatar 头像基础地址 |
| `cors_allowed_origins` | `""` | 允许跨域访问的来源列表 |
| `cors_allow_credentials` | `false` | 跨域时是否允许带凭据 |
| `cors_max_age_secs` | `3600` | 浏览器缓存预检结果的秒数 |

## 修改后什么时候生效

- 认证 Cookie 安全策略和 Token 有效期: 新登录、刷新和分享密码验证请求会立即按新值生效
- WebDAV 开关: 修改后立即生效
- 回收站保留天数: 后台清理任务会按新值清理
- 历史版本数量: 新版本产生时按新规则处理
- 团队归档保留天数: 后台清理任务会按新值清理
- 默认用户配额: 只影响之后新创建的用户
- 审计日志开关和保留天数: 修改后按新规则生效
- Gravatar 地址: 用户切换到 Gravatar 头像后按新地址加载
- CORS 设置: 新请求会立即按新规则响应

## 管理员最常改的项目

### 新用户默认配额

如果你希望每个新用户一开始就有统一空间，在这里设置。

### 认证与会话

如果你已经切到 HTTPS，对外服务时应保持 `auth_cookie_secure = true`。
如果只是临时纯 HTTP 内网调试，可以先把它关掉，但别把这种状态长期带到正式环境里。

### 回收站保留天数

如果磁盘空间紧张，可以缩短保留时间。  
如果你更看重误删恢复，可以适当延长。

### 历史版本数量

如果用户经常编辑文本文件或通过 WebDAV 覆盖保存，版本数量越大，可恢复空间越大，但也会占用更多存储。

### 团队归档保留天数

团队归档后不会立刻消失，而是进入保留期。
如果你们经常临时停用团队空间，可以适当延长这个天数。

### 审计日志

如果你需要看最近活动、排查问题或追踪关键操作，就保持审计日志开启。

### Gravatar 头像地址

如果你的服务器所在网络访问不到官方 Gravatar，可以把 `gravatar_base_url` 改成可访问的镜像地址。

### 跨域设置（CORS）

只有在这些场景下，你通常才需要改 CORS:

- 浏览器页面和 AsterDrive 不在同一个域名下
- 你自己做了单独的前端站点
- 你要让其他站点在浏览器里直接调用 AsterDrive

大多数“同域名直接访问”的部署，不需要改这里。

## 关于“自定义配置”

后台里也可以新增自定义配置项。  
如果你平时只是正常维护 AsterDrive，优先改内置设置就够了。
