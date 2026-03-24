# 文件夹 API

以下路径都相对于 `/api/v1`，且都需要认证。

## 一览

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| `GET` | `/folders` | 列出根目录内容 |
| `POST` | `/folders` | 创建文件夹 |
| `GET` | `/folders/{id}` | 列出指定文件夹内容 |
| `PATCH` | `/folders/{id}` | 重命名、移动、设置策略 |
| `DELETE` | `/folders/{id}` | 软删除文件夹 |
| `POST` | `/folders/{id}/lock` | 简化锁定 / 解锁 |
| `POST` | `/folders/{id}/copy` | 递归复制文件夹 |

## 目录读取

- `GET /folders`：读取根目录内容
- `GET /folders/{id}`：读取指定目录内容

目录列表会过滤一批常见系统垃圾文件名，例如 `._*`、`~$*`、`.DS_Store`。

## 创建与修改

创建请求很简单：

```json
{
  "name": "Documents",
  "parent_id": null
}
```

`parent_id = null` 表示在根目录创建。

`PATCH /folders/{id}` 当前支持三件事：

- 重命名
- 移动到其他父目录
- 设置目录级存储策略覆盖

同时会做这些校验：

- 不能移动到自己下面或子孙目录下面
- 目标位置同名会报错
- 被锁定文件夹不能修改

当前限制：

- `parent_id = null` 还不能表达“移回根目录”
- `policy_id = null` 也不能表达“清除策略覆盖”

## 删除、锁和复制

- `DELETE /folders/{id}`：软删除，会递归进入回收站
- `POST /folders/{id}/lock`：`locked = true` 加锁，`locked = false` 解锁
- `POST /folders/{id}/copy`：递归复制目录树，成功返回 `201`

复制时底层文件内容不会物理复制，只增加 Blob 引用计数；目标位置同名会自动生成副本名。

当前复制限制：

- `parent_id = null` 不能表达“复制到根目录”
- 新目录树不会继承源目录上的 `policy_id`
