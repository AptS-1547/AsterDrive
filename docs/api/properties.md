# 属性 API

属性接口用于给文件或文件夹挂载自定义键值对。

以下路径都相对于 `/api/v1`，且都需要认证。

## 接口列表

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| `GET` | `/properties/{entity_type}/{entity_id}` | 列出实体属性 |
| `PUT` | `/properties/{entity_type}/{entity_id}` | 新增或更新属性 |
| `DELETE` | `/properties/{entity_type}/{entity_id}/{namespace}/{name}` | 删除属性 |

其中 `entity_type` 只能是：

- `file`
- `folder`

## `GET /properties/{entity_type}/{entity_id}`

返回该实体的全部属性数组。

## `PUT /properties/{entity_type}/{entity_id}`

请求体：

```json
{
  "namespace": "custom",
  "name": "color",
  "value": "blue"
}
```

`value` 可以为 `null`。

## `DELETE /properties/{entity_type}/{entity_id}/{namespace}/{name}`

删除指定属性。

## 只读命名空间

当前实现禁止通过 REST 改写 `DAV:` 命名空间。

也就是说：

- `namespace = "DAV:"` 不能 `PUT`
- `namespace = "DAV:"` 不能 `DELETE`

这是为了避免 REST 接口破坏 WebDAV 协议属性。
