# 美团订单管理系统

> 美团商家后台订单数据自动抓取、存储、查询、对账的一体化本地服务。

![version](https://img.shields.io/badge/version-v0.6.1-blue)
![platform](https://img.shields.io/badge/platform-Windows-0078d4)
![language](https://img.shields.io/badge/language-Rust-orange)

## ✨ 特性

- **智能双轨同步** — 1~10秒微轮询拉取最新订单 + 30分钟深度对账
- **物理消失订单检测** — 自动标记美团后台撤销核销的订单
- **计费价自动匹配** — 关键词匹配套餐 → 计费系统价格
- **财务价计算** — 消费金额 - 商家优惠 - 服务费7%
- **多维度统计** — 月度/季度/半年/年度趋势图表
- **列显示自定义** — PC/移动端独立配置，持久化保存
- **系统托盘** — 后台运行，单击打开网页，右键菜单操作
- **单文件部署** — 一个 `.exe` 即可运行，无需依赖

## 🚀 快速开始

```bash
# 首次运行（自动打开浏览器登录）
meituan-rs.exe

# 登录成功后浏览器自动打开
# 或手动访问 http://localhost:8899
```

## 📁 项目结构

```
meituan_orders/
├── src/main.rs              ← Rust 后端源码
├── meituan_query.html       ← Web 前端（单文件）
├── Cargo.toml               ← 项目配置
├── build.rs                 ← 编译脚本（版本号）
├── meituan-rs.exe           ← 编译产物（单文件部署）
├── meituan_orders.db        ← SQLite 数据库（自动创建）
├── meituan_cookies.json     ← 美团 Cookie（自动生成）
├── settings.json            ← 业务规则配置（自动生成）
├── meituan-rs.log           ← 运行日志
├── CHANGELOG.md             ← 版本变更记录
├── ARCHITECTURE.md          ← 架构设计
├── DESIGN_SYSTEM.md         ← UI 设计规范
├── AUDIT_PROCESS.md         ← 审计测试流程
├── GIT_WORKFLOW.md          ← Git 工作流
├── DEPLOY.md                ← 部署指南
└── README.md                ← 本文件
```

## 💻 系统要求

- Windows 7+ （需 [VC++ Redistributable](https://aka.ms/vs/17/release/vc_redist.x64.exe)）
- Microsoft Edge 或 Chrome（仅首次登录时需要）
- 无需 Python、Node.js 或其他运行时

## 🔄 使用流程

```
首次启动 → 自动打开浏览器 → 手动登录美团商家后台
    ↓ 登录成功
自动提取 Cookie → 保存到本地 → 启动 Web 服务
    ↓
浏览器可关闭 → 智能双轨同步（微轮询 + 深度对账）
    ↓
访问 http://localhost:8899 查看对账报表
```

## 🎯 系统托盘

启动后在系统托盘显示图标（美团黄底 + 白色 M Logo）：

| 操作 | 功能 |
|------|------|
| **左键单击** | 打开浏览器管理页面 |
| **右键菜单** | 打开网页 / 刷新数据 / 设置 / 关于 / 退出 |

服务后台运行，关闭浏览器不影响数据同步。

## ⚙️ 配置

```bash
# 修改端口
set PORT=8080 && meituan-rs.exe

# 禁用自动登录（无头服务器场景）
set AUTO_LOGIN=0 && meituan-rs.exe
```

## 📊 数据同步机制

| 模式 | 间隔 | 窗口 | 用途 |
|------|------|------|------|
| 微轮询 | 1~10秒随机 | 15分钟 | 极速同步新订单 + 检测撤销 |
| 深度对账 | 30分钟 | 50小时 | 检测48小时自动退款 |

## 🛡️ 安全说明

- Cookie 明文存储在 `meituan_cookies.json`
- 服务绑定 `0.0.0.0`，建议在内网环境使用
- 无用户认证，不适宜直接暴露到公网

## 📝 版本历史

见 [CHANGELOG.md](CHANGELOG.md)

## 📜 许可证

自用项目，非商业用途。

绑定 `0.0.0.0`，局域网内其他设备可通过 `http://本机IP:8899` 访问。

## 数据来源

- 美团内部API：`/couponrecord/queryCouponRecordDetails`
- 通过持久化Cookie（从浏览器提取）进行身份认证
- 无需美团开放平台权限

## 许可证

私有项目
