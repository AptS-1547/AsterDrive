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

## 公开访问

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| `GET` | `/s/{token}` | 读取分享公开信息 |
| `POST` | `/s/{token}/verify` | 校验分享密码 |
| `GET` | `/s/{token}/download` | 下载分享文件 |
| `GET` | `/s/{token}/content` | 读取分享文件夹根层内容 |
| `GET` | `/s/{token}/files/{file_id}/download` | 下载分享文件夹中的子文件 |
| `GET` | `/s/{token}/thumbnail` | 获取分享文件缩略图 |

其中：

- `/verify` 成功后会写入 1 小时有效的 `aster_share_<token>` Cookie
- `/download` 只适用于文件分享
- `/content` 只返回文件夹分享的根目录内容
- `/files/{file_id}/download` 用于下载分享文件夹树中的子文件
- `/thumbnail` 只适用于图片文件分享

当前边界直接记一句就够：

- 公开页支持根目录浏览
- 支持根目录中展示出来的文件下载与预览
- 仍不支持继续进入子文件夹浏览

前端公开页路径是：

```text
/s/:token
```
