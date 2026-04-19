# 自定义前端

::: tip 这一篇讲什么
AsterDrive 的前端是可替换的：官方前端嵌进了二进制里，但你可以用**自己的前端资源**覆盖掉它。本篇讲覆盖机制、index.html 里的占位符、用"自定义配置"当全局变量持久化层，以及 CSP 限制。
面向的是**想替换或魔改前端**的开发者，不是日常用户或管理员。
:::

## 覆盖机制

AsterDrive 所有前端路由（首页、`/assets/*`、`/static/*`、`/pdfjs/*`、`/favicon.svg`、PWA 文件、SPA fallback）都走同一个加载顺序：

1. **先看当前工作目录下的 `./frontend-override/`** —— 有就用这个
2. **找不到再回退到嵌入的官方前端**（编译进二进制）

也就是说，你只需要把自己的前端产物放进 `./frontend-override/`，AsterDrive 就会**优先**从这里加载所有资源，不需要重新编译二进制。

::: warning 相对当前工作目录
`./frontend-override/` 是**相对启动时的工作目录**解析的，不是相对二进制位置：

- 本地直接运行 —— 项目根目录下的 `frontend-override/`
- systemd —— `WorkingDirectory/frontend-override/`
- Docker —— 容器里的 `/frontend-override/`（默认工作目录是 `/`，需要手动挂载到这里）

Docker 里最省事的做法是挂卷：`-v /path/to/my-dist:/frontend-override:ro`
:::

覆盖是**按文件级**的：你自己的 `dist/` 里有什么就用什么，没有的继续回退到官方嵌入版。所以你只替换 `index.html` + 部分 assets，其他继续用官方的，也行。

## index.html 支持的占位符

加载 `index.html` 时，AsterDrive 会在返回给浏览器前替换下面这些字符串：

| 占位符 | 来源 | 说明 |
| --- | --- | --- |
| `%ASTERDRIVE_VERSION%` | 二进制版本 | 编译期的 `CARGO_PKG_VERSION` |
| `%ASTERDRIVE_TITLE%` | 运行时配置 | `站点标题`（后台 `站点配置` 里维护） |
| `%ASTERDRIVE_DESCRIPTION%` | 运行时配置 | `站点描述` |
| `%ASTERDRIVE_FAVICON_URL%` | 运行时配置 | `favicon` 地址 |
| `%ASTERDRIVE_CSP%` | 常量 | 页面基线 `Content-Security-Policy` |

所有替换值会做 HTML 实体转义，所以直接塞进 `<title>` / `<meta>` 是安全的。

典型用法：

```html
<!DOCTYPE html>
<html lang="zh-CN">
<head>
  <meta charset="UTF-8" />
  <meta http-equiv="Content-Security-Policy" content="%ASTERDRIVE_CSP%" />
  <title>%ASTERDRIVE_TITLE%</title>
  <meta name="description" content="%ASTERDRIVE_DESCRIPTION%" />
  <link rel="icon" href="%ASTERDRIVE_FAVICON_URL%" />
  <meta name="generator" content="AsterDrive %ASTERDRIVE_VERSION%" />
</head>
<body>
  <div id="app"></div>
  <script type="module" src="/assets/index.js"></script>
</body>
</html>
```

## 用"自定义配置"持久化全局变量

你的前端多半需要一些全站级别的持久化配置——主题色、品牌名、第三方凭据、开关等。AsterDrive 提供了 `自定义配置`（`system_config` 表里 `source="custom"` 的条目）作为**官方推荐的持久化层**。

**命名约定**：`{namespace}.{name}`

| 用途 | 示例 key |
| --- | --- |
| 你自定义前端的主题色 | `my-frontend.theme.primary_color` |
| 某个功能开关 | `my-frontend.feature.enable_xxx` |
| 第三方接入地址 | `my-frontend.integration.xxx_api_url` |
| 客户侧品牌文案 | `my-frontend.brand.slogan` |

`namespace` 用你前端的标识（最好带 `-`），避免和官方系统配置或其他自定义前端冲突。

::: warning 不要用 `wopi.` / `auth.` / `mail.` 这种前缀
这些前缀可能被系统配置的新版本占用。`my-frontend.` / `acme-panel.` 这种私有 namespace 最稳。
:::

### 读写 API

自定义配置和系统配置走**同一套 Admin API**（区别只是 `source` 字段）：

| 操作 | 端点 |
| --- | --- |
| 列出所有配置（分页） | `GET /api/v1/admin/config` |
| 读单个 key | `GET /api/v1/admin/config/{key}` |
| 写入 / 更新 | `PUT /api/v1/admin/config/{key}` body `{"value": "..."}` |
| 删除 | `DELETE /api/v1/admin/config/{key}` |

::: tip 当前权限边界
Admin 配置接口**只对管理员开放**。想让普通用户也能读到某些自定义配置（比如主题色），当前只能：

1. 在 index.html 里用占位符把值预注入（需要扩展后端占位符集，还没开放）
2. 或者让你的前端读登录后的管理员专属接口

如果你需要"公开只读"的配置通道，可以在 [GitHub Issues](https://github.com/AptS-1547/AsterDrive/issues) 里提需求，我们愿意一起设计。
:::

### 从 CLI 批量操作

运维 CLI 也支持自定义配置——`list` / `get` / `set` / `delete` / `validate` / `export` / `import` 全部通用。详见 [运维 CLI](/deployment/ops-cli)。

典型场景：

```bash
# 在停机窗口批量写入你自定义前端的配置
./aster_drive config \
  --database-url "sqlite:///var/lib/asterdrive/data/asterdrive.db?mode=rwc" \
  import \
  --input-file ./my-frontend-config.json
```

输入文件示例：

```json
[
  { "key": "my-frontend.theme.primary_color", "value": "#6366f1" },
  { "key": "my-frontend.feature.enable_beta_tab", "value": "true" }
]
```

## CSP 限制

AsterDrive 对所有前端响应强制附加了一份基线 `Content-Security-Policy`，关键约束：

- `default-src 'self'` —— 默认只允许同源资源
- `script-src 'self' 'unsafe-inline'` —— 允许内联脚本
- `style-src 'self' 'unsafe-inline'` —— 允许内联样式
- `img-src 'self' data: blob: https:` —— 图片可以是 data URI、blob 或任意 HTTPS 源
- `connect-src 'self' ws: wss:` —— XHR / fetch / WebSocket 只能同源
- `frame-src 'self' https:` —— iframe 可嵌任意 HTTPS 源（用于 WOPI 等）
- `frame-ancestors 'self'` —— 本站只能被自己嵌入
- `object-src 'none'` —— 完全禁用插件对象

::: warning 第三方脚本 / 字体 / 字库会被 CSP 拦住
如果你的前端用了 Google Fonts、外部 CDN 脚本、Sentry、GA 之类的第三方资源，**会直接被浏览器拦下**。

当前没有提供 CSP 的可配置覆盖机制。想用外部依赖，建议：

1. 把依赖打包进你自己的 `dist/`（最推荐）
2. 或者**先提 issue 讨论**再考虑怎么放行特定源
:::

## PWA 与特殊路径

这几个路径会绕过 SPA fallback，按实际文件处理：

- `/sw.js` —— Service Worker
- `/manifest.webmanifest` —— PWA manifest
- `/workbox-*` —— Workbox 运行时
- `/pdfjs/*` —— PDF.js 资源（不会回退到 SPA，缺失直接 404）

其他路径在找不到具体文件时都会落到 SPA fallback，返回 `index.html`。

## 开发建议

- **本地开发** —— 直接跑 vite dev server，反代 `/api` 到 AsterDrive；不需要动 `./frontend-override/`
- **上线替换** —— 只替换 `./frontend-override/`，不要改二进制
- **想和官方前端并存** —— 当前版本不支持 A/B 或多前端并存，只能二选一
- **版本对齐** —— 二进制升级可能带新 API 或行为变更；你的自定义前端需要跟着测一轮

::: tip 希望 AsterDrive 提供更好的自定义前端支持？
现在这套机制是**最小可用**的——能跑，但粗糙。如果你在做自定义前端并且有具体的扩展需求（公开只读配置、自定义 CSP、多前端切换等），[开 issue 告诉我们](https://github.com/AptS-1547/AsterDrive/issues)，这种反馈会被优先看。
:::
