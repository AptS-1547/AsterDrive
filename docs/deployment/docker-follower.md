# Docker 部署从节点

::: tip 这一篇适合谁
想把另一台 AsterDrive 用 Docker 跑成 `follower`，并且希望**首次启动时直接靠环境变量自动 enroll**，不再手动 `docker exec ... aster_drive node enroll`。
:::

这一篇只讲 Docker 场景下的从节点部署。  
主控端怎么理解远程节点、怎么创建远程存储策略，仍然看 [远程节点](/guide/remote-nodes)。

和旧流程最大的区别只有一件事：

- **现在 follower 容器可以在启动时直接吃 bootstrap ENV，自动完成 enroll**

也就是说，第一次启动时不再需要：

- 手动进入容器执行 `aster_drive node enroll`
- enroll 完之后再额外重启一次容器

## 先确认这 4 件事

### 1. 主控节点已经正常运行

最少要满足：

- 主控后台可以正常打开
- `管理 -> 系统设置 -> 站点配置 -> 公开站点地址` 已经填成真实可访问的 HTTP(S) 来源；多来源配置时，follower 能访问到的主控来源应放第一行
- 你已经想好这个 follower 的名称、命名空间，以及主控将来访问它的 `base_url`

### 2. follower 必须有自己独立的 `data/`

主控和 follower **绝对不能共用**：

- `data/config.toml`
- 数据库
- 上传目录
- 临时目录

从节点不是“主控的另一个副本”，它是另一台独立的 AsterDrive。

### 3. 主控必须能访问 follower 的 `base_url`

如果主控和 follower 在不同机器上，`base_url` 通常会是：

- `https://follower.example.com`
- `http://10.0.0.23:3000`
- `http://host.example.com:3001`

如果它们都在 Docker 网络里，主控也能解析容器名，那么也可以直接填容器内地址。  
但别想当然地写 `http://localhost:3000`，那通常只对 follower 自己成立。

### 4. token 是一次性的

主控后台生成的 enrollment token：

- 默认 30 分钟过期
- 成功兑换一次后就作废

所以这类 ENV 只适合**首启 bootstrap**，不要长期把旧 token 留在 Compose 里。

## 1. 在主控后台创建远程节点并生成 token

入口：

```text
管理 -> 远程节点
```

先创建一条远程节点记录，至少填好：

- 名称
- 命名空间
- `base_url`

保存后，后台会生成一组 enroll 信息。Docker follower 启动时真正需要的是这两个值：

- `master_url`
- `token`

如果你不打算显式指定入站策略，就不用额外准备别的参数。  
空数据库首次启动时，follower 默认会有一条本地策略 `Local Default`，直接拿它接收入站对象就够了。

## 2. 准备 follower 的数据目录

如果你用 bind mount，把宿主机目录先建好并改属主：

```bash
mkdir -p ./data
sudo chown -R 10001:10001 ./data
```

如果你用 named volume，可以跳过这一步。

## 3. 写 `compose.yaml`

下面这份示例假设 follower 对外暴露在宿主机 `3001` 端口：

```yaml
services:
  asterdrive-follower:
    image: ghcr.io/apts-1547/asterdrive:latest
    container_name: asterdrive-follower
    ports:
      - "3001:3000"
    environment:
      ASTER__SERVER__HOST: 0.0.0.0
      ASTER__SERVER__START_MODE: follower
      ASTER__DATABASE__URL: sqlite:///data/asterdrive.db?mode=rwc
      ASTER_BOOTSTRAP_REMOTE_MASTER_URL: https://drive.example.com
      ASTER_BOOTSTRAP_REMOTE_ENROLLMENT_TOKEN: enr_replace_me
      # 可选：只有想覆盖 follower 默认落点时才传
      # ASTER_BOOTSTRAP_REMOTE_INGRESS_POLICY_ID: "1"
    volumes:
      - ./data:/data
      - /etc/localtime:/etc/localtime:ro
    restart: unless-stopped
```

这里最容易搞混的是两类环境变量：

- `ASTER__SERVER__START_MODE=follower`
  这是**长期运行配置**，建议保留
- `ASTER_BOOTSTRAP_REMOTE_*`
  这是**一次性 bootstrap 输入**，首次 enroll 成功后建议移除

大多数情况下，这里**不用**传 `ASTER_BOOTSTRAP_REMOTE_INGRESS_POLICY_ID`。

不传时，follower 会直接使用自己的**默认存储策略**作为入站落点。  
只有当你明确想覆盖这个默认落点时，才需要加：

```yaml
ASTER_BOOTSTRAP_REMOTE_INGRESS_POLICY_ID: "1"
```

这个值的真正含义是：

- 指定“主控写进这个 follower 的对象，最后落到 follower 的哪条本地策略”

它不是给主控端远程策略用的，也不是 enrollment token 的一部分。  
它只影响 follower 自己怎么接收入站对象。

所以最常见的两种情况是：

- follower 只有一条默认 `Local Default`
  那就留空，完全不用管
- follower 上有多条本地 / S3 策略，而且你希望远程对象固定落到其中一条非默认策略
  这时才传 `ASTER_BOOTSTRAP_REMOTE_INGRESS_POLICY_ID`

这个策略必须是 follower 本地可落地的策略，例如 `local` 或 `s3`，不能再套一层 `remote`。

## 4. 首次启动

```bash
docker compose up -d
docker logs -f asterdrive-follower
```

正常情况下，首次启动会依次完成这些事：

1. 在 `/data/config.toml` 不存在时自动生成配置
2. 以 `follower` 模式启动
3. 用 `ASTER_BOOTSTRAP_REMOTE_MASTER_URL` 和 `ASTER_BOOTSTRAP_REMOTE_ENROLLMENT_TOKEN` 去主控兑换 bootstrap 信息
4. 在本地数据库写入主控绑定
5. 继续完成 follower 运行时初始化

你应该能在日志里看到类似信息：

- `Configuration loaded from: /data/config.toml`
- `bootstrapped follower enrollment from environment`
- `startup complete — listening on 0.0.0.0:3000`

这条路径里，**不需要再手动执行 `node enroll`，也不需要首启后再额外重启一遍**。

## 5. 验证 follower 已经 ready

先看容器状态：

```bash
docker ps
```

再直接检查健康状态：

```bash
curl http://127.0.0.1:3001/health
curl http://127.0.0.1:3001/health/ready
```

期望结果：

- `/health` 返回 `200`
- `/health/ready` 在 enroll 成功并完成启动后也应返回 `200`

然后回主控后台：

```text
管理 -> 远程节点
```

点击“测试连接”。通过后，再去：

```text
管理 -> 存储策略
```

创建 `远程节点` 类型的存储策略。

## 6. 首次成功后，把一次性 bootstrap ENV 移掉

确认 follower 已经 ready、主控测试连接也通过后，把这几个 ENV 从 Compose 里删掉：

- `ASTER_BOOTSTRAP_REMOTE_MASTER_URL`
- `ASTER_BOOTSTRAP_REMOTE_ENROLLMENT_TOKEN`
- `ASTER_BOOTSTRAP_REMOTE_INGRESS_POLICY_ID`（如果你用了）

然后重新执行：

```bash
docker compose up -d
```

数据库里的主控绑定已经持久化了；后续重启 follower，不需要再重复 bootstrap。  
但 `ASTER__SERVER__START_MODE=follower` 这种长期运行配置，仍然应该保留。

## 常见坑

### 日志里提示 token 已完成、已过期或已被替换

这说明你拿的是旧 token。  
回主控后台重新生成一条新的 enrollment token，再更新 Compose。

### `/health` 是 200，但 `/health/ready` 还是 503

通常表示 follower 进程活着，但主控绑定还没有生效。优先检查：

- bootstrap ENV 有没有写对
- token 有没有过期
- follower 本地数据库里是否真的写入了绑定
- 日志里是否出现 bootstrap 失败 warning

### follower 能启动，但主控测试连接失败

优先检查这三件事：

- 主控后台里填的 `base_url` 是不是主控真正能访问到的地址
- 端口映射、反向代理或 NAT 有没有把流量正确转到 follower 的 `3000`
- follower 的 `server.host` 是否允许外部访问

### 已有旧的 `/data/config.toml`，里面还是 `primary`

最稳的做法有两个：

- 直接在 `/data/config.toml` 里把 `[server].start_mode` 改成 `follower`
- 或者像上面的 Compose 一样，长期保留 `ASTER__SERVER__START_MODE=follower`

别指望 bootstrap token 自己把一份已存在的 `primary` 配置文件改成 `follower`。  
它不会替你偷偷改现有配置。
