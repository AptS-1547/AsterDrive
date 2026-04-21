# 内部存储协议（Follower）

这组接口是主节点和 follower 节点之间的内部对象存储协议，不是给浏览器前端或第三方普通客户端用的公开 API。

以下路径都相对于：

```text
/api/v1/internal/storage
```

并且只会在 `follower` 节点注册。

## 认证方式

当前有两种访问方式：

- 主节点签名请求
  - `x-aster-access-key`
  - `x-aster-timestamp`
  - `x-aster-nonce`
  - `x-aster-signature`
- 预签名 query
  - `aster_access_key`
  - `aster_expires`
  - `aster_signature`

常规控制面接口都要求签名头；对象 GET / PUT 会按场景支持预签名 URL。

## 接口列表

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| `GET` | `/capabilities` | 读取 follower 声明的协议能力 |
| `PUT` | `/binding` | 同步主节点维护的远端节点绑定信息 |
| `POST` | `/compose` | 把多个 part 对象拼成目标对象 |
| `GET` | `/objects` | 按前缀列举对象 key |
| `GET` | `/objects/{tail}/metadata` | 读取对象元信息 |
| `PUT` | `/objects/{tail}` | 上传对象内容 |
| `GET` | `/objects/{tail}` | 读取对象内容 |
| `HEAD` | `/objects/{tail}` | 探测对象是否存在并返回头信息 |
| `DELETE` | `/objects/{tail}` | 删除对象 |

## `GET /capabilities`

返回仍然走统一 JSON 包装，典型字段包括：

- `protocol_version`
- `supports_list`
- `supports_range_read`
- `supports_stream_upload`

当前默认协议版本是 `v1`。

## `PUT /binding`

主节点会用这条接口把 follower 绑定信息同步过去，请求体字段包括：

- `name`
- `namespace`
- `is_enabled`

这条接口只更新绑定元信息，不直接搬运对象数据。

## `POST /compose`

这条接口用于把多个上传 part 合成为最终对象，请求体包括：

- `target_key`
- `part_keys`
- `expected_size`

成功后返回 `bytes_written`。实现上会在拼接成功后清理被消费的 part 对象。

## 对象读写

### `PUT /objects/{tail}`

写入一个对象。请求必须带 `Content-Length`，follower 会按 ingress 策略检查对象大小上限。

### `GET /objects/{tail}`

返回原始对象字节流，不走 JSON 包装。

可选 query：

- `offset`
- `length`
- `response-cache-control`
- `response-content-disposition`
- `response-content-type`

也就是说，这条接口既支持整对象读取，也支持范围读取和响应头覆写。

### `HEAD /objects/{tail}`

返回对象是否存在以及基础响应头，常用于轻量探测。

### `GET /objects/{tail}/metadata`

返回统一 JSON 包装，`data` 里当前主要有：

- `size`
- `content_type`

### `DELETE /objects/{tail}`

删除对象，成功时返回空的统一成功响应。

## 列举

### `GET /objects`

支持 `prefix` query，返回匹配前缀下的对象 key 列表。

当前返回体里的 `items` 是 follower 绑定命名空间下的相对 key，不会把 provider 内部前缀原样暴露回去。

## 什么时候看这页

下面这些情况，不要再去普通 `files` / `upload` / `shares` 路由里瞎找：

- 主节点写远端存储节点失败
- 受管 follower 拼 part 失败
- 远端节点健康正常，但对象列举 / 读取 / 删除异常
- 远端节点 enrollment 成功后，后续对象同步行为不对
