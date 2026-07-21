# 架构设计文档

## 系统架构

```
┌────────────────────────────────────────────────────────────────┐
│                       meituan-rs.exe                          │
│                                                                │
│  ┌───────────────────┐     ┌──────────────────────────────┐   │
│  │   Cookie登录模块   │     │      HTTP服务 (actix-web)    │   │
│  │  (首次/过期时调用)  │     │  4 worker, 0.0.0.0:8899    │   │
│  │                   │     │                              │   │
│  │  CDP WebSocket →  │     │  /         → meituan_query   │   │
│  │  提取浏览器Cookie │     │  /api/health                │   │
│  └────────┬──────────┘     │  /api/stats                 │   │
│           │                │  /api/query                 │   │
│           ▼                │  /api/stats/detail          │   │
│  ┌───────────────────┐     │  /api/refresh               │   │
│  │  meituan_cookies  │     └───────────┬──────────────────┘   │
│  │  .json            │                 │                      │
│  └────────┬──────────┘                 │                      │
│           │                            │                      │
│           ▼                            ▼                      │
│  ┌────────────────────────────────────────────────────────┐   │
│  │     双轨智能数据刷新模块 (高频 1-10s 短轮询 + 30m 深度大轮询)     │   │
│  │                                                        │   │
│  │  ureq (10s超时) → 差集分析防撤单判定 → 内存预加载计费 → upsert   │   │
│  └────────────────────────┬───────────────────────────────┘   │
│                           │                                    │
│                           ▼                                    │
│  ┌────────────────────────────────────────────────────────┐   │
│  │              SQLite (r2d2连接池 8连接)                  │   │
│  │  meituan_orders.db                                     │   │
│  │  表: orders (coupon_value UNIQUE)                      │   │
│  └────────────────────────────────────────────────────────┘   │
└────────────────────────────────────────────────────────────────┘
```

## 数据流

### 首次启动
```
meituan-rs.exe
  → 检测 meituan_cookies.json
  → 不存在 → 打开Edge浏览器 → 用户手动登录美团
  → CDP WebSocket → Network.getAllCookies
  → 保存到 meituan_cookies.json
  → 启动HTTP服务
```

### 正常运行与滑动切片保障
```
高频微轮询 (每1-10秒随机):
  基于最新订单时间锚点前推15分钟 → 提取 Cookie 鉴权 → 发送 API 请求
  → 比对本地正常订单与 API 快照求差集 → 自动判定并更正物理消失的撤单数据 (is_refunded = 1)
  
深度大轮询 (每30分钟定时/手动强刷):
  基于最新订单时间锚点前推50小时 → 全量拉取美团记录
  → 同步48稳时内超时未理自动退款的订单
  → 大于50小时的历史旧账自动锁定归档，禁止覆写偏移

分段滑动时间窗安全切片 (长跨度容灾保障):
  当系统因关机（如1个月）导致本地最新订单与当前时钟存在超大断层时：
  系统自动启用分段滑动时间窗算法，以 3 天为最大步长进行切片，小包、多批次循环拉取，
  避开美团对单次查询跨度的限制，并彻底规避 5000 条硬编码分页上限，保证绝对不漏单。

HTTP请求到达:
  内存一次性预加载 settings.json 计费规则 (免去循环重复读盘)
  → 内存级折算与累加汇总 → 毫秒级极速响应大盘
```

### 工作流程
```
浏览器登录 → Cookie → 纯HTTP抓取 → SQLite → Web界面
 (一次)      (持久化)   (每60秒)     (本地)    (localhost)
```

## 数据库设计

```sql
CREATE TABLE orders (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    coupon_value    TEXT UNIQUE,        -- 券号（去重键）
    order_id        TEXT,                -- 订单ID
    product_info    TEXT,                -- 交易快照
    product_type    TEXT,                -- 商品类型
    sale_price      TEXT,                -- 消费金额
    discount_price  TEXT,                -- 商家优惠金额
    consume_date    TEXT,                -- 消费时间
    mobile          TEXT,                -- 用户手机
    description     TEXT,                -- 备注
    shop_info       TEXT,                -- 验证门店
    verify_account  TEXT,                -- 核销账号
    is_refunded     INTEGER DEFAULT 0,   -- 是否已退款
    extra_json      TEXT,                -- 扩展字段
    updated_at      TEXT DEFAULT CURRENT_TIMESTAMP
);

-- 索引
idx_consume_date   ON orders(consume_date)
idx_coupon_value   ON orders(coupon_value)
idx_is_refunded    ON orders(is_refunded)
```

## 关键技术决策

### 为什么用Rust而不是Python？
- Python HTTPServer单线程，reqwest阻塞时整个服务卡死
- Rust的actix-web 4 worker线程，独立后台任务，互不阻塞
- 编译为原生exe，无需运行时依赖

### 为什么废退 Python 脚本，改用全 Rust 自研 CDP？
- 避免用户在 Windows 上必须额外安装 Python 与复杂的 Playwright 环境。
- 在 Rust 中通过 TCP 端口对拉起的 Edge 浏览器建立 Chrome 调试协议（CDP）WebSocket 双向通信，以无头/有头兼容模式直接获取并保存 Cookie，真正实现单 EXE 文件解压即用。

### 为什么用 ureq 代替 reqwest？
- `reqwest` 的 Rust 编译依赖极重（包含大量 async 运行时与 openSSL 配置），且在旧版 Windows 系统的 TLS 握手上有兼容性警报。
- `ureq` 是一个轻量级、无 async 依赖的阻塞式 HTTP 客户端，编译体积极小，对多线程下 Actix 的阻塞线程生成完美兼容。

### 计费价匹配与性能策略
```
1. 内存级预加载 (load_fee_plans): 
   - 外部调用时一次性在内存中反序列化 settings.json 的 fee_json 规则列表
   - 避免了在数千条对账记录循环中高频重复读磁盘造成 I/O 阻塞。
2. product_info (交易快照) 文本关键词双重匹配:
   - 例: "5070显卡3小时体验券[128.00元][1345882867]"
   - 匹配规则：包含 "5070显卡" 且 包含 "3小时" → 折算计费价为 ¥34.00。
   - 无自定义匹配规则时，自动落入后端与前端完全对齐的 17 种硬编码默认计费规则。
```

## 安全说明

- Cookie明文存储在 `meituan_cookies.json`
- 服务绑定 `0.0.0.0`，建议在内网环境使用
- 无用户认证，不适宜直接暴露到公网
- **列显示设置为纯前端 UI 偏好**，仅存储于 `localStorage`，后端 `settings.json` 通过 `#[serde(skip)]` 彻底隔离，API 不传输 UI 状态
