# 版本变更记录

---

## [v0.7.2] — 2026-07-21

### ⚠️ 重大 Bug 修复：退款/撤销检测逻辑

**根因**：同步脚本使用"API 未返回 = 撤销"的错误逻辑，导致大量正常订单被错误标记为已撤销。

**错误逻辑（已移除）**：
```
本地有但 API 没有 → 标记撤销 ❌
```

**正确逻辑（已修复）**：
```
API 返回 → 检查 description 字段 → 含"退款/撤销/退费/撤单/逆向" → 标记 ✅
```

**影响范围**：
- 快速同步（60秒/次）：15 分钟窗口内订单逐条校验
- 深度同步（30分钟/次）：50 小时窗口内订单逐条校验
- 退款/撤销订单在 API 中**不会被物理删除**，只通过 `description` 字段标记

### 🔧 同步机制重构
- **移除 ureq/reqwest HTTP 客户端**：Rust HTTP 库在远程环境连接美团 API 超时
- **改用 Python 脚本**：`meituan_sync.py` 负责 HTTP 请求 + 数据校验 + 入库
- **辅助脚本**：`http_helper.py` 用于单独 HTTP 请求场景

### 📊 同步频率
- 快速同步：60秒/次（可调 5~3600秒），15分钟窗口，覆盖 10 分钟内撤销验券
- 深度同步：30分钟/次（固定），50小时窗口，覆盖 48 小时自动退款
- 切片策略：1 天切片（API 响应最快，约 1 秒/请求）

### 🍪 Cookie 登录
- 登录页支持文件上传 + 粘贴 Cookie（Cookie-Editor JSON 格式）
- 保存后调用美团 API 校验 Cookie 有效性，显示最近 1 天核销记录数
- 同步锁超时 5 分钟自动释放，防止锁死

### 🧹 清理
- 移除 QR 码登录方案（过于复杂）
- 移除 Chromium CDP 辅助脚本（login_helper.js）

---

## [v0.7.0] — 2026-07-21

### 🏗️ 架构重构：UI 偏好与后端业务配置分离
**根因修复**：后端 `settings.json` 存储列显示设置（columns、key_info、copy_header）与前端默认值不同步，导致 PC 端默认只显示 4 列。

**变更要点**：
- `Settings` 结构体标记 `columns`、`key_info`、`copy_header` 为 `#[serde(skip)]`，不再序列化到 `settings.json`
- `normalize_settings` 移除列校验逻辑
- 前端初始化不再从后端 `s.columns` 恢复列设置，只用 `localStorage` + 内置默认值
- 前端添加 `loadColumnSettings` 版本号机制（`_version: 2`），旧格式自动丢弃
- API `/api/settings` 不再传输 UI 字段，前端列设置保存只走 `localStorage`
- 启动横幅移除"显示列"输出（已不属于后端配置）

**架构原则**：后端管业务配置（班次/计费/刷新间隔），前端管 UI 偏好（列显示/复制偏好），API 契约不传 UI 状态。

### ✨ 新增
- 选择框/输入框自适应文字宽度（`resizeSelect` / `resizeInput`）：选择"全部套餐"显示 4 字宽度，选中长套餐名或输入内容时自动变宽
- 日期时间选择器增加秒数精度（`step="1"`）
- 结束时间自动补 `:59`（`getDT('end')`），确保不遗漏记录

### 🧹 清理
- 移除后端启动横幅中的"显示列"统计输出
- 移除前端从后端恢复列设置的 3 处逻辑

---

## [v0.6.1] — 2026-07-07 (最新稳定版)

**工业级重构 + 全面修复**

### 🔧 关键 Bug 修复
- **P0-1**: 修复 `CheckMenuItem::with_id` 参数顺序（enabled/checked 写反导致菜单项灰色）
- **P0-2**: 修复托盘单击触发两次 `open_browser`（Click 事件匹配了 Down+Up）
- **P0-3**: 修复 `save_settings` 丢失字段（改用 serde 自动序列化）
- **P0-4**: 修复 `removeFeeRow` 仅隐藏 DOM（改为 `.remove()`）
- **P0-5**: 修复前端 `calcFee` 缺少空值守卫
- **P0-6**: 修复 WebSocket `get_cookies_via_cdp` 无限循环（添加 30s 超时）
- **P0-7**: 修复 `http_get` 响应头解析逻辑
- **P0-8**: 修复月份设置字段名不匹配（`month_start` → `month_start_cal`）

### 📋 托盘简化
- 移除诊断控制台功能，日志写入文件即可
- 菜单项简洁：打开网页 / 刷新数据 / 设置 / 关于 / 退出
- 单击打开浏览器（无控制台闪烁）
- 右键菜单全部可用

### 🧹 清理
- 移除 `auto_console`、`auto_browser` 字段
- 移除控制台管理代码
- 统一版本号为 SemVer
- 编译零警告

### 📐 新增文档
- `DESIGN_SYSTEM.md` - UI 设计规范
- `GIT_WORKFLOW.md` - Git 分支/提交规范
- `AUDIT_PROCESS.md` - 审计与测试流程
- `DEPLOY.md` - 部署指南

**升级建议**: 直接替换 `meituan-rs.exe` 即可，数据库向后兼容。

---

### 🔧 关键Bug修复
- **P0-1**: 修复 `CheckMenuItem::with_id` 参数顺序（enabled/checked 写反导致菜单项灰色）
- **P0-2**: 修复托盘单击触发两次 `open_browser`（`Click` 事件匹配了 Down+Up）
- **P0-3**: 修复 `save_settings` 丢失字段（改用 serde 自动序列化）
- **P0-4**: 修复 `removeFeeRow` 仅隐藏 DOM（改为 `.remove()`）
- **P0-5**: 修复前端 `calcFee` 缺少空值守卫
- **P0-6**: 修复 WebSocket `get_cookies_via_cdp` 无限循环（添加 30s 超时）
- **P0-7**: 修复 `http_get` 响应头解析逻辑
- **P0-8**: 修复月份设置字段名不匹配（`month_start` → `month_start_cal`）

### 📋 托盘简化
- 移除诊断控制台功能（日志文件足够）
- 菜单项简洁：打开网页 / 刷新数据 / 设置 / 关于 / 退出
- 单击打开浏览器（cmd /c start，无控制台闪烁）
- 右键菜单全部可用（非灰色）

### 🧹 清理
- 移除 `auto_console`、`auto_browser` 字段
- 移除控制台管理代码
- 统一版本号为 SemVer
- 编译零警告

### 🔧 修复
- **P0-1**: 修复计费规则默认顺序 — 电竞+包天(110元) 优先于通用包天(100元)
- **P0-2**: 后端 Settings 补充 `copy_header`、`month_start_cal`、`month_end_prev` 字段
- **P0-3**: 修正代码注释引用 (`getFeePlans` → `DEFAULT_FEE_PLANS`)
- **P0-4**: 修复 `new_count`/`updated_count` 计数逻辑反转 — 先检查存在性再计数，只计算实际发生变化的订单
- **P0-5**: 修复 `http_get` 空行检测逻辑 — 增加 `line.trim().is_empty()` 判断

### 📋 控制台输出全中文化
- **启动横幅**: 美观的 ASCII 艺术字 + 版本号
- **日志格式**: `2026-07-07 11:30:00 [✅]` (北京时间 + emoji 级别)
- **全中文消息**: 所有 `info!`/`warn!`/`println!` 改为中文
- **屏蔽冗余日志**: actix_server / tray_icon / muda 等非关键日志降为 Error/Warn 级别
- **启动流程**: 4 步指引（检查登录 → 初始化数据库 → 检查配置 → 启动服务）

### 🎨 UI/UX 设计系统
- **CSS 变量统一**: `--primary` / `--success` / `--danger` / `--cola` 等
- **字体系统**: `--text-xs` 到 `--text-xl`
- **间距系统**: `--space-1` 到 `--space-6`
- **圆角系统**: `--radius-sm` 到 `--radius-full`
- **阴影/过渡**: 统一使用 CSS 变量

### 🗄️ 数据库优化
- **新增复合索引**: `idx_consume_refunded` (consume_date + is_refunded)
- **新增复合索引**: `idx_product_info` (product_info)

### 📐 文档
- `GIT_WORKFLOW.md` — Git 分支策略、Commit 规范、SemVer、发布流程
- `AUDIT_PROCESS.md` — Pre-commit 检查、性能审计、安全检查清单
- `DEPLOY.md` — 部署清单、故障排查、文件说明
- `DESIGN_SYSTEM.md` — UI 设计规范

### 🐛 同步修复
- **增量更新准确化**: 只计算字段实际变化的订单为"更新"，不再将所有已存在订单计入
- **启动自动刷新**: `init()` 启动时立即触发一次 `autoSync()`
- **定时刷新增强**: `startAuto()` 每 60 秒调用 `query()` 刷新数据

---

## [v0.5.2] — 2026-07-06

### Added
- **计费规则内存级预加载算法**：彻底解决了每月统计、年度统计和每天明细等需要遍历上万行数据计算计费价时引起的磁盘 I/O 锁死阻塞瓶颈。将读取并解析整个 `settings.json` 的操作提到循环外只执行 1 次，循环内直接使用内存引用，接口运行效率暴涨数百倍。
- **物理失踪订单差集识别算法**：首创本地数据库最近 24/50 小时正常单与美团 API 快照求差集（Set Difference）的防撤单漏记算法。凡在美团返回快照中“物理失踪”但在本地为正常态的订单，判定被“撤销核销”，自动在库中置 `is_refunded=1`。
- **高低频双轨拉取机制**：重构同步窗口推算。1-10秒微轮询采用 15 分钟短查找窗口，极速校验撤单；30分钟定时/手动强刷采用 50 小时深度大轮询，覆查商家自动退款时效。
- **设计系统规范**：创建 `DESIGN_SYSTEM.md`、`GIT_WORKFLOW.md`、`AUDIT_PROCESS.md`、`DEPLOY.md` 文档

### Changed
- **复制过滤净化**：一键复制（`executeCopy`）和降级复制逻辑重构，自动过滤已退款/已撤销的订单，确保不复制至剪切板。
- **券号列去备注化**：渲染券号列时移除了拼接的退款红色标签（由整行红底加删除线替代），保持券号文本纯净。
- **大盘默认过滤**：前端加载时默认将退款状态过滤器锁定在“正常”，不再默认展示历史已退款和已撤销的数据。
- **版本号更新**：0.5.1 → 0.5.2

### Fixed
- **P0-1**: 修复计费规则默认顺序（电竞+包天=110 优先于通用包天=100）
- **P0-2**: 后端 Settings 结构体补充 `copy_header`、`month_start_cal`、`month_end_prev` 字段
- **P0-3**: 修正代码注释引用（`getFeePlans` → `DEFAULT_FEE_PLANS`）
- **P0-4**: 修复 `new_count`/`updated_count` 计数逻辑反转（先检查存在性再计数）
- **P0-5**: 修复 `http_get` 空行检测逻辑
- **启动刷新**: `init()` 启动时自动触发 `autoSync()`
- **自动刷新**: `startAuto()` 每 60 秒调用 `query()` 刷新数据

## [v0.5.1] — 2026-07-06

### Added
- 前端防抖连击锁：为 `query()` 注入 `window._isQuerying` 排它锁，消除由于高频连击异步时序竞争引起的表格渲染错位。
- 自动化注销 SW 的净化脚本：前端在初始化阶段自动物理注销任何过往项目残留在此端口上的旧 Service Worker，消除控制台 PWA 错误。
- 极奢双栏列联动设置面板：重构设置面板为苹果 iOS 风格，将“显示列”与“复制列”双轨滑块联动管理，支持一键复制到剪贴板，无需中间弹窗。

### Changed
- 清除 PWA 残余：移除了无用的 PWA 支持（彻底废退 `sw.js` 和 `manifest.json` 及后端对应的路由）。
- 按钮美学净化：清除了白班和晚班按钮上的 `08-20` / `20-08` 动态时间段后缀，使切换栏更加紧凑纯粹。

### Fixed
- **金融级日趋势核算重构（特危）**：修复了 `build_daily_breakdown` 的 SQLite `GROUP BY` 套餐名随机乘积对账漏洞，重构为 Rust 内存级 HashMap 逐单计费累加算法。
- **美团数据拉取 8小时时区 Bug（大隐患）**：修复了 NaiveDateTime 错误以 UTC 时间解析导致 `beginDate` 误偏向未来的 Bug，纠正为符合东八区的 `FixedOffset::east_opt`。
- **白班 0秒锁死 Bug（重大体验问题）**：修复了 8点至 9点之间由于配置置空逻辑漏洞导致白班结束时间强锁为 08:00:00 导致数据空白的错误，现在白班进行中时结束时间自动跟随当前最新时钟。
- **网络拉取死锁保障**：为后端 `ureq` 的 API 发包器强制加入了 10 秒全局网络超时限制。

---

## [v0.4] — 2026-07-04

### Added
- 统一班次时间设置面板（白班/晚班时间、本月统计范围可配置）
- 月统计支持 00点起/08点起、包含本班/不含本班
- `meituan-rs.exe` 单文件部署（首次运行时自动打开浏览器登录）
- `ARCHITECTURE.md`、`CHANGELOG.md`、`README.md` 项目文档

### Changed
- 数据刷新逻辑从Python脚本改为纯Rust实现（reqwest blocking）
- 月度统计SQL改为 `datetime(consume_date, '-8 hours')`，以白班日08:00为分界
- 服务器绑定 `0.0.0.0:8899`，支持局域网访问
- 前端计费价列移到"门店"列后面

### Removed
- `refresh_data.py` 不再需要
- `meituan_http_api.py`、`meituan_cookie_manager.py` 不再需要

### Fixed
- 白班/晚班按钮单机无响应（缺少 onclick 事件）
- Rust reqwest 异步请求兼容性问题，改用 blocking 模式

---

## [v0.3] — 2026-07-04

### Added
- 纯Rust HTTP服务（actix-web + r2d2连接池）
- 计费价自动匹配（17种套餐配置）
- 双击班次按钮弹出自定义时间
- `/api/health` 接口返回数据库统计
- `/api/refresh` 手动数据刷新接口

### Changed
- Python HTTPServer → Rust actix-web（单线程→4 worker多线程）
- SQLite直连 → r2d2连接池（8连接）
- 自动更新周期 60秒→30秒

### Fixed
- 自动更新reqwest兼容性问题
- 前端 `newBadge` JS语法错误

---

## [v0.2] — 2026-07-03

### Added
- meituan_server_v4.py 纯HTTP服务（无CDP依赖）
- meituan_http_api.py 纯HTTP客户端模块
- Cookie持久化管理（12小时过期检测→API实际认证检测）
- 自定义Pill展开式时间选择器

### Changed
- 数据获取从CDP浏览器 → 持久化Cookie+纯HTTP
- 默认时间范围根据当前时段自动选择白班/晚班

---

## [v0.1] — 2026-06-26

### Added
- 初始版本
- CDP + Playwright 浏览器自动化获取Cookie
- Python HTTPServer 基础订单查询
- Zoho Sheet API 集成
