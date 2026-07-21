# 审计与测试流程规范

**项目**: meituan-rs  
**版本**: 0.5.2  
**更新**: 2026-07-07

---

## 1. 代码审计流程

### 1.1 每次提交前（Pre-commit）

```bash
# 1. 编译检查
cargo check

# 2. 前端 JS 检查
node --check meituan_query.html  # 或用脚本提取JS检查

# 3. 代码格式化
cargo fmt --check

# 4. Lint
cargo clippy -- -D warnings
```

### 1.2 每次发布前（Pre-release）

```bash
# 1. 编译 Release 版本
cargo build --release

# 2. 运行测试
cargo test

# 3. 数据库完整性检查
sqlite3 meituan_orders.db "PRAGMA integrity_check;"

# 4. 检查未使用依赖
cargo udeps  # 需要 cargo install cargo-udeps
```

### 1.3 定期审计（每月）

- 安全审计：检查依赖漏洞 `cargo audit`
- 性能审计：Lighthouse + 后端 profiling
- 数据库审计：索引使用分析、慢查询日志

---

## 2. 自动化测试

### 2.1 单元测试（Rust）

位置：`src/tests/` 或 `src/main.rs` 中的 `#[cfg(test)]` 模块

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calc_fee_basic() {
        let plans = vec![
            FeePlan { cat: "新会员".into(), plan: "特惠".into(), fee: 30.0 },
        ];
        assert_eq!(calc_fee(&plans, "新会员特惠体验"), 30.0);
    }

    #[test]
    fn test_calc_fee_default_fallback() {
        let plans: Vec<FeePlan> = vec![];
        assert_eq!(calc_fee(&plans, "5070显卡包天"), 110.0);
    }

    #[test]
    fn test_calc_financial() {
        // 财务价 = 销售价 - 折扣 - 7%服务费
        let fin = calc_financial("¥100.00", "商家立减:20.00");
        assert_eq!(fin, 100.0 - 20.0 - 7.0);
    }

};
```

运行：`cargo test`

### 2.2 集成测试

位置：`tests/integration.rs`

```rust
#[actix_web::test]
async fn test_health_endpoint() {
    let app = test::init_service(App::new().route("/api/health", web::get().to(handle_health))).await;
    let req = test::TestRequest::get().uri("/api/health").to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
}
```

### 2.3 前端测试

```javascript
// 使用浏览器开发者工具手动测试
// 或使用 Playwright 进行 E2E 测试

// tests/e2e.spec.js (Playwright)
const { test, expect } = require('@playwright/test');

test('页面加载成功', async ({ page }) => {
  await page.goto('http://localhost:8899');
  await expect(page.locator('text=美团订单管理')).toBeVisible();
});

test('查询功能', async ({ page }) => {
  await page.goto('http://localhost:8899');
  await page.click('text=查询');
  await expect(page.locator('#tbody tr')).not.toHaveCount(0);
});
```

---

## 3. 性能优化

### 3.1 前端性能（Lighthouse）

**目标分数**：
- Performance ≥ 90
- Accessibility ≥ 90
- Best Practices ≥ 90

**优化清单**：
- [ ] CSS/JS 最小化（生产环境）
- [ ] 图片使用 WebP 格式
- [ ] 关键渲染路径优化
- [ ] 减少主线程工作
- [ ] 避免大型 DOM（虚拟滚动）
- [ ] 字体加载优化

```bash
# 运行 Lighthouse
npx lighthouse http://localhost:8899 --view --output html --output-path report.html
```

### 3.2 后端性能（Profiling）

```rust
// 使用 perf/flamegraph 分析热点
// cargo install flamegraph
cargo flamegraph --bin meituan-rs

// 或使用 criterion 做基准测试
#[bench]
fn bench_calc_fee(b: &mut Bencher) {
    b.iter(|| calc_fee(&plans, "5070显卡4小时体验券"));
}
```

**优化清单**：
- [ ] 连接池大小调优
- [ ] 减少锁竞争（SQLite 写入串行化）
- [ ] 预编译语句缓存
- [ ] 避免不必要的 Clone
- [ ] 异步任务不阻塞主线程
- [ ] 内存预加载（已实施）

### 3.3 数据库索引优化

```sql
-- 现有索引
CREATE INDEX IF NOT EXISTS idx_consume_date ON orders(consume_date);
CREATE INDEX IF NOT EXISTS idx_coupon_value ON orders(coupon_value);
CREATE INDEX IF NOT EXISTS idx_is_refunded ON orders(is_refunded);

-- 推荐新增复合索引（根据查询模式）
CREATE INDEX IF NOT EXISTS idx_consume_date_refunded ON orders(consume_date, is_refunded);
CREATE INDEX IF NOT EXISTS idx_product_info ON orders(product_info);

-- 查询性能分析
EXPLAIN QUERY PLAN
SELECT * FROM orders WHERE consume_date >= '2026-01-01' AND is_refunded = 0;

-- 索引使用统计
SELECT name, tbl_name FROM sqlite_master WHERE type = 'index';
```

---

## 4. 安全检查清单

- [ ] SQL 注入防护（使用参数化查询）
- [ ] XSS 防护（前端 esc() 函数转义）
- [ ] CSRF 防护（SameSite Cookie）
- [ ] 敏感信息不暴露（Cookie 不明文传输）
- [ ] 输入验证（API 参数校验）
- [ ] 依赖漏洞检查（`cargo audit`）
- [ ] 日志脱敏（不记录敏感信息）

---

## 5. 持续集成（CI/CD）

### GitHub Actions 配置

```yaml
# .github/workflows/ci.yml
name: CI
on: [push, pull_request]

jobs:
  test:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo check
      - run: cargo test
      - run: cargo clippy -- -D warnings
      - run: cargo fmt --check
      
  build:
    needs: test
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo build --release
      - uses: actions/upload-artifact@v4
        with:
          name: meituan-rs
          path: target/release/meituan-rs.exe
```

---

**本文档为审计与测试流程唯一规范，所有开发需遵守。**
