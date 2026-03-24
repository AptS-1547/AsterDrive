# systemd 部署

systemd 适合长期稳定运行。这里最重要的是先确定 `WorkingDirectory`，因为默认配置、SQLite 和本地上传目录都会受它影响。

## 1. 安装二进制

```bash
sudo install -m 0755 target/release/aster_drive /usr/local/bin/aster_drive
```

## 2. 创建运行用户与目录

```bash
sudo useradd -r -s /usr/sbin/nologin asterdrive
sudo mkdir -p /var/lib/asterdrive
sudo chown -R asterdrive:asterdrive /var/lib/asterdrive
```

## 3. 准备配置文件

把配置文件放进工作目录：

```bash
sudo cp config.toml /var/lib/asterdrive/config.toml
sudo chown asterdrive:asterdrive /var/lib/asterdrive/config.toml
```

如果你不想手写配置，也可以先让服务在这个目录里启动一次，让它自动生成默认配置，再回头修改。

如果继续使用默认 SQLite 和默认本地存储，工作目录下会自动出现：

- `asterdrive.db`
- `data/uploads`

## 4. Service 文件

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

如果你现在还只是内网 HTTP 测试，记得在 `config.toml` 里把 `auth.cookie_secure` 设成 `false`。

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

上线后建议先确认：

- `/health` 返回 200
- `/health/ready` 返回 200
- 首次启动日志里已完成数据库迁移和默认策略初始化
- 浏览器可以正常登录
- 如果启用了 WebDAV，实际挂载路径与 `[webdav].prefix` 一致

## 7. 常见变体

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

## 8. HTTPS 与域名

systemd 只负责拉起服务。若你需要 HTTPS、域名和 WebDAV 客户端访问，请在前面再加一层反向代理，见 [反向代理部署](/deployment/proxy)。
