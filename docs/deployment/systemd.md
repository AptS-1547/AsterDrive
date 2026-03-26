# systemd 部署

systemd 适合长期稳定运行的 Linux 主机。  
先把工作目录定下来，再决定配置文件、数据库和本地上传目录放在哪。

## 1. 准备运行目录

```bash
sudo useradd -r -s /usr/sbin/nologin asterdrive
sudo mkdir -p /var/lib/asterdrive
sudo chown -R asterdrive:asterdrive /var/lib/asterdrive
```

## 2. 放置可执行文件

把 `aster_drive` 可执行文件放到一个固定路径，例如：

```bash
sudo install -m 0755 ./aster_drive /usr/local/bin/aster_drive
```

## 3. 准备配置文件

把 `config.toml` 放进工作目录：

```bash
sudo cp config.toml /var/lib/asterdrive/config.toml
sudo chown asterdrive:asterdrive /var/lib/asterdrive/config.toml
```

如果继续使用默认 SQLite 和默认本地存储，工作目录下会自动出现：

- `asterdrive.db`
- `data/uploads`

如果你打算长期运行，建议数据库路径和本地存储路径尽量使用绝对路径，避免以后修改 `WorkingDirectory` 时找错数据。

## 4. 写入 Service 文件

创建 `/etc/systemd/system/asterdrive.service`：

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
Environment=ASTER__AUTH__JWT_SECRET=your-fixed-secret
```

## 9. HTTPS 与域名

systemd 只负责拉起服务。要提供 HTTPS、域名和 WebDAV 客户端访问，还需要在前面加一层反向代理，见 [反向代理部署](/deployment/proxy)。
