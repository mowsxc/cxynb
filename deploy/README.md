# cxynb 部署说明（真实环境，非 MOCK）

将美团订单系统（Rust `meituan-rs`）与收银台（Python `cashier`）部署到 `192.168.1.15`，
通过 `cxynb.mow.kdns.fr` 对外提供访问：

| 路径 | 服务 | 后端 |
|------|------|------|
| `/mtdd/` | 美团订单管理 | Rust `meituan-rs` :8899 |
| `/syt/`  | 收银台 | Python `cashier` :5000 |

两个页面右上角/顶部均有互相跳转的导航链接（美团订单页 → 收银台；收银台 → 美团订单）。

## 架构

```
浏览器 ──https──> Cloudflare(cxynb.mow.kdns.fr) ──http──> 本机 nginx :80
                                                        ├─ /mtdd/*  -> 127.0.0.1:8899 (meituan-rs)
                                                        └─ /syt/*   -> 127.0.0.1:5000 (cashier)
```

- **前端自适配子路径**：两个 HTML 页面在加载时根据 `location.pathname`（以 `/mtdd` 或 `/syt` 开头）
  自动计算 API 基址 `API`，所有 `/api/...` 请求都会带上对应前缀，因此无需改动业务 JS 逻辑即可挂在子路径下。
- **nginx 负责路径重写**：`location /mtdd/ { proxy_pass http://127.0.0.1:8899/; }` 会把
  `/mtdd/api/query` 透明改写为 `/api/query` 转给 Rust 后端，收银台同理。
- **Cloudflare 已终结 TLS**：本机 nginx 只需监听 80（Flexible 模式）。如需 Full/Strict，
  在 `cxynb.conf` 增加 `listen 443 ssl` 与证书即可。

## 一键部署

在目标服务器（`.15`）上，把本 bundle 传过去后以 root 执行：

```bash
# 方式 A：直接在本机解压 bundle 后运行
sudo bash deploy/install.sh

# 方式 B：从本项目仓库拉取后运行
git clone <你的仓库> /tmp/cxynb && sudo bash /tmp/cxynb/deploy/install.sh
```

脚本会自动：装依赖 → 拷贝程序 → 建 cashier 虚拟环境 → 注册 systemd 服务 → 配置 nginx → 放行 80 端口。

## 手动步骤（等价于一键脚本）

```bash
# 1) 依赖
apt-get update && apt-get install -y nginx python3 python3-venv python3-pip ufw

# 2) 程序目录
mkdir -p /opt/cxynb && cp meituan-rs meituan_query.html meituan_settings.html \
  meituan_sync.py logo.png settings.json /opt/cxynb/
cp -r cashier /opt/cxynb/cashier
python3 -m venv /opt/cxynb/cashier/venv
/opt/cxynb/cashier/venv/bin/pip install flask

# 3) 服务（systemd unit 见 systemd/）
systemctl enable --now cxynb-meituan cxynb-cashier

# 4) nginx（配置见 nginx/cxynb.conf）
ln -sf /etc/nginx/sites-available/cxynb.conf /etc/nginx/sites-enabled/cxynb.conf
nginx -t && systemctl restart nginx && ufw allow 80
```

## 首次登录（美团 Cookie）

`meituan-rs` 需要美团商家后台 Cookie 才能拉取订单。服务器通常为无界面环境，无法自动弹浏览器登录：

1. 在一台已登录美团商家后台的电脑上，把 `meituan_cookies.json` 复制到 `/opt/cxynb/`；
2. `sudo systemctl restart cxynb-meituan.service`；
3. 或临时在有桌面环境的机器上运行 `meituan-rs.exe/二进制`，登录后把生成的 `meituan_cookies.json` 拷回服务器。

## 运维

```bash
systemctl status cxynb-meituan cxynb-cashier nginx
journalctl -u cxynb-meituan -u cxynb-cashier -f
```
