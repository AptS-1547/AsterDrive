# 存储策略

存储策略不在 `config.toml` 里，而是持久化在数据库中，通过 [管理 API](/api/admin) 和管理面板维护。

## 作用

`StoragePolicy` 决定：

- 文件存储在哪个后端
- 上传时使用哪种模式
- 单文件大小上限
- 根目录或对象前缀

首次启动时，如果系统里还没有任何策略，服务会自动创建默认本地策略。

## 支持的驱动

| 类型 | 说明 |
| --- | --- |
| `local` | 本地文件系统 |
| `s3` | S3 兼容对象存储 |

## 生效顺序

```text
文件夹 policy_id -> 用户默认策略 -> 系统默认策略
```

## 当前重要字段

| 字段 | 说明 |
| --- | --- |
| `driver_type` | `local` 或 `s3` |
| `endpoint` | S3 兼容服务地址；本地策略可为空 |
| `bucket` | S3 bucket 名称 |
| `base_path` | 本地目录或对象前缀 |
| `max_file_size` | 单文件大小上限；`0` 表示不限制 |
| `chunk_size` | 分片大小；`0` 表示禁用分片上传 |
| `is_default` | 是否为系统默认策略 |
| `options` | JSON 对象；当前只识别 `presigned_upload` |

## 三种上传模式的决策

`POST /api/v1/files/upload/init` 会根据当前策略返回：

- `direct`
- `chunked`
- `presigned`

规则如下：

### `direct`

- `chunk_size == 0`
- 或文件大小 `<= chunk_size`

### `chunked`

- 文件大小 `> chunk_size`
- 且当前不是可用的 S3 预签名直传场景

### `presigned`

只有同时满足这些条件才会返回：

- 当前策略驱动是 `s3`
- `options` 含 `{"presigned_upload": true}`
- 文件大小不超过 5 GiB

## `options` 的当前有效键

```json
{
  "presigned_upload": true
}
```

启用后，前端和 API 都会协商出 `presigned` 上传模式。

## 默认本地策略

自动创建的默认策略具有这些特征：

- 名称：`Local Default`
- 驱动：`local`
- 路径：`data/uploads`
- 默认分片大小：`5 MiB`

## 当前 API 限制

这些限制都来自当前实现本身：

- `POST /api/v1/admin/policies` 虽然请求体带 `chunk_size`，但创建逻辑仍会先写固定值 `5 MiB`
- 若要调整 `chunk_size`，需要创建后再 `PATCH`
- 当前 `PATCH /api/v1/admin/policies/{id}` 不能修改 `driver_type`
- `allowed_types` 字段已经在模型中存在，但当前 REST API 没有管理它，上传链路也没有执行类型限制
