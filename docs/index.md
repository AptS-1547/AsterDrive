---
layout: home

hero:
  name: AsterDrive
  text: 自托管文件、团队空间与 WebDAV
  tagline: 给部署者、管理员和普通用户使用的在线手册，按当前版本实际页面、入口和默认行为编写
  actions:
    - theme: brand
      text: 快速开始
      link: /guide/getting-started
    - theme: alt
      text: 部署手册
      link: /guide/installation
    - theme: alt
      text: 用户手册
      link: /guide/user-guide

features:
  - title: 一个服务交付完整站点
    details: 浏览器文件管理、公开分享页、管理后台和 WebDAV 都由同一个 AsterDrive 服务提供，部署时不用再拆第二套前端站点
  - title: 从试跑到正式上线一条线讲清楚
    details: 快速开始负责把服务跑起来，部署手册负责选 Docker、systemd 或二进制，配置说明负责改站点地址、数据库、存储和后台规则
  - title: 按真实页面和入口来写
    details: 文档会直接对应登录页、文件页面、分享页、WebDAV 页面、团队页面和管理后台，而不是站在代码结构角度解释
  - title: 先告诉你去哪一页
    details: 不同角色会直接给出起点页。第一次部署看快速开始，普通用户看用户手册，管理员看管理后台和配置说明
  - title: 个人空间和团队空间分开说明
    details: 上传、分享、回收站、WebDAV、团队协作这些操作会明确区分“我的云盘”和团队空间，避免第一次用就走错地方
  - title: 本地盘和 S3 都能落地
    details: 默认本地存储开箱即用；如果你要接 MinIO、AWS S3 或其他 S3 兼容对象存储，也有对应配置和排查说明
  - title: 管理后台按日常维护来组织
    details: 用户、团队、存储策略、策略组、分享、锁、系统设置、审计日志和版本信息都按管理员日常动作编排
---

## 从哪里开始

- 第一次把服务跑起来：看 [快速开始](/guide/getting-started)
- 还没决定用 Docker、systemd 还是直接运行：看 [部署手册](/guide/installation)
- 想知道登录后怎么上传、分享、恢复、编辑和管理团队空间：看 [用户手册](/guide/user-guide)
- 想按场景做事，例如新部署后的首轮检查、给不同用户安排存储路线、处理误删：看 [常用流程](/guide/core-workflows)
- 想了解管理后台每个入口负责什么：看 [管理后台](/guide/admin-console)
- 想改端口、数据库、登录密钥、WebDAV、系统设置、存储策略或日志：看 [配置说明](/config/)
- 准备挂 HTTPS、反向代理、升级或检查首次启动结果：看 [部署与升级](/deployment/)
