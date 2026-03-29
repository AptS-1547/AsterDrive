# systemd 部署

systemd 适合长期稳定运行的 Linux 服务器。  
这类部署最重要的是先定好工作目录，再决定配置文件、数据库、上传目录和临时目录放在哪里。

## 1. 准备运行目录

```bash
sudo useradd -r -s /usr/sbin/nologin asterdrive
sudo mkdir -p /var/lib/asterdrive
sudo chown -R asterdrive:asterdrive /var/lib/asterdrive
```

## 2. 放置可执行文件

把 `aster_drive` 可执行文件放到固定路径，例如:

```bash
sudo install -m 0755 ./aster_drive /usr/local/bin/aster_drive
```

## 3. 准备配置文件

把 `config.toml` 放进工作目录:

```bash
sudo cp config.toml /var/lib/asterdrive/config.toml
sudo chown asterdrive:asterdrive /var/lib/asterdrive/config.toml
```

如果你继续使用默认相对路径，工作目录下通常会出现:

- `asterdrive.db`
- `data/uploads`
- `data/.tmp`
- `data/.uploads`

长期部署时，建议数据库路径、本地存储路径和临时目录都尽量使用绝对路径。

## 4. 写入 Service 文件

创建 `/etc/systemd/system/asterdrive.service`:

```ini
[Unit]
Description=AsterDrive
After=network.target

[Service]
Type=simple
User=asterdrive
Group=asterdrive
WorkingDirectory=/var/lib/asterdrive
ExecStart=/usr/local/bin/aster_drive
Restart=on-failure
RestartSec=5
Environment=RUST_LOG=info

[Install]
WantedBy=multi-user.target
```

如果你现在还是内网 HTTP 测试，记得在 `config.toml` 里把 `auth.cookie_secure` 设成 `false`。正式切到 HTTPS 后再改回 `true`。

## 5. 启动服务

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now asterdrive
sudo systemctl status asterdrive
```

## 6. 查看日志

```bash
journalctl -u asterdrive -f
```

## 7. 首次验收

- `/health` 返回 200
- `/health/ready` 返回 200
- 首次启动日志里已完成数据库更新和默认策略初始化
- 浏览器可以正常登录
- 如果启用了 WebDAV，实际挂载路径与 `[webdav].prefix` 一致
- 如果你把数据库、上传目录或临时目录放到其他磁盘，确认路径和权限没有写错

## 8. 常见环境变量写法

### 把数据库放到其他位置

```ini
Environment=ASTER__DATABASE__URL=sqlite:///srv/asterdrive/asterdrive.db?mode=rwc
```

### 监听所有网卡

```ini
Environment=ASTER__SERVER__HOST=0.0.0.0
```

### 固定 JWT 密钥

```ini
Environment=ASTER__AUTH__JWT_SECRET=replace-with-your-own-secret
```

## 9. HTTPS 与域名

systemd 只负责把服务拉起来。  
如果你要提供 HTTPS、域名和 WebDAV 客户端访问，还需要在前面加一层反向代理，见 [反向代理部署](/deployment/proxy)。
