# 美团订单管理系统

**美团商家后台订单自动抓取、对账、计费系统。纯 Rust 后端 + 原生 JS 前端，单 EXE 运行。**

## 核心特性

- **智能双轨同步** — 快速同步（15分钟窗口，~1秒）+ 深度同步（50小时窗口，~2秒）
- **纯 Rust 同步** — 无 Python 依赖，90天窗口分片 + 并发分页拉取
- **物理消失订单检测** — 自动标记美团后台撤销核销的订单
- **计费价自动匹配** — 关键词匹配套餐 → 计费系统价格
- **财务价计算** — 消费金额 - 商家优惠 - 服务费7%
- **多维度统计** — 月度/季度/半年/年度趋势图表
- **列显示自定义** — PC/移动端独立配置，持久化保存
- **系统托盘** — 后台运行，单击打开网页，右键菜单操作
- **单文件部署** — 一个 `.exe` 即可运行，无需依赖

## 技术栈

- **后端**: Rust 1.80+, actix-web 4, rusqlite (r2d2 连接池), tray-icon, ureq 3
- **前端**: 原生 HTML/CSS/JS（无框架），单文件 `meituan_query.html`
- **数据库**: SQLite
- **同步**: 纯 Rust 实现（`src/sync.rs`），无 Python 依赖
- **构建**: `cargo build` → 单 EXE

## 项目结构

```
├── src/main.rs              ← Rust 后端源码（HTTP服务 + 认证加密）
├── src/sync.rs              ← 订单同步模块（纯Rust，并发分页）
├── src/crypto.rs            ← 加密模块（AES-256-GCM + Argon2id）
├── meituan_query.html       ← Web 前端（单文件）
├── Cargo.toml               ← 项目配置
├── build.rs                 ← 编译脚本（版本号）
├── meituan-rs.exe           ← 编译产物（单文件部署）
├── meituan_orders.db        ← SQLite 数据库（自动创建）
├── meituan_cookies.json     ← 美团 Cookie（自动生成）
├── settings.json            ← 业务规则配置（自动生成）
├── meituan-rs.log           ← 运行日志
├── CHANGELOG.md             ← 版本变更记录
├── ARCHITECTURE.md          ← 架构设计（含审计流程 + 设计系统）
└── AGENTS.md                ← 开发规范（含 Git 工作流）
```

## 快速开始

```bash
# 构建
cargo build

# 运行
./target/debug/meituan-rs

# 验证
cargo check 2>&1 | grep -i error
curl http://localhost:8899/api/health
```

**首次启动流程**：
1. 程序启动 HTTP 服务（端口 8899）
2. 自动打开浏览器 `http://localhost:8899`
3. 前端弹出登录覆盖层 → 用户导入/粘贴 Cookie
4. 校验 Cookie 有效性后自动触发首次全量同步
5. 数据实时入库，前端可查看进度

## 架构原则（核心约束）

### 数据分离（v0.7.0 重构）
```
后端 settings.json  ←→  API  ←→  前端 localStorage
  业务配置                         UI 偏好
  (班次/计费/刷新)                 (列显示/复制偏好)
```

**铁律**：
- `Settings` 结构体中 `columns`/`key_info`/`copy_header` 必须保持 `#[serde(skip)]`
- API 绝不传输 UI 状态字段
- 前端列设置只走 `localStorage` + 版本号机制
- 修改 `COLUMNS` 定义时必须同步递增 `COLUMN_SETTINGS_VERSION`

## 双轨智能同步

**同步机制**：纯 Rust 实现（`src/sync.rs`），使用 `ureq` + `std::thread::scope` 并发分页拉取。

| 同步类型 | 频率 | 时间窗口 | 覆盖时效 |
|---------|------|---------|---------|
| 快速同步 | 60秒（可调 5~3600秒） | 15 分钟前推 | 10 分钟内撤销验券 |
| 深度同步 | 30 分钟（固定） | 50 小时前推 | 48 小时自动退款 |

**首次全量同步**：DB 为空时从 `2026-01-01 00:00:00` 全量拉取，按 90 天窗口分片。

**数据封存**：>50h 历史数据永不重复拉取。

**分片策略**：
- 90 天窗口（API 最大支持，超过返回 0）
- 窗口内并发分页拉取（先拉第 1 页获取总数，剩余页并发）
- 8 worker 线程并发处理多个窗口
- 每个窗口拉完立即写库（前端可实时查看进度）

**性能**：快速 ~1 秒，深度 ~2 秒，全量首次 ~28 秒（9,649 条）

**退款/撤销检测逻辑**（重要！）：
- API 返回的退款/撤销订单**不会被删除**，而是通过 `description` 字段标记
- 同步时逐条对比数据库：
  - 券号不存在 → 新订单，按 `description` 判断入库
  - 券号已存在 → 对比 `description`，含"退款/撤销/退费/撤单/逆向"等关键词则更新状态
- **绝不靠"API 未返回"推断退款状态**（这是严重错误逻辑）

## 部署指南

### 核心文件（必需）

| 文件 | 说明 |
|------|------|
| `meituan-rs.exe` | 主程序（Release 版本） |
| `meituan_query.html` | 前端单页应用 |

### 数据文件（推荐打包）

| 文件 | 说明 |
|------|------|
| `meituan_orders.db` | SQLite 数据库（订单历史） |
| `meituan_cookies.enc` | 美团登录凭证（加密） |
| `settings.json` | 业务规则配置（班次、计费规则） |
| `meituan.meta.json` | 密码元数据 |

### 快速部署

将文件复制到目标电脑同一目录 → 双击 `meituan-rs.exe`

### 故障排查

| 问题 | 解决 |
|------|------|
| Cookie 失效 | 删除 `meituan_cookies.enc`，重启程序重新登录 |
| 数据库损坏 | 删除 `meituan_orders.db`，重启自动重建 |
| 端口占用 | 修改 `PORT` 环境变量 |

## API 端点

| 端点 | 方法 | 说明 |
|------|------|------|
| `/` | GET | 前端页面 |
| `/api/health` | GET | 健康检查（返回数据量、Cookie 状态） |
| `/api/refresh` | GET | 触发同步（`?deep=1` 深度同步） |
| `/api/settings` | GET/PUT | 读取/保存业务配置 |
| `/api/auth/*` | POST | 认证（setup/unlock/recover） |
| `/api/cookie/validate` | GET | 校验 Cookie 有效性 |

## 开发命令

```bash
# 构建
cargo build && cargo build --release

# 检查
cargo check
cargo clippy -- -D warnings
cargo fmt --check

# 测试
cargo test

# 前端 JS 语法检查
node --check <(python -c "import re;print(re.findall(r'<script>(.*?)</script>',open('meituan_query.html').read(),re.DOTALL)[0])")
```

## 已知编辑风险点

1. `meituan_query.html` 的 `<script>` 块任何语法错误 → 整个页面瘫痪
2. `settings.json` 的 `fee_json` 字段 → 中文不能走二次 JSON 序列化
3. `Settings` 结构体字段必须保持 `#[serde(skip)]`
4. `COLUMNS` 定义变更时必须递增 `COLUMN_SETTINGS_VERSION`
5. **退款/撤销检测必须基于 API `description` 字段，绝不靠"API 未返回"推断**

## 继续开发注意事项

1. 任何涉及 `columns`/`key_info`/`copy_header` 的改动，必须确认 `#[serde(skip)]` 仍在
2. 前端列显示逻辑改动后，必须验证 PC 默认 10 列、财务价隐藏
3. 新增 API 字段时，确认是否属于 UI 偏好（不应存后端）
4. 修改 `COLUMNS` 后运行自检：浏览器 Console 应输出 `[自检] 列显示: 10/11 列可见`
# cxynb
