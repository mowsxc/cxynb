# meituan_orders - 工作规范

## 铁律：每次改代码必做

### 1. 改之前：精读上下文
- 读取目标函数/代码块 ≥20行上下文
- 用 `grep -n "fn name"` 或 `Select-String` 定位精确行号
- 理解代码结构再下手

### 2. 改时：最小化改动
- 一次只改一个函数/一个问题
- 禁止大段复制粘贴覆盖
- 用 `Edit` 精确替换，不改无关行
- 特别注意 `{}` 配对、括号闭合、JavaScript语法

### 3. 改之后：立即验证

#### Rust 代码（每次改完即查）：
```bash
cargo check 2>&1 | Select-String -Pattern "error"
```
发现错误 → 修复 → 再跑，直到0 error

#### JavaScript/HTML（改完即查）：
```bash
# 提取 <script> 内容并检查语法
python -c "import re; open('tmp.js','w').write(re.findall(r'<script>(.*?)</script>', open('meituan_query.html',encoding='utf-8').read(), re.DOTALL)[0])"
node --check tmp.js
```
发现错误 → 修复 → 再跑，直到OK

#### Python 代码（改完即查）：
```bash
python -c "import py_compile; py_compile.compile('cashier/app.py', doraise=True)"
```

### 4. 重启服务并验证接口
```bash
# 后端
Stop-Process -Name "meituan-rs" -Force -ErrorAction SilentlyContinue
cargo build  # release或debug
Start-Process -WindowStyle Hidden -FilePath ".\target\debug\meituan-rs.exe" -WorkingDirectory "D:\code\ZohoSheet\meituan_orders"
# 验证
Invoke-RestMethod -Uri 'http://localhost:8899/api/health'

# 收银台
Stop-Process -Name "python" -Force -ErrorAction SilentlyContinue
# 启动后验证
Invoke-WebRequest -Uri 'http://127.0.0.1:5000/' -UseBasicParsing
```

### 5. 提交前最终检查
- [ ] cargo check 无 error
- [ ] JS node --check 通过
- [ ] Python py_compile 通过
- [ ] 服务启动成功
- [ ] 至少3个API接口响应正常
- [ ] 前端HTML页面可加载

## 技术备忘

### 项目结构
- `src/main.rs`: Rust actix-web HTTP服务（8899端口）
- `meituan_query.html`: 前端订单管理页面
- `cashier/app.py`: Flask收银台服务（5000端口）
- `cashier/templates/cashier.html`: 收银台前端
- `settings.json`: 计费配置/班次配置/**不再存储列显示设置**（columns/key_info/copy_header 已改为 #[serde(skip)]）
- `meituan_cookies.json`: 美团登录凭证
- `meituan_orders.db`: SQLite订单数据库

### 💡 列显示设置架构（重要）
- **列设置为纯前端 UI 偏好**，只存 `localStorage`，不同步到后端
- `Settings` 结构体中 `columns`、`key_info`、`copy_header` 标记为 `#[serde(skip)]`
- `loadColumnSettings` 包含 `_version` 检查，版本不匹配时丢弃旧格式用默认值
- 默认 PC 端显示 10 列：交易快照、类型、券号、消费金额、商家优惠、消费时间、用户手机、备注、验证门店、计费价
- 财务价列默认隐藏，计费价和财务价默认不复制

### 💡 美团同步对账与状态判定规则（防错备忘）
1. **智能双轨同步**：
   - **高频短轮询（1-10秒）**：往前推 15 分钟查找窗口。快速检测 10 分钟内发生“撤销验券”而物理消失的订单。
   - **深度大轮询（30分钟 / 手动刷新）**：往前推 50 小时查找窗口。全面覆查 48 小时自动退款时效。
   - **数据封存**：大于 50 小时的数据为永久存档，绝不重复拉取，严禁触碰历史数据防止造成账目漂移。
2. **撤销判定（差集分析）**：
   - 撤销验券在美团 API 返回的已核销列表中是直接**物理消失**的。
   - 算法：对比本地 24 小时（或 50 小时）内的正常订单与美团快照，凡本地存在但美团已失踪、且核销超 2 分钟的订单，自动置 `is_refunded=1`，备注为 `[已撤销]`。
3. **大盘渲染与复制**：
   - 券号列必须是**纯净券号**，禁止自动备注或拼接“已退款/已撤销”。
   - 默认不显示已撤销/已退款单（`refundFilter` 默认值为 `normal`），且默认**不显示财务价列**。
   - 复制按钮（`executeCopy`）必须自动过滤 `is_refunded=true` 的已撤销/已退款订单，且默认**不复制计费价和财务价列**，同时默认**不复制表头列**，防止复制到 Excel 中导致财务记账错误。

### 关键依赖
- Rust: actix-web 4, tokio, rusqlite, rand, tray-icon
- Python: Flask, sqlite3
- JS: 原生无框架

### 已知编辑风险点
- `meituan_query.html` 的 `<script>` 块有任何语法错误 → 整个页面完全瘫痪
- `settings.json` 的 `fee_json` 字段 → 保存时中文不能走二次JSON序列化
- Flask路由装饰器顺序 → `@app.route` 在上,其他装饰器在下
- `Settings` 结构体的 `columns`/`key_info`/`copy_header` 必须保持 `#[serde(skip)]`，否则会导致前后端同步污染
- `loadColumnSettings` 的 `_version` 必须随 `COLUMNS` 定义变更同步递增
