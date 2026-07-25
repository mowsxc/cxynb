#!/usr/bin/env bash
# =====================================================================
#  cxynb 一键部署脚本（在目标服务器 192.168.1.15 上以 root 运行）
#  用法:
#    sudo bash deploy/install.sh
#  说明:
#    - 把本目录（bundle）里的程序/页面拷贝到 /opt/cxynb
#    - 安装并启动两个服务: meituan-rs(8899) + cashier(5000)
#    - 配置 nginx 反向代理: /mtdd -> 美团订单, /syt -> 收银台
#    - 前置 Cloudflare 已把 cxynb.mow.kdns.fr 指向本机 80 端口
# =====================================================================
set -euo pipefail

APP_USER="${APP_USER:-mo}"
APP_DIR="${APP_DIR:-/opt/cxynb}"
BUNDLE="$(cd "$(dirname "$0")/.." && pwd)"
NGINX_AVAILABLE="/etc/nginx/sites-available"
NGINX_ENABLED="/etc/nginx/sites-enabled"

echo "==> Bundle: $BUNDLE"
echo "==> 目标目录: $APP_DIR  运行用户: $APP_USER"

if [ "$(id -u)" -ne 0 ]; then
  echo "请用 root 运行: sudo bash $0" >&2
  exit 1
fi

# ── 1. 系统依赖 ────────────────────────────────────────────────────────
echo "==> [1/7] 安装系统依赖 (nginx, python3, venv, 构建工具) ..."
export DEBIAN_FRONTEND=noninteractive
apt-get update -y
apt-get install -y nginx python3 python3-venv python3-pip curl git \
  ca-certificates build-essential pkg-config libssl-dev libssl3 ufw || true

# ── 2. 拷贝程序文件 ────────────────────────────────────────────────────
echo "==> [2/7] 拷贝程序到 $APP_DIR ..."
mkdir -p "$APP_DIR" "$APP_DIR/cashier"
# 代码/页面：总是覆盖
for f in meituan_query.html meituan_settings.html meituan_sync.py logo.png settings.json README.md DESIGN_SYSTEM.md; do
  [ -e "$BUNDLE/$f" ] && cp -f "$BUNDLE/$f" "$APP_DIR/"
done
# 预编译二进制：兼容顶层 与 target/release 两种位置
if [ -e "$BUNDLE/target/release/meituan-rs" ]; then
  cp -f "$BUNDLE/target/release/meituan-rs" "$APP_DIR/meituan-rs"
elif [ -e "$BUNDLE/meituan-rs" ]; then
  cp -f "$BUNDLE/meituan-rs" "$APP_DIR/meituan-rs"
fi
# 数据文件：仅当目标不存在时拷贝（避免覆盖线上数据）
# 注意：meituan_orders.db 是 SQLCipher 加密库，与原始主密码绑定，不能跨密码复用，
#       因此这里不拷贝它——首次启动会用生成的主密码新建一个空库并从美团拉取真实数据。
#       若需保留历史：请把原 password.meta.json + meituan_orders.db 放到 $APP_DIR，
#       并在 /opt/cxynb/.master_pwd 里填入你原来的主密码后重启服务。
if [ -e "$BUNDLE/meituan_cookies.json" ] && [ ! -e "$APP_DIR/meituan_cookies.json" ]; then
  cp -f "$BUNDLE/meituan_cookies.json" "$APP_DIR/"
fi
# cashier 目录（代码 + 模板）
rm -rf "$APP_DIR/cashier"
cp -r "$BUNDLE/cashier" "$APP_DIR/cashier"

# 校验二进制可在本机运行（兼容 glibc / 依赖库）；不可用则尝试源码构建
if [ ! -x "$APP_DIR/meituan-rs" ] || ! ldd "$APP_DIR/meituan-rs" >/tmp/ldd_meituan 2>&1 || grep -q "not found" /tmp/ldd_meituan; then
  echo "预编译 meituan-rs 不可用（缺失或未兼容当前 glibc），尝试源码构建 ..."
  if [ -f "$BUNDLE/Cargo.toml" ] && command -v cargo >/dev/null 2>&1; then
    ( cd "$BUNDLE" && cargo build --release && cp -f "$BUNDLE/target/release/meituan-rs" "$APP_DIR/" )
  else
    echo "  安装 Rust 工具链并从 https://github.com/mowsxc/cxynb 构建 ..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    # shellcheck disable=SC1091
    [ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
    rm -rf /tmp/cxynb-src && git clone --depth 1 https://github.com/mowsxc/cxynb /tmp/cxynb-src
    ( cd /tmp/cxynb-src && cargo build --release && cp -f target/release/meituan-rs "$APP_DIR/" )
  fi
fi
chmod +x "$APP_DIR/meituan-rs"

# ── 3. cashier 虚拟环境 ────────────────────────────────────────────────
echo "==> [3/7] 配置收银台 Python 虚拟环境 ..."
python3 -m venv "$APP_DIR/cashier/venv"
"$APP_DIR/cashier/venv/bin/pip" install --quiet --upgrade pip
"$APP_DIR/cashier/venv/bin/pip" install --quiet flask

# ── 4. 赋权 + 主密码文件 ───────────────────────────────────────────────
echo "==> [4/7] 设置归属与权限 ..."
id -u "$APP_USER" >/dev/null 2>&1 || useradd -m -s /bin/bash "$APP_USER"
chown -R "$APP_USER:$APP_USER" "$APP_DIR"

# 主密码（用于服务端 Cookie/数据加密，首次启动自动写入 password.meta.json）
# 仅生成一次，重装不会覆盖，保证与已存在的 password.meta.json 一致。
if [ ! -f "$APP_DIR/.master_pwd" ]; then
  MASTER_PWD="$(head -c 18 /dev/urandom | base64 | tr -dc 'A-Za-z0-9' | head -c 24)"
  echo "MEITUAN_PWD=$MASTER_PWD" > "$APP_DIR/.master_pwd"
  chmod 600 "$APP_DIR/.master_pwd"
  chown root:root "$APP_DIR/.master_pwd"
  echo "    已生成主密码（请记录，忘记可用恢复密钥重置）: $MASTER_PWD"
fi

# ── 5. systemd 服务 ────────────────────────────────────────────────────
echo "==> [5/7] 注册 systemd 服务 ..."
sed "s/^User=mo/User=$APP_USER/" "$BUNDLE/deploy/systemd/cxynb-meituan.service" > /etc/systemd/system/cxynb-meituan.service
sed "s/^User=mo/User=$APP_USER/" "$BUNDLE/deploy/systemd/cxynb-cashier.service" > /etc/systemd/system/cxynb-cashier.service
systemctl daemon-reload
systemctl enable --now cxynb-meituan.service
systemctl enable --now cxynb-cashier.service
sleep 2
systemctl restart cxynb-meituan.service cxynb-cashier.service

# ── 6. nginx 反向代理 ──────────────────────────────────────────────────
echo "==> [6/7] 配置 nginx ..."
cp -f "$BUNDLE/deploy/nginx/cxynb.conf" "$NGINX_AVAILABLE/cxynb.conf"
ln -sf "$NGINX_AVAILABLE/cxynb.conf" "$NGINX_ENABLED/cxynb.conf"
rm -f "$NGINX_ENABLED/default" 2>/dev/null || true
nginx -t
systemctl enable --now nginx
systemctl restart nginx
ufw allow 80/tcp 2>/dev/null || true

# ── 7. 完成 ────────────────────────────────────────────────────────────
echo "==> [7/7] 部署完成"
echo
echo "  美团订单:  http://cxynb.mow.kdns.fr/mtdd/"
echo "  收银台:    http://cxynb.mow.kdns.fr/syt/"
echo
echo "  服务状态:"
systemctl is-active cxynb-meituan.service cxynb-cashier.service nginx
echo
echo "  备注:"
echo "   - 美团 Cookie 首次需登录一次: 把已登录的 meituan_cookies.json 放到 $APP_DIR/ 并重启服务"
echo "     sudo systemctl restart cxynb-meituan.service"
echo "   - 查看日志: sudo journalctl -u cxynb-meituan -u cxynb-cashier -f"
