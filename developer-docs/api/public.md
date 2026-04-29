# 公共接口

这组路径都相对于 `/api/v1`，且不需要认证。

其中前两条主要给匿名页面启动用，后两条用于 primary 和 follower 之间的远端节点 enrollment 握手。

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| `GET` | `/public/branding` | 读取登录页、公开页和匿名入口需要的品牌配置 |
| `GET` | `/public/preview-apps` | 读取匿名态可见的预览应用注册表 |
| `POST` | `/public/remote-enrollment/redeem` | follower 用 enrollment token 兑换远端节点绑定信息 |
| `POST` | `/public/remote-enrollment/ack` | follower 确认 enrollment 已完成 |

## `GET /public/branding`

返回仍然使用统一 JSON 包装：

```json
{
  "code": 0,
  "msg": "",
  "data": {
    "title": "AsterDrive",
    "description": "Self-hosted cloud storage",
    "favicon_url": "/favicon.svg",
    "wordmark_dark_url": "/static/asterdrive/asterdrive-dark.svg",
    "wordmark_light_url": "/static/asterdrive/asterdrive-light.svg",
    "site_urls": ["https://drive.example.com", "https://panel.example.com"],
    "allow_user_registration": true
  }
}
```

字段含义：

- `title` / `description`：公开页面展示文案
- `favicon_url`：站点图标
- `wordmark_dark_url` / `wordmark_light_url`：亮暗背景下使用的品牌字标
- `site_urls`：当前对外公开站点来源列表；未配置时为空数组
- `allow_user_registration`：匿名页是否应展示注册入口

当前前端登录页和公开入口会先拉这条接口，再决定匿名态 UI，而不是把这些值硬编码进前端构建产物。

`site_urls` 对应运行时配置 key 仍然是 `public_site_url`。管理接口把它作为 `string_array` 暴露，写入时必须传 JSON 字符串数组；服务端保存前会规范化每一项。每一项必须是精确 HTTP(S) origin，不能包含路径、通配符或非 HTTP(S) scheme。

## `GET /public/preview-apps`

这条接口同样返回统一 JSON 包装，`data` 里是一个公开可见的预览应用注册表：

```json
{
  "code": 0,
  "msg": "",
  "data": {
    "version": 2,
    "apps": [
      {
        "key": "builtin.formatted",
        "provider": "builtin",
        "icon": "/static/preview-apps/json.svg",
        "enabled": true,
        "labels": {
          "en": "Formatted view",
          "zh": "格式化视图"
        },
        "extensions": ["json", "xml"]
      }
    ]
  }
}
```

要点：

- `apps` 是当前匿名页面可见的预览器定义；`provider` 目前有 `builtin`、`url_template`、`wopi`
- 当前是 v2 结构，不再返回顶层 `rules`；匹配信息直接挂在每个 app 自己的 `extensions` 与 `config` 上
- 返回结果已经过滤掉被禁用的 app
- `config` 是 provider 相关配置：
  - `url_template` 预览器常见字段有 `mode`、`url_template`、`allowed_origins`
  - `wopi` 预览器常见字段有 `mode`、`action` / `action_url` / `action_url_template`、`discovery_url`
- 前端文件预览、公开分享预览和 WOPI 集成入口都会依赖这份注册表，而不是把预览器信息硬编码在前端里
- 管理员当前可以通过 `/api/v1/admin/config/frontend_preview_apps_json` 维护这份注册表

## `POST /public/remote-enrollment/redeem`

这条接口不是给匿名网页用的，而是给 follower CLI enrollment 流程用的。

请求体：

```json
{
  "token": "enr_xxxxx"
}
```

成功后返回绑定引导数据：

```json
{
  "code": 0,
  "msg": "",
  "data": {
    "remote_node_id": 7,
    "remote_node_name": "edge-sh-01",
    "master_url": "https://drive.example.com",
    "access_key": "rk_xxx",
    "secret_key": "rs_xxx",
    "namespace": "remote/edge-sh-01",
    "is_enabled": true,
    "ack_token": "enr_ack_xxx"
  }
}
```

要点：

- `master_url` 要求主节点已经配置 `public_site_url`；多来源配置时使用第一项作为 enrollment 主控地址
- `access_key` / `secret_key` / `namespace` 是后续内部存储协议要用的绑定信息
- 兑换成功后，并不代表 enrollment 已彻底完成；follower 还需要继续调用 ack 接口

## `POST /public/remote-enrollment/ack`

同样是 enrollment 流程专用接口。

请求体：

```json
{
  "ack_token": "enr_ack_xxx"
}
```

成功时返回空的统一成功响应：

```json
{
  "code": 0,
  "msg": ""
}
```

语义上，这表示 follower 已经拿到绑定信息，并向主节点确认这次 enrollment 会话可以结束。
