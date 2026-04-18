---
layout: home

hero:
  name: AsterDrive
  text: 自托管云存储
  tagline: 你的文件，真的在你自己手里。一条命令跑起来，用 Rust 写的，MIT 开源。
  actions:
    - theme: brand
      text: 快速开始
      link: /guide/getting-started
    - theme: alt
      text: 关于 AsterDrive
      link: /guide/about
    - theme: alt
      text: GitHub
      link: https://github.com/AptS-1547/AsterDrive

features:
  - title: 一条命令跑起来
    details: 单个 Rust 二进制，没有 PHP、没有外部 runtime、没有一堆要先装的依赖。一条 docker run 就能在 10 分钟内看到自己的第一个文件躺在自己服务器上。
  - title: 默认 SQLite，真上线再切 PG
    details: 默认零运维启动，让你 5 分钟看一下"这个项目是什么"。等真要上线、数据多起来，自带跨数据库迁移工具，平滑切到 PostgreSQL 或 MySQL，不会被困死在 SQLite 里。
  - title: 改得动（hackable）
    details: 代码用 Rust 写，分层清晰、错误码千位分域、repo 与 service 拆开。你能 fork 之后看懂、改、PR 回来——我们最喜欢的 issue 是带 PR 来的那种。
  - title: 不会有 Pro 版
    details: 不会有付费版、Pro 版或功能墙。所有功能在 MIT 协议下开源，每个人能用到的东西完全一样。现在没有，将来也不会有。
  - title: 先是一个安心放文件的地方
    details: 文件管理、上传下载、回收站是骨架。回收站不是附加功能——没有它，你不敢真的把重要文件放进去。再之上才是分享、团队、WebDAV、Office 在线编辑。
  - title: 本地盘和 S3 都能落地
    details: 默认本地存储开箱即用；如果你要接 MinIO、AWS S3 或其他 S3 兼容对象存储，配上去就能跑。策略组让你按用户或团队分流到不同的存储路线。
  - title: Office 文件也能在线编辑
    details: 浏览器内直接编辑文本类文件；Office 类文件可以接 Collabora 或 OnlyOffice 之类的 WOPI 服务，让它们交给兼容的在线编辑器打开。
  - title: 给运维留了路
    details: 启动 healthcheck、跨库迁移工具、doctor 一致性检查、系统设置可热改、完整的 systemd / Docker / 反向代理部署文档——不假装运维不存在。
---

## 从哪里开始

按你的角色和目的，直接挑一条往下走：

- **第一次把服务跑起来** → [快速开始](/guide/getting-started)
- **想了解这个项目本身** → [关于 AsterDrive](/guide/about)
- **决定用 Docker、systemd 还是二进制** → [部署手册](/guide/installation)
- **登录后想知道怎么上传、分享、恢复、编辑和管理团队空间** → [用户手册](/guide/user-guide)
- **按场景做事**（新部署首轮检查、安排存储路线、处理误删等） → [常用流程](/guide/core-workflows)
- **想了解管理后台每个入口** → [管理后台](/guide/admin-console)
- **改端口、数据库、登录密钥、WebDAV、系统设置或日志** → [配置说明](/config/)
- **挂 HTTPS、反向代理、升级或备份恢复** → [部署与升级](/deployment/)
- **在命令行里做部署检查、离线改系统设置或跨数据库迁移** → [运维 CLI](/deployment/ops-cli)
- **碰到问题** → [错误码处理](/guide/errors) / [故障排查](/deployment/troubleshooting)

---

::: tip 一句话
**别给自己的数据增加心智负担**——这是我们做 AsterDrive 的初衷。
:::
