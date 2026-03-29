# 批量操作 API

以下路径都相对于 `/api/v1`，且都需要认证。

## 接口列表

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| `POST` | `/batch/delete` | 批量删除文件和文件夹 |
| `POST` | `/batch/move` | 批量移动文件和文件夹 |
| `POST` | `/batch/copy` | 批量复制文件和文件夹 |

## 请求体结构

三组接口都使用混合资源请求体：

```json
{
  "file_ids": [1, 2],
  "folder_ids": [10, 11]
}
```

其中：

- `file_ids` 和 `folder_ids` 可以同时存在
- 单次总项目数上限是 1000
- 每个条目独立执行，不会因为一个失败就让整批全部回滚

## 返回结果

批量接口都会返回 `BatchResult` 风格的数据，包含：

- `succeeded`
- `failed`
- `errors`

这也是前端批量操作条和批量 toast 汇总提示的依据。

## `POST /batch/delete`

行为：

- 文件和文件夹会走和单项删除一致的软删除逻辑
- 删除结果逐项统计
- 某一项失败不会阻断其他项继续执行

## `POST /batch/move`

请求体还会带目标目录：

```json
{
  "file_ids": [1, 2],
  "folder_ids": [10],
  "target_folder_id": 99
}
```

行为：

- 支持把文件和文件夹一起移动到同一个目标目录
- `target_folder_id = null` 表示移动到根目录
- 前端拖拽移动和批量移动共用这类能力

## `POST /batch/copy`

请求体还会带目标目录：

```json
{
  "file_ids": [1],
  "folder_ids": [10],
  "target_folder_id": 99
}
```

行为：

- 文件复制不会物理复制 Blob，只增加引用计数
- 文件夹复制会递归复制目录树
- 与单项复制一样，目标位置同名时会自动生成副本名

## 使用场景

这组接口主要服务当前前端已经实现的：

- 多选批量删除
- 多选批量复制
- 多选批量移动
- 拖拽多个项目一起移动

## 相关文档

- [文件 API](/api/files)
- [文件夹 API](/api/folders)
- [核心流程](/guide/core-workflows)
