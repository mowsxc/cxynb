# 项目上下文：美团订单管理系统

## 项目概述
美团商家后台订单自动抓取、对账、计费系统。Rust 后端 + 原生 JS 前端，单 EXE 运行。

## 技术栈
- **后端**: Rust 1.80+, actix-web 4, rusqlite (r2d2 连接池), ureq, tray-icon
- **前端**: 原生 HTML/CSS/JS（无框架），单文件 `meituan_query.html`
- **数据库**: SQLite
- **构建**: `cargo build` → 单 EXE

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

### 文件职责
| 文件 | 职责 |
|------|------|
| `src/main.rs` | HTTP 服务 + 托盘图标 + 双轨同步 + 设置对话框 |
| `meituan_query.html` | 订单查询/显示/复制/导出（纯前端） |
| `settings.json` | 业务配置（班次/计费规则/刷新间隔） |
| `meituan_cookies.json` | 美团登录凭证 |

## 关键模块

### 双轨智能同步
- **高频短轮询**（1-10s）：前推 15 分钟，检测 10 分钟内撤销验券
- **深度大轮询**（30m/手动）：前推 50 小时，覆盖 48h 自动退款
- **数据封存**：>50h 历史数据永不重复拉取

### 列显示设置（前端）
```javascript
const COLUMNS = [
  {id:'product_info', label:'交易快照', pc:true, mob:true},
  {id:'product_type', label:'类型', pc:true, mob:false},
  // ... 共 11 列
  {id:'financial', label:'财务价', pc:false, mob:false},  // 默认隐藏
];
```
- PC 默认显示 10 列（财务价隐藏）
- 计费价/财务价默认不复制
- `loadColumnSettings()` 有 `_version` 检查

### 安全约束
- 券号列必须是纯净券号，禁止拼接备注
- 复制时自动过滤 `is_refunded=true` 的订单
- 默认不显示已撤销/已退款单

## 开发命令

```bash
# 构建
cargo build

# 运行
./target/debug/meituan-rs.exe

# 验证
cargo check 2>&1 | grep -i error
node --check <(python -c "import re;print(re.findall(r'<script>(.*?)</script>',open('meituan_query.html').read(),re.DOTALL)[0])")

# API 测试
curl http://localhost:8899/api/health
curl http://localhost:8899/api/settings
```

## 已知编辑风险点
1. `meituan_query.html` 的 `<script>` 块任何语法错误 → 整个页面瘫痪
2. `settings.json` 的 `fee_json` 字段 → 中文不能走二次 JSON 序列化
3. `Settings` 结构体字段必须保持 `#[serde(skip)]`
4. `COLUMNS` 定义变更时必须递增 `COLUMN_SETTINGS_VERSION`
5. Flask 路由装饰器顺序：`@app.route` 在上

## 最近变更（v0.7.0）
- 架构重构：UI 偏好与后端业务配置彻底分离
- 新增列显示自检函数和 E2E 测试标记
- 新增 `resizeSelect`/`resizeInput` 自适应宽度
- 日期时间选择器支持秒数精度
- Windows 托盘 GUI 移除了 web 专属的列设置复选框

## 继续开发注意事项
1. 任何涉及 `columns`/`key_info`/`copy_header` 的改动，必须确认 `#[serde(skip)]` 仍在
2. 前端列显示逻辑改动后，必须验证 PC 默认 10 列、财务价隐藏
3. 新增 API 字段时，确认是否属于 UI 偏好（不应存后端）
4. 修改 `COLUMNS` 后运行自检：浏览器 Console 应输出 `[自检] 列显示: 10/11 列可见`
