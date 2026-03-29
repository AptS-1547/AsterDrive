# 存储策略

存储策略在管理后台的 `管理 -> 存储策略` 页面里维护，不在 `config.toml` 里。  
它决定文件真正存到哪里，也决定上传时走哪种方式。

## 一条策略会决定什么

- 文件最终存到哪里
- 上传时使用哪种模式
- 单文件大小上限
- 本地目录或对象存储前缀

## 支持的驱动

| 类型 | 说明 |
| --- | --- |
| `local` | 本地文件系统 |
| `s3` | S3 兼容对象存储 |

## 管理后台里最常见的字段

| 字段 | 说明 |
| --- | --- |
| `name` | 策略名称 |
| `driver_type` | `local` 或 `s3` |
| `endpoint` | S3 兼容服务地址；本地策略留空 |
| `bucket` | S3 存储桶名称 |
| `base_path` | 本地目录或对象前缀 |
| `max_file_size` | 单文件大小上限，单位字节；`0` 表示不限制 |
| `chunk_size` | 分片大小，管理后台里按 MB 输入；`0` 表示尽量单次上传 |
| `options` | 仅 S3 主要用它控制上传策略；常见值见下文 |
| `is_default` | 是否为系统默认策略 |

## S3 上传策略

S3 策略现在用 `options.s3_upload_strategy` 控制上传传输方式，可选值：

- `proxy_tempfile`：浏览器先把文件传给 AsterDrive，服务端先写本地临时文件/分片目录，再写入 S3。兼容性最好，但会占用本地磁盘
- `relay_stream`：浏览器先把文件传给 AsterDrive，服务端直接把字节流中继到 S3，不落本地临时文件；该模式不做 SHA256 去重
- `presigned`：浏览器直接上传到 S3 / MinIO；同样不做 SHA256 去重，并且要求对象存储配置好 CORS

补充说明：

- `chunk_size` 对 `relay_stream` 和 `presigned` 也会控制 S3 multipart 的 part 大小
- S3 multipart 仍然受 5 MiB 最小 part 大小约束；即使你把 `chunk_size` 设得更小，实际 part 大小也会被抬到 5 MiB
- 旧配置 `{"presigned_upload":true}` 仍兼容，等价于 `{"s3_upload_strategy":"presigned"}`
- 旧配置 `{"presigned_upload":false}` 或未配置该字段时，默认等价于 `{"s3_upload_strategy":"proxy_tempfile"}`

## 默认会怎么用

新部署实例第一次启动后，会自动创建一条默认本地策略：

- 名称：`Local Default`
- 驱动：`local`
- 路径：`data/uploads`
- 默认分片大小：`5 MiB`

新用户注册后，会自动分配当前默认策略。  
如果你需要区分不同用户的落盘位置或配额，再到 `管理 -> 用户` 里给用户分配额外策略并设置默认策略。

## 本地存储和 S3 怎么选

### 选 `local`

适合单机、NAS 和文件直接落本地磁盘的场景。

### 选 `s3`

适合文件放到 MinIO、AWS S3 或其他兼容对象存储的场景。

## 使用时一定注意这几件事

- 开启 `presigned` 后，对象存储必须配置浏览器上传所需的 CORS
- 使用 `relay_stream` 或 `presigned` 时，服务端不会回读对象计算 SHA256，所以不会做 Blob 去重
- 使用本地存储时，建议 `base_path` 写绝对路径，避免以后因为工作目录变化找错位置
- 使用 S3 时，要确认桶、访问密钥和前缀都正确
- 单文件大小限制和用户配额都会影响上传是否成功
- 已经有文件写入的策略，不要直接修改 `base_path`、`bucket` 或 `endpoint`。旧文件仍然会按原策略 ID 读取，直接改位置会导致已有文件找不到
