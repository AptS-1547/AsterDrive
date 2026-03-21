# 分享 API

分享接口分成两类：

- 登录后管理自己的分享
- 公开访问分享内容

以下路径都相对于 `/api/v1`。

## 认证分享接口

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| `POST` | `/shares` | 创建分享 |
| `GET` | `/shares` | 列出当前用户创建的分享 |
| `DELETE` | `/shares/{id}` | 删除分享 |

### `POST /shares`

请求体：

```json
{
  "file_id": 1,
  "folder_id": null,
  "password": "123456",
  "expires_at": "2026-03-31T12:00:00Z",
  "max_downloads": 10
}
```

当前实现注意点：

- 只要求 `file_id` 和 `folder_id` 至少一个非空，不会强制互斥
- 为避免歧义，实际使用时应只传一个
- 同一资源只允许存在一个活跃分享；已过期旧分享会被自动删除
- `max_downloads = 0` 表示不限下载次数
- 密码为空字符串等价于“不设密码”

## 公开分享接口

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| `GET` | `/s/{token}` | 读取分享公开信息 |
| `POST` | `/s/{token}/verify` | 校验分享密码 |
| `GET` | `/s/{token}/download` | 下载分享文件 |
| `GET` | `/s/{token}/content` | 读取分享文件夹根层内容 |
| `GET` | `/s/{token}/thumbnail` | 获取分享文件缩略图 |

### `GET /s/{token}`

返回公开信息：

- 名称
- 分享类型：`file` 或 `folder`
- 是否有密码
- 过期时间
- 下载次数与浏览次数

### `POST /s/{token}/verify`

请求体：

```json
{ "password": "123456" }
```

成功后会写入一个 1 小时有效的 HttpOnly Cookie：

```text
aster_share_<token>
```

如果分享本身没有密码，这个接口会返回校验错误。

### `GET /s/{token}/download`

仅适用于文件分享。

如果分享受密码保护，则必须先完成 `/verify`。

下载次数限制也在这里扣减。

### `GET /s/{token}/content`

仅适用于文件夹分享。

当前实现只返回“分享根目录”的内容，不支持继续通过公开 API 下钻到任意子目录。

### `GET /s/{token}/thumbnail`

仅适用于图片文件分享。

如果分享受密码保护，同样要先完成 `/verify`。

## 前端公开页

对应的前端访问路径是：

```text
/s/:token
```

当前前端表现：

- 文件分享：展示下载按钮
- 文件夹分享：展示根目录内容，只读不下钻
