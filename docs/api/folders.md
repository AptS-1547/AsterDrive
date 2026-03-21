# 文件夹 API

以下路径都相对于 `/api/v1`，且都需要认证。

## 接口列表

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| `GET` | `/folders` | 列出根目录内容 |
| `POST` | `/folders` | 创建文件夹 |
| `GET` | `/folders/{id}` | 列出指定文件夹内容 |
| `PATCH` | `/folders/{id}` | 重命名、移动、设置策略 |
| `DELETE` | `/folders/{id}` | 软删除文件夹 |
| `POST` | `/folders/{id}/lock` | 简化锁定 / 解锁 |
| `POST` | `/folders/{id}/copy` | 递归复制文件夹 |

## `GET /folders`

返回根目录内容：

```json
{
  "folders": [],
  "files": []
}
```

当前认证态目录列表会过滤一批常见的系统垃圾文件名，例如：

- `._*`
- `~$*`
- `.DS_Store`

## `POST /folders`

请求体：

```json
{
  "name": "Documents",
  "parent_id": null
}
```

`parent_id = null` 表示在根目录创建。

## `GET /folders/{id}`

读取指定文件夹下的子文件夹和文件列表。

## `PATCH /folders/{id}`

请求体：

```json
{
  "name": "New Name",
  "parent_id": 3,
  "policy_id": 2
}
```

当前实现支持：

- 重命名
- 移动到其他父目录
- 给目录设置存储策略覆盖

额外校验：

- 不能把文件夹移动到自己下面
- 不能把文件夹移动到自己的子孙目录下
- 目标位置出现同名文件夹会报错
- 被锁定文件夹不能修改

当前限制：

- `parent_id = null` 与“不传”在现有接口里等价，因此无法通过 PATCH 把文件夹移动回根目录
- `policy_id = null` 也无法表达“清除策略覆盖”；当前只能设置、不能通过此接口清空

## `DELETE /folders/{id}`

删除是软删除，会递归标记子文件和子文件夹进入回收站。

## `POST /folders/{id}/lock`

请求体：

```json
{ "locked": true }
```

## `POST /folders/{id}/copy`

请求体：

```json
{ "parent_id": 10 }
```

当前行为：

- 递归复制整个目录树
- 底层文件内容不会物理复制，只增加 Blob 引用计数
- 若目标位置同名，会自动生成 `Folder (1)` 这类名称

当前限制与实现细节：

- `parent_id = null` 当前表示“沿用原父目录”，不能通过此接口复制到根目录
- 复制出来的新文件夹不会继承源文件夹的 `policy_id`，新的目录树默认没有文件夹级策略覆盖
