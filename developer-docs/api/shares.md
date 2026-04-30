# 分享 API

分享接口分成两块：自己管理分享，以及公开访问分享内容。

以下路径都相对于 `/api/v1`。

## 自己的分享

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| `POST` | `/shares` | 创建分享 |
| `GET` | `/shares` | 列出当前用户创建的分享 |
| `PATCH` | `/shares/{id}` | 编辑已有分享 |
| `DELETE` | `/shares/{id}` | 删除分享 |
| `POST` | `/shares/batch-delete` | 批量删除分享 |

创建请求示例：

```json
{
  "target": {
    "type": "file",
    "id": 1
  },
  "password": "123456",
  "expires_at": "2026-03-31T12:00:00Z",
  "max_downloads": 10
}
```

要点：

- `target.type` 只能是 `file` 或 `folder`，`target.id` 是对应资源 ID
- 当前请求体已经不再接受旧的顶层 `file_id` / `folder_id`
- 同一资源同一时间只允许一个活跃分享
- `max_downloads = 0` 表示不限次数
- 空密码等价于不设密码
- `GET /shares` 现在是分页接口，支持 `limit` 和 `offset`

编辑请求示例：

```json
{
  "password": "new-secret",
  "expires_at": "2026-04-02T12:00:00Z",
  "max_downloads": 5
}
```

编辑语义：

- `password` 不传：保留现有密码
- `password = ""`：移除密码
- `password = "xxx"`：替换为新密码
- `expires_at = null`：改为永不过期
- `max_downloads = 0`：改为不限次数

批量删除请求示例：

```json
{
  "share_ids": [1, 2, 3]
}
```

批量删除行为：

- 单次总项目数上限是 1000
- 每个 share 独立执行，不会因为一个失败而整批回滚
- 返回结果使用和其他 batch 接口一致的 `BatchResult` 结构

## 公开访问

下面这组 `/s/...` 仍然是“相对于 `/api/v1`”的 REST 路径，也就是完整地址实际是 `/api/v1/s/{token}/*`。
前端公开页面路由才是根路径 `/s/:token`。

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| `GET` | `/s/{token}` | 读取分享公开信息 |
| `POST` | `/s/{token}/verify` | 校验分享密码 |
| `POST` | `/s/{token}/preview-link` | 为分享文件生成短期预览链接 |
| `GET` | `/s/{token}/download` | 下载分享文件 |
| `GET` | `/s/{token}/content` | 读取分享文件夹根层内容 |
| `GET` | `/s/{token}/folders/{folder_id}/content` | 浏览分享目录树中的子目录 |
| `GET` | `/s/{token}/files/{file_id}/download` | 下载分享文件夹中的子文件 |
| `POST` | `/s/{token}/files/{file_id}/preview-link` | 为分享目录树中的子文件生成短期预览链接 |
| `GET` | `/s/{token}/thumbnail` | 获取分享文件缩略图 |
| `GET` | `/s/{token}/files/{file_id}/thumbnail` | 获取分享目录树中子文件的缩略图 |
| `GET` | `/s/{token}/avatar/{size}` | 获取分享拥有者已上传头像 |

其中：

- `/verify` 成功后会写入 1 小时有效的 `aster_share_<token>` Cookie
- `/preview-link` 和 `/files/{file_id}/preview-link` 也会校验这枚 Cookie；受密码保护的分享必须先过 `/verify`
- `/download` 只适用于文件分享
- `/preview-link` 只适用于文件分享；返回的 `PreviewLinkInfo.path` 最终指向根路径 `/pv/{token}/{filename}`
- `/content` 只返回文件夹分享的根目录内容
- `/folders/{folder_id}/content` 用于继续浏览分享目录树中的子目录
- `/files/{file_id}/download` 用于下载分享文件夹树中的子文件
- `/files/{file_id}/preview-link` 用于分享目录树里子文件的短期预览
- `/thumbnail` 只适用于服务端当前支持生成缩略图的文件分享
- `/files/{file_id}/thumbnail` 只适用于分享目录树中服务端当前支持生成缩略图的文件
- `/avatar/{size}` 只返回分享拥有者“已上传头像”的二进制资源，当前支持 `512` 和 `1024`

文件夹分享的两个内容接口还支持和普通目录列表一致的参数：

- `folder_limit` / `folder_offset`
- `file_limit`
- `sort_by` / `sort_order`
- `file_after_value` / `file_after_id`

返回体同样会带 `next_file_cursor`。

当前边界直接记一句就够：

- 公开页已经支持在分享目录树内继续进入子文件夹浏览
- 子目录访问、子文件下载和子文件缩略图都会校验是否仍处在分享根目录范围内
- 子文件预览链接也会校验是否仍处在分享根目录范围内
- 越过分享范围访问其他目录或文件会返回 `403`
- 如果拥有者当前头像来源是 `gravatar` 或 `none`，前端应直接使用 `GET /s/{token}` 返回的 `shared_by.avatar.url_*`

前端公开页路径是：

```text
/s/:token
```
