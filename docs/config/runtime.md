# 运行时配置

运行时配置存放在数据库 `system_config` 表，而不是 `config.toml`。

特点：

- 由管理员通过 API 或管理面板在线修改
- 值统一以字符串形式存储
- 适合放“无需重启即可调整”的策略项

## 当前真正生效的配置项

| Key | 默认值 | 作用 |
| --- | --- | --- |
| `default_storage_quota` | `"0"` | 新注册用户的默认总配额，`0` 表示不限制 |
| `webdav_enabled` | `"true"` | 控制 WebDAV 是否接受请求 |
| `trash_retention_days` | `"7"` | 回收站保留天数，后台每小时清理一次 |
| `max_versions_per_file` | `"10"` | 单文件最多保留多少历史版本 |

## 生效时机说明

### `default_storage_quota`

- 只在“新用户注册”时读取
- 不会自动修改已有用户的 `storage_quota`

### `webdav_enabled`

- 关闭后 `/webdav` 返回 `503`
- 不需要重启

### `trash_retention_days`

- 由后台任务每小时读取并执行一次清理

### `max_versions_per_file`

- 在文件被覆盖并产生新历史版本时生效

## 管理方式

### 读取全部配置

```bash
curl -X GET http://localhost:3000/api/v1/admin/config \
  -b cookies.txt
```

### 设置单个 key

```bash
curl -X PUT http://localhost:3000/api/v1/admin/config/trash_retention_days \
  -b cookies.txt \
  -H "Content-Type: application/json" \
  -d '{"value":"14"}'
```

### 删除单个 key

```bash
curl -X DELETE http://localhost:3000/api/v1/admin/config/max_versions_per_file \
  -b cookies.txt
```

删除后逻辑会回退到代码里的默认值。

## 当前未接线的配置项

`src/config/schema.rs` 的注释里提到过 `webdav_max_upload_size`，但当前代码并没有真正读取它。

如果你要限制 WebDAV 上传，当前应优先使用：

- 静态配置 `webdav.payload_limit`
- 存储策略里的 `max_file_size`
