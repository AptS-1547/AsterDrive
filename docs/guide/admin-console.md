# 管理后台

这一页说明 AsterDrive 当前已经稳定提供的管理员能力，重点是后端真实支持的管理动作。

## 入口

第一个注册用户会自动成为 `admin`。

当前内置管理页面对应这些路由：

- `/admin/users`
- `/admin/policies`
- `/admin/shares`
- `/admin/locks`
- `/admin/settings`

前端界面后续可以调整，但下面这些管理能力已经由服务端实现。

## 用户管理

管理员可以管理全部用户账号。

### 当前可做的事

- 列出全部用户
- 查看单个用户详情
- 在 `user` 和 `admin` 之间切换角色
- 禁用或重新启用账号
- 修改用户总存储配额
- 强制永久删除非管理员用户及其全部数据

更新用户：

```bash
curl -X PATCH http://127.0.0.1:3000/api/v1/admin/users/42 \
  -b cookies.txt \
  -H "Content-Type: application/json" \
  -d '{"role":"user","status":"active","storage_quota":107374182400}'
```

强制删除非管理员用户：

```bash
curl -X DELETE http://127.0.0.1:3000/api/v1/admin/users/42 \
  -b cookies.txt
```

当前保护规则：

- 初始管理员账号 `id = 1` 不能被禁用
- 初始管理员账号不能被降级
- 初始管理员账号不能被删除
- 其他管理员账号必须先降级为普通用户，才能被强制删除

强制删除是不可逆操作，会清理这个用户的文件、文件夹、分享、WebDAV 账号、策略分配、上传会话、资源锁以及用户记录本身。

## 存储策略管理

存储策略决定文件写到哪里，以及上传时用什么方式。

### 支持的策略类型

- `local`
- `s3`

### 当前可做的事

- 创建本地或 S3 兼容策略
- 编辑已有策略
- 测试已保存策略的连通性
- 在保存前直接测试一组参数
- 把某个策略设为系统默认策略
- 删除可安全删除的策略

创建一个 S3 策略：

```bash
curl -X POST http://127.0.0.1:3000/api/v1/admin/policies \
  -b cookies.txt \
  -H "Content-Type: application/json" \
  -d '{
    "name":"archive-s3",
    "driver_type":"s3",
    "endpoint":"https://s3.example.com",
    "bucket":"archive",
    "access_key":"AKIA...",
    "secret_key":"secret",
    "base_path":"asterdrive/",
    "max_file_size":10737418240,
    "is_default":false
  }'
```

测试已保存策略：

```bash
curl -X POST http://127.0.0.1:3000/api/v1/admin/policies/3/test \
  -b cookies.txt
```

删除保护和当前限制：

- 不能删除系统里唯一的默认存储策略
- 只要还有 Blob 引用该策略，就不能删除
- 如果取消默认标记会导致系统没有任何默认策略，也会被拒绝
- `PATCH /api/v1/admin/policies/{id}` 不能修改 `driver_type`
- 创建策略时 `chunk_size` 当前会先写成固定的 `5 MiB`，如果要改，创建后再 `PATCH`

## 用户存储策略分配

用户级策略分配决定某个用户能用哪些存储策略，以及默认使用哪一个。

### 当前可做的事

- 给一个用户分配多个策略
- 指定其中一个为该用户默认策略
- 为每条用户策略分配设置配额
- 在用户仍有其他策略时移除某条分配

给用户分配策略：

```bash
curl -X POST http://127.0.0.1:3000/api/v1/admin/users/42/policies \
  -b cookies.txt \
  -H "Content-Type: application/json" \
  -d '{"policy_id":3,"is_default":true,"quota_bytes":53687091200}'
```

这里有几点要区分：

- `quota_bytes` 是“这个用户在这条策略分配上的额度”
- 用户本身的 `storage_quota` 是“用户总额度”
- 一个用户只能有一个默认分配策略
- 不能移除用户唯一剩下的那条策略分配

上传时的策略解析顺序是：

```text
文件夹策略 -> 用户默认策略 -> 系统默认策略
```

## 系统运行时配置

AsterDrive 现在有两层配置。

### 静态配置

`config.toml` 负责启动期配置，例如：

- 服务监听地址和端口
- 数据库连接
- JWT 配置
- 缓存后端
- 日志
- WebDAV 前缀和 payload 上限

### 运行时配置

`system_config` 表负责在线可调的运行时配置，不需要改 `config.toml`。

当前内置系统配置项：

| Key | 类型 | 作用 |
| --- | --- | --- |
| `webdav_enabled` | boolean | 控制 WebDAV 是否接受请求；关闭后返回 `503` |
| `max_versions_per_file` | number | 单文件最多保留多少历史版本 |
| `trash_retention_days` | number | 回收站自动清理窗口 |
| `default_storage_quota` | number | 新注册用户默认配额，单位字节 |

设置运行时配置：

```bash
curl -X PUT http://127.0.0.1:3000/api/v1/admin/config/trash_retention_days \
  -b cookies.txt \
  -H "Content-Type: application/json" \
  -d '{"value":"14"}'
```

补充说明：

- 系统配置会做类型校验
- 系统配置不允许删除
- 管理员可以创建自定义配置项，供插件或自定义前端使用
- 系统配置的 schema 可通过 `/api/v1/admin/config/schema` 读取

## WebDAV 锁管理

管理员可以查看并释放当前实例里的资源锁。

### 当前可做的事

- 列出全部锁
- 强制解锁单个资源
- 清理全部过期锁

清理过期锁：

```bash
curl -X DELETE http://127.0.0.1:3000/api/v1/admin/locks/expired \
  -b cookies.txt
```

强制解锁单个锁：

```bash
curl -X DELETE http://127.0.0.1:3000/api/v1/admin/locks/15 \
  -b cookies.txt
```

适用场景通常是某个 WebDAV 客户端异常退出，锁没有正常释放。需要注意的是，如果客户端还以为自己持有这把锁，强制释放后该客户端后续写入可能报错。

## 分享链接管理

管理员可以审计和删除全站分享。

### 当前可做的事

- 列出全部分享
- 查看文件分享或文件夹分享状态
- 查看分享是否过期，或是否达到下载次数上限
- 直接删除任意分享

删除一个分享：

```bash
curl -X DELETE http://127.0.0.1:3000/api/v1/admin/shares/9 \
  -b cookies.txt
```

删除后，公开链接会立即失效。
