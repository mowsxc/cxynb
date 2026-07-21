# 美团订单管理系统 - 部署指南

**项目**: meituan-rs (美团订单管理)  
**版本**: 0.5.1  
**类型**: 自用项目（非商业通用）  
**更新**: 2026-07-05

---

## 📦 部署文件清单

### 核心文件（必需）

| 文件 | 大小 | 说明 |
|------|------|------|
| `meituan-rs.exe` | ~6.8MB | 主程序（Release 版本，含图标） |
| `meituan_query.html` | ~54KB | 前端单页应用 |
| `vc_redist.x64.exe` | ~24.4MB | Visual C++ 运行库（离线安装包，已下载） |

### 数据文件（推荐打包，自用项目）

| 文件 | 说明 |
|------|------|
| `meituan_orders.db` | SQLite 数据库（订单历史，永久保存） |
| `meituan_cookies.json` | 美团登录凭证（Cookie，自动增量更新） |
| `settings.json` | 业务规则配置（班次、计费规则） |

### 工具文件（可选）

| 文件 | 说明 |
|------|------|
| `meituan_cookie_manager.py` | Cookie 手动管理工具 |
| `python.exe` | Python 运行时（仅用于 cookie 工具） |

---

## 🚀 快速部署（推荐）

### 步骤 1：准备文件

将以下文件复制到目标电脑同一目录：

```
目标目录/
├── meituan-rs.exe              ← 必需
├── meituan_query.html          ← 必需
├── VisualCppRedist_Aio.exe     ← 必需（首次运行）
├── meituan_orders.db           ← 推荐（携带历史数据）
├── meituan_cookies.json        ← 推荐（携带登录状态）
└── settings.json               ← 可选（携带业务规则）
```

### 步骤 2：安装运行库（首次）

```bash
# 双击运行
VisualCppRedist_Aio.exe
```

**注意**：如果目标电脑已安装 Visual C++ Redistributable，可跳过此步。

### 步骤 3：启动程序

```bash
# 双击运行
meituan-rs.exe
```

**首次启动流程**：
1. 程序启动 HTTP 服务（端口 8899）
2. 自动打开浏览器 `http://localhost:8899`
3. 如果 `meituan_cookies.json` 不存在或失效：
   - 自动弹出 Edge 浏览器
   - 跳转到美团商家后台登录页
   - 登录成功后自动提取 Cookie 并保存
4. 启动后自动触发一次数据刷新（从美团 API 拉取最新订单）

### 步骤 4：验证

浏览器打开后应看到：
- 顶部标题栏：`美团订单管理`
- 统计信息：`数据库 X 条 | 日期范围 | 更新时间 | v2026...`
- 表格：订单列表（若有历史数据）

---

## ❓ 常见问题

### Q1: meituan_cookies.json 是默认内置的吗？

**不是**。

- 代码中检查：`if std::path::Path::new("meituan_cookies.json").exists()`
- 如果不存在，启动时会 warn "Cookie: not found"
- 然后 `ensure_cookies()` 会通过 CDP（Chrome DevTools Protocol）自动打开 Edge 浏览器
- 跳转到美团商家后台，**手动登录后自动提取 Cookie**
- 提取成功后保存到 `meituan_cookies.json`

**所以**：
- 首次使用：需要手动登录一次
- 后续使用：如果 Cookie 有效，无需登录
- Cookie 失效（401/403）：会重新触发登录流程

### Q2: 为什么数据库文件不打包？

**可以打包**。

由于这是**自用项目**（非商业通用），数据完整性有保障：

**打包策略**：
```
✅ 打包 meituan_orders.db（携带所有历史订单）
✅ 打包 meituan_cookies.json（携带登录状态）
✅ 打包 settings.json（携带业务规则）
```

**启动后行为**：
1. 程序启动
2. 自动调用 `rust_refresh()`（每 60 秒一次）
3. 从美团 API 拉取**增量数据**（从数据库最新时间开始）
4. 自动补全新订单、更新退款状态

**优势**：
- 首次启动无需等待全量拉取
- 保留所有历史统计数据
- 保留所有计费价记录

### Q3: 订单数据是永久的吗？

**是的**（除非退款）。

**订单生命周期**：
- 订单生成 → 写入数据库 → 永久保存
- 退款发生 → `is_refunded` 字段更新为 `1`
- 其他字段（商品、金额、时间）**不会改变**

**所以**：
- 打包数据库文件是安全的
- 启动后只需增量更新（新订单 + 退款状态）
- 不会出现数据错乱

### Q4: 启动后自动更新补全吗？

**是的**。

**自动刷新机制**：
1. **后端**：`actix_web::rt::spawn` 每 60 秒调用 `rust_refresh()`
2. **前端**：`startAuto()` 每 60 秒调用 `query()` 刷新表格

**首次启动**：
- 如果有 `meituan_cookies.json`，自动触发一次 `rust_refresh()`
- 从数据库最新时间开始拉取增量数据
- 自动补全新订单、更新退款状态

### Q5: 数据库文件损坏怎么办？

**备份策略**：
```bash
# 定期备份（建议每周）
copy meituan_orders.db meituan_orders_backup_$(date +%Y%m%d).db
```

**恢复策略**：
- 如果数据库损坏，删除 `meituan_orders.db`
- 重启程序，首次启动会从美团 API 拉取全量数据重建

---

## 📋 完整部署清单（自用项目推荐）

### 最小部署（仅可执行文件）

```
meituan-rs.exe
meituan_query.html
VisualCppRedist_Aio.exe  (首次)
```

**首次启动**：
- 自动弹出浏览器登录美团
- 自动拉取全量数据（可能需要几分钟）

### 完整部署（推荐，携带历史数据）

```
meituan-rs.exe
meituan_query.html
VisualCppRedist_Aio.exe  (首次)
meituan_orders.db        (历史订单)
meituan_cookies.json     (登录状态)
settings.json            (业务规则)
```

**首次启动**：
- 直接加载历史数据
- 自动增量更新（几秒完成）
- 无需等待全量拉取

---

## 🔧 故障排查

### 问题 1: 启动提示缺少 DLL

**症状**：`The program can't start because VCRUNTIME140.dll is missing`

**解决**：
```bash
# 运行 Visual C++ 运行库安装包
VisualCppRedist_Aio.exe
```

### 问题 2: 没有系统托盘图标

**原因**：使用的是 Debug 版本

**解决**：
```bash
# 使用 Release 版本
target\release\meituan-rs.exe
```

### 问题 3: Cookie 失效，自动登录失败

**症状**：启动后一直停留在登录页

**解决**：
1. 删除 `meituan_cookies.json`
2. 重启程序
3. 手动完成登录流程

### 问题 4: 数据库损坏

**症状**：启动后提示数据库错误

**解决**：
```bash
# 删除损坏的数据库
del meituan_orders.db

# 重启程序（会自动重建）
meituan-rs.exe
```

---

## 📝 更新日志

### v0.5.1 (2026-07-05)
- ✅ 纯 Rust 刷新（删除 Python 依赖）
- ✅ 年度统计图表（月/季/半年/年）
- ✅ 财务价计算（实收金额 - 服务费 7%）
- ✅ 列显示设置（localStorage 保存）
- ✅ 业务规则共享（后端 API）

### v0.5.0 (2026-07-04)
- ✅ 系统托盘集成
- ✅ 列显示设置面板
- ✅ 财务价列

---

## 🔗 相关文档

- [DESIGN_SYSTEM.md](DESIGN_SYSTEM.md) - UI 设计规范
- [ARCHITECTURE.md](ARCHITECTURE.md) - 架构设计
- [CHANGELOG.md](CHANGELOG.md) - 更新日志

---

**部署完成！启动 `meituan-rs.exe` 即可使用。**
