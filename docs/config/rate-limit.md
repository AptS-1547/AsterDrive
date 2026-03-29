# 访问限流

```toml
[rate_limit]
enabled = false

[rate_limit.auth]
seconds_per_request = 2
burst_size = 5

[rate_limit.public]
seconds_per_request = 1
burst_size = 30

[rate_limit.api]
seconds_per_request = 1
burst_size = 120

[rate_limit.write]
seconds_per_request = 2
burst_size = 10
```

限流默认关闭。打开后，会按访问来源 IP 对不同类型的访问分别限流。

## 什么时候建议开启

- 服务直接暴露在公网
- 想限制登录入口被暴力尝试
- 想限制公开分享页被频繁探测
- 想控制高成本写操作的瞬时压力

## 四组规则分别管什么

| 分组 | 作用 |
| --- | --- |
| `auth` | 登录、注册、分享密码验证这类敏感操作 |
| `public` | 公开分享页和匿名访问 |
| `api` | 已登录用户的大多数日常操作 |
| `write` | 批量操作、管理后台等高成本写操作 |

## 两个字段怎么理解

| 字段 | 说明 |
| --- | --- |
| `seconds_per_request` | 平均多久允许一次请求 |
| `burst_size` | 短时间内允许的突发请求数 |

例如:

```toml
[rate_limit.auth]
seconds_per_request = 2
burst_size = 5
```

表示同一个来源地址在认证类访问上可以先连续发出少量请求，超过后就会开始被限速。

## 触发后用户会看到什么

触发限流后，用户会收到“稍后再试”这一类反馈。  
服务端响应会是 `429 Too Many Requests`，并带 `Retry-After` 响应头。

## 反向代理场景一定要注意

当前版本的应用层限流按 AsterDrive 实际看到的连接来源 IP 工作。

如果你的部署是:

- Nginx / Caddy 反代到 AsterDrive
- Docker 网桥
- 其他会让所有请求都从同一个代理地址进入 AsterDrive 的方式

那么应用层限流很可能把所有用户都当成同一个来源。

这类部署里，更稳妥的做法通常是:

- 要么继续关闭 AsterDrive 应用层限流
- 要么把主要限流放到反向代理层处理

## 使用建议

- 第一次启用时，先保守一些，不要把 `burst_size` 设得太小
- 对外开放公开分享页时，优先关注 `auth` 和 `public`
- 不确定时，先在测试环境里观察一段时间再正式开启
