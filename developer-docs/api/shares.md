# 分享 API

分享接口分成两块：自己管理分享，以及公开访问分享内容。

以下路径都相对于 `/api/v1`。

## 自己的分享

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| `POST` | `/shares` | 创建分享 |
| `GET` | `/shares` | 列出当前用户创建的分享 |
| `DELETE` | `/shares/{id}` | 删除分享 |

创建请求示例：

```json
{
  "file_id": 1,
  "folder_id": null,
  "password": "123456",
  "expires_at": "2026-03-31T12:00:00Z",
  "max_downloads": 10
}
```

要点：

- `file_id` 和 `folder_id` 至少一个非空；实际使用时只传一个更清楚
- 同一资源同一时间只允许一个活跃分享
- `max_downloads = 0` 表示不限次数
- 空密码等价于不设密码
- `GET /shares` 现在是分页接口，支持 `limit` 和 `offset`

## 公开访问

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| `GET` | `/s/{token}` | 读取分享公开信息 |
| `POST` | `/s/{token}/verify` | 校验分享密码 |
| `GET` | `/s/{token}/download` | 下载分享文件 |
| `GET` | `/s/{token}/content` | 读取分享文件夹根层内容 |
| `GET` | `/s/{token}/folders/{folder_id}/content` | 浏览分享目录树中的子目录 |
| `GET` | `/s/{token}/files/{file_id}/download` | 下载分享文件夹中的子文件 |
| `GET` | `/s/{token}/thumbnail` | 获取分享文件缩略图 |
| `GET` | `/s/{token}/files/{file_id}/thumbnail` | 获取分享目录树中子文件的缩略图 |
| `GET` | `/s/{token}/avatar/{size}` | 获取分享拥有者头像 |

其中：

- `/verify` 成功后会写入 1 小时有效的 `aster_share_<token>` Cookie
- `/download` 只适用于文件分享
- `/content` 只返回文件夹分享的根目录内容
- `/folders/{folder_id}/content` 用于继续浏览分享目录树中的子目录
- `/files/{file_id}/download` 用于下载分享文件夹树中的子文件
- `/thumbnail` 只适用于图片文件分享
- `/files/{file_id}/thumbnail` 只适用于分享目录树中的图片文件
- `/avatar/{size}` 返回分享拥有者头像，当前支持 `512` 和 `1024`

当前边界直接记一句就够：

- 公开页已经支持在分享目录树内继续进入子文件夹浏览
- 子目录访问、子文件下载和子文件缩略图都会校验是否仍处在分享根目录范围内
- 越过分享范围访问其他目录或文件会返回 `403`

前端公开页路径是：

```text
/s/:token
```
