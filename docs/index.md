---
layout: home

hero:
  name: AsterDrive
  text: 自托管文件与分享服务
  tagline: 从部署、登录到上传、分享、WebDAV 和管理后台，这套文档都按实际页面和当前版本来写
  actions:
    - theme: brand
      text: 快速开始
      link: /guide/getting-started
    - theme: alt
      text: 安装部署
      link: /guide/installation
    - theme: alt
      text: 用户手册
      link: /guide/user-guide

features:
  - title: 一套服务就能用
    details: 浏览器页面、公开分享页和 WebDAV 都由同一个 AsterDrive 服务提供，不需要额外再拆一个管理站点
  - title: 部署手册按用户场景写
    details: 先告诉你该怎么部署、数据放哪里、什么时候该开 HTTPS，再给你需要的命令和示例
  - title: 文件日常操作齐全
    details: 支持文件夹、上传、下载、搜索、拖拽整理、多选批量操作、预览、文本编辑和回收站
  - title: 分享和 WebDAV 可直接上手
    details: 文件和文件夹都能发公开链接；也可以为每台设备创建独立的 WebDAV 账号
  - title: 管理后台覆盖常见维护
    details: 管理员可以管理用户、配额、存储策略、系统设置、分享链接、锁和审计日志
  - title: 本地磁盘或对象存储都支持
    details: 默认本地存储开箱即用，也可以改成 S3 或 MinIO，并按策略控制上传方式
---

## 常用入口

- 第一次把服务跑起来：看 [快速开始](/guide/getting-started)
- 还没决定用 Docker、systemd 还是直接跑二进制：看 [安装部署](/guide/installation)
- 想知道登录后怎么上传、分享、恢复文件：看 [用户手册](/guide/user-guide)
- 想知道管理员日常要改什么：看 [管理后台](/guide/admin-console)
- 想改端口、数据库、登录、WebDAV 或存储方式：看 [配置概览](/config/)
