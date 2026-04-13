# 公共接口

这组路径都相对于 `/api/v1`，且不需要认证。

目前公开给匿名页面启动用的接口有两条：

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| `GET` | `/public/branding` | 读取登录页、公开页和匿名入口需要的品牌配置 |
| `GET` | `/public/preview-apps` | 读取匿名态可见的预览应用注册表 |

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
    "site_url": "https://drive.example.com",
    "allow_user_registration": true
  }
}
```

字段含义：

- `title` / `description`：公开页面展示文案
- `favicon_url`：站点图标
- `wordmark_dark_url` / `wordmark_light_url`：亮暗背景下使用的品牌字标
- `site_url`：当前对外公开站点地址；未配置时可能为 `null`
- `allow_user_registration`：匿名页是否应展示注册入口

当前前端登录页和公开入口会先拉这条接口，再决定匿名态 UI，而不是把这些值硬编码进前端构建产物。

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
